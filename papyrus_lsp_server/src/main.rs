use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::RwLock;
use tower_lsp::{Client, LanguageServer, LspService, Server, jsonrpc::Result, lsp_types::*};

use tree_sitter::{Parser, Tree};

use tree_sitter_papyrus_sse;

/* =========================================================
 * AST CACHE
 * ========================================================= */

#[derive(Clone)]
struct Document {
    text: String,
    tree: Tree,
}

/* =========================================================
 * SYMBOL
 * ========================================================= */

#[derive(Clone)]
struct Symbol {
    name: String,
    file: PathBuf,
    line: usize,
}

/* =========================================================
 * STATE
 * ========================================================= */

struct Backend {
    client: Client,
    parser: RwLock<Parser>,
    docs: RwLock<HashMap<String, Document>>,
    symbols: RwLock<HashMap<String, Vec<Symbol>>>,
}

/* =========================================================
 * INIT PARSER
 * ========================================================= */

fn new_parser() -> Parser {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_papyrus_sse::LANGUAGE.into())
        .expect("grammar load failed");
    p
}

/* =========================================================
 * TREE HELPERS
 * ========================================================= */

fn collect_errors(tree: &Tree) -> Vec<Diagnostic> {
    let mut out = vec![];
    let root = tree.root_node();

    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        if node.is_error() || node.has_error() {
            let r = node.range();

            out.push(Diagnostic {
                range: Range {
                    start: Position::new(r.start_point.row as u32, r.start_point.column as u32),
                    end: Position::new(r.end_point.row as u32, r.end_point.column as u32),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: "syntax error".into(),
                ..Default::default()
            });
        }
    }

    out
}

fn extract_functions(src: &str, tree: &Tree, file: PathBuf) -> HashMap<String, Vec<Symbol>> {
    let mut map = HashMap::new();
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
                }
            }

            if let Some(name) = name {
                map.entry(name).or_default().push(Symbol {
                    name,
                    file: file.clone(),
                    line: node.start_position().row,
                });
            }
        }
    }

    map
}

/* =========================================================
 * FORMATTER（簡易）
 * ========================================================= */

fn format_papyrus(src: &str) -> String {
    let mut indent = 0;
    let mut out = String::new();

    for line in src.lines() {
        let t = line.trim();

        if t.starts_with("End") {
            indent = indent.saturating_sub(1);
        }

        out.push_str(&"  ".repeat(indent));
        out.push_str(t);
        out.push('\n');

        if t.starts_with("Function") || t.starts_with("If") || t.starts_with("While") {
            indent += 1;
        }
    }

    out
}

/* =========================================================
 * LSP
 * ========================================================= */

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "papyrus-lsp".into(),
                version: Some("0.2".into()),
            }),
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                definition_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Papyrus LSP ready")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text;

        let mut parser = self.parser.write().await;
        let tree = parser.parse(&text, None).unwrap();

        let docs = Document {
            text: text.clone(),
            tree: tree.clone(),
        };

        let mut cache = self.docs.write().await;
        cache.insert(uri.clone(), docs);

        let diags = collect_errors(&tree);

        self.client
            .publish_diagnostics(params.text_document.uri, diags, None)
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let docs = self.docs.read().await;

        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let contents = HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```papyrus\n{}\n```", doc.text),
        });

        Ok(Some(Hover {
            contents,
            range: None,
        }))
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let symbols = self.symbols.read().await;

        let mut items = vec![];

        for k in symbols.keys() {
            items.push(CompletionItem::new_simple(k.clone(), "function".into()));
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();

        let docs = self.docs.read().await;

        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let pos = params.text_document_position_params.position;

        let word = doc
            .text
            .split_whitespace()
            .find(|w| w.contains("Function"))
            .unwrap_or("Function");

        let symbols = self.symbols.read().await;

        if let Some(list) = symbols.get(word) {
            let s = &list[0];

            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: Url::from_file_path(&s.file).unwrap(),
                range: Range::default(),
            })));
        }

        Ok(None)
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri.to_string();
        let docs = self.docs.read().await;

        let doc = match docs.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let formatted = format_papyrus(&doc.text);

        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(u32::MAX, 0),
            },
            new_text: formatted,
        }]))
    }

    async fn shutdown(&self) -> Result<()> {
     
    }
}

/* =========================================================
 * MAIN
 * ========================================================= */

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| {
        let parser = new_parser();

        Backend {
            client,
            parser: RwLock::new(parser),
            docs: RwLock::new(HashMap::new()),
            symbols: RwLock::new(HashMap::new()),
        }
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
