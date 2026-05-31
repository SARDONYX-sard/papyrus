use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::mpsc;

use tower_lsp::{Client, LanguageServer, LspService, Server, lsp_types::*};

use tree_sitter::{Parser, Tree};

use tree_sitter_papyrus_sse;

/* =========================================================
 * SYMBOL
 * ========================================================= */

#[expect(unused)]
#[derive(Debug, Clone)]
struct Symbol {
    name: String,
    kind: SymbolKind,
    file: PathBuf,
    line: usize,
}

#[derive(Debug, Clone, Copy)]
enum SymbolKind {
    #[expect(unused)]
    Script,
    Function,
}

#[derive(Default)]
struct SymbolTable {
    scripts: HashMap<String, Symbol>,
    functions: HashMap<String, Vec<Symbol>>,
}

impl SymbolTable {
    #[expect(unused)]
    fn insert_script(&mut self, s: Symbol) {
        self.scripts.insert(s.name.clone(), s);
    }

    fn insert_function(&mut self, s: Symbol) {
        self.functions.entry(s.name.clone()).or_default().push(s);
    }
}

/* =========================================================
 * PARSER
 * ========================================================= */

struct TsParser {
    parser: Parser,
}

impl TsParser {
    fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_papyrus_sse::LANGUAGE.into())
            .expect("grammar load failed");

        Self { parser }
    }

    fn parse(&mut self, src: &str) -> Option<Tree> {
        self.parser.parse(src, None)
    }
}

/* =========================================================
 * INDEX EVENT (progress channel)
 * ========================================================= */

#[derive(Debug)]
enum IndexEvent {
    Progress { done: usize, total: usize },
    Done,
}

/* =========================================================
 * INDEXER
 * ========================================================= */

struct Indexer {
    parser: TsParser,
    table: Arc<tokio::sync::RwLock<SymbolTable>>,
}

impl Indexer {
    fn new(parser: TsParser, table: Arc<tokio::sync::RwLock<SymbolTable>>) -> Self {
        Self { parser, table }
    }

    fn collect_psc_files(base: &Path) -> Vec<PathBuf> {
        let mut out = vec![];
        Self::walk(base, &mut out);
        out
    }

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for e in entries.flatten() {
            let p = e.path();

            if p.is_dir() {
                Self::walk(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("psc") {
                out.push(p);
            }
        }
    }

    fn index_file(&mut self, path: &Path) {
        let Ok(src) = fs::read_to_string(path) else {
            return;
        };

        let Some(tree) = self.parser.parse(&src) else {
            return;
        };

        let root = tree.root_node();

        let mut cursor = root.walk();
        for node in root.children(&mut cursor) {
            if node.kind().contains("function") {
                let mut name = None;

                let mut c = node.walk();
                for ch in node.children(&mut c) {
                    if ch.kind() == "identifier" {
                        name = src
                            .get(ch.start_byte()..ch.end_byte())
                            .map(|s| s.to_string());
                        break;
                    }
                }

                if let Some(name) = name {
                    let symbol = Symbol {
                        name,
                        kind: SymbolKind::Function,
                        file: path.to_path_buf(),
                        line: node.start_position().row,
                    };

                    let table = self.table.clone();
                    let mut guard = futures::executor::block_on(table.write());
                    guard.insert_function(symbol);
                }
            }
        }
    }

    fn run_workspace_index(&self, skyrim_dir: PathBuf, sender: mpsc::Sender<IndexEvent>) {
        let parser = TsParser::new();
        let table = self.table.clone();

        std::thread::spawn(move || {
            let mut local = Indexer { parser, table };

            let base = skyrim_dir.join("Source").join("Scripts");
            let files = Self::collect_psc_files(&base);

            let total = files.len();

            for (i, f) in files.iter().enumerate() {
                local.index_file(f);

                let _ = sender.blocking_send(IndexEvent::Progress { done: i + 1, total });
            }

            let _ = sender.blocking_send(IndexEvent::Done);
        });
    }
}

/* =========================================================
 * BACKEND
 * ========================================================= */

struct Backend {
    client: Client,
    indexer: Indexer,
    table: Arc<tokio::sync::RwLock<SymbolTable>>,
}

/* =========================================================
 * LSP
 * ========================================================= */

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "papyrus-lsp".into(),
                version: Some("0.1".into()),
            }),
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "papyrus-lsp started")
            .await;

        let (tx, mut rx) = mpsc::channel::<IndexEvent>(32);

        let client = self.client.clone();

        tokio::spawn(async move {
            let token = NumberOrString::String("papyrus-index".into());

            let _ = client
                .send_request::<request::WorkDoneProgressCreate>(WorkDoneProgressCreateParams {
                    token: token.clone(),
                })
                .await;

            while let Some(event) = rx.recv().await {
                match event {
                    IndexEvent::Progress { done, total } => {
                        let percent = (done * 100 / total.max(1)) as u32;

                        let _ = client
                            .send_notification::<notification::Progress>(ProgressParams {
                                token: token.clone(),
                                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                                    WorkDoneProgressReport {
                                        message: Some("indexing papyrus".into()),
                                        percentage: Some(percent),
                                        cancellable: None,
                                    },
                                )),
                            })
                            .await;
                    }
                    IndexEvent::Done => {
                        let _ = client
                            .send_notification::<notification::Progress>(ProgressParams {
                                token,
                                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(
                                    WorkDoneProgressEnd {
                                        message: Some("done".into()),
                                    },
                                )),
                            })
                            .await;
                        break;
                    }
                }
            }
        });

        let skyrim_dir = PathBuf::from("D:/STEAM/steamapps/common/Skyrim Special Edition/Data");

        self.indexer.run_workspace_index(skyrim_dir, tx);
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn hover(&self, _: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let table = self.table.read().await;

        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "scripts: {}, functions: {}",
                table.scripts.len(),
                table.functions.len()
            ))),
            range: None,
        }))
    }

    async fn completion(
        &self,
        _: CompletionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<CompletionResponse>> {
        let table = self.table.read().await;

        let mut items = vec![];

        for k in table.functions.keys() {
            items.push(CompletionItem::new_simple(k.clone(), "function".into()));
        }

        for k in table.scripts.keys() {
            items.push(CompletionItem::new_simple(k.clone(), "script".into()));
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}

/* =========================================================
 * MAIN
 * ========================================================= */

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let table = Arc::new(tokio::sync::RwLock::new(SymbolTable::default()));

    let (service, socket) = LspService::new(|client| {
        let parser = TsParser::new();

        let indexer = Indexer::new(parser, table.clone());

        Backend {
            client,
            indexer,
            table,
        }
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
