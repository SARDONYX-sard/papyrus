use std::error::Error;

use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use tree_sitter::Parser;

struct Backend {
    client: Client,
    parser: RwLock<Parser>,
}

fn new_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_papyrus_sse::LANGUAGE.into())
        .expect("failed to load grammar");
    parser
}

fn collect_diagnostics(tree: &tree_sitter::Tree, src: &str) -> Vec<Diagnostic> {
    let mut out = vec![];

    let root = tree.root_node();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if node.is_error() {
            let range = node.range();
            let message = explain_error(node, src);

            out.push(Diagnostic {
                range: Range {
                    start: Position::new(
                        range.start_point.row as u32,
                        range.start_point.column as u32,
                    ),
                    end: Position::new(range.end_point.row as u32, range.end_point.column as u32),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message,
                source: Some("papyrus-sse".to_string()),
                ..Default::default()
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    out
}

fn explain_error(node: tree_sitter::Node, src: &str) -> String {
    let parent = node.parent();

    match parent.map(|p| p.kind()) {
        Some("callExpression") => {
            "function call is missing arguments or wrong arguments".to_string()
        }
        Some("binaryExpression") => "invalid expression on both sides of operator".to_string(),
        Some("assignment") => "invalid assignment".to_string(),
        Some("function") => "invalid function definition".to_string(),
        _ => {
            let text = &src[node.byte_range()];
            format!("syntax error near '{}'", text)
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "papyrus-lsp".into(),
                version: Some("0.1".into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let _ = self
            .client
            .log_message(MessageType::INFO, "parser diagnostic LSP started")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let text = params.text_document.text;

        // parse
        let mut parser = self.parser.write().await;
        let tree = match parser.parse(&text, None) {
            Some(t) => t,
            None => return,
        };

        // diagnostics
        let diags = collect_diagnostics(&tree, &text);

        self.client
            .publish_diagnostics(params.text_document.uri, diags, None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = &params.content_changes[0].text;

        let mut parser = self.parser.write().await;
        let tree = match parser.parse(text, None) {
            Some(t) => t,
            None => return,
        };

        let diags = collect_diagnostics(&tree, text);

        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn shutdown(&self) -> Result<(), tower_lsp::jsonrpc::Error> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (service, socket) = LspService::new(|client| Backend {
        client,
        parser: RwLock::new(new_parser()),
    });

    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;

    Ok(())
}
