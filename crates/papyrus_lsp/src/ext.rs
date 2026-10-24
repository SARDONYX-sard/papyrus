//! Protocol extensions specific to this server.

use lsp_types::{TextDocumentIdentifier, request::Request};
use serde::{Deserialize, Serialize};

/// `papyrus/restart`: soft-restarts the server.
///
/// Drops all indexes, applies the optional settings, re-indexes the workspace
/// and republishes diagnostics. Open documents are kept. Returns the status
/// after the restart was initiated (indexing runs in the background).
pub(crate) enum Restart {}

impl Request for Restart {
    type Params = Option<RestartParams>;
    type Result = StatusResult;
    const METHOD: &'static str = "papyrus/restart";
}

/// Parameters of [`Restart`].
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestartParams {
    /// New settings, in the same format as `initializationOptions`.
    pub(crate) settings: Option<serde_json::Value>,
}

/// `papyrus/status`: what the server currently knows.
pub(crate) enum Status {}

impl Request for Status {
    type Params = ();
    type Result = StatusResult;
    const METHOD: &'static str = "papyrus/status";
}

/// Result of [`Status`] and [`Restart`].
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusResult {
    /// Number of known script files.
    pub(crate) files: usize,
    /// Number of files open in the editor.
    pub(crate) open_files: usize,
    /// Whether the workspace is being indexed.
    pub(crate) indexing: bool,
    /// Incremented on every restart.
    pub(crate) generation: u64,
    /// Workspace folders.
    pub(crate) roots: Vec<String>,
    /// Configured import directories.
    pub(crate) import_directories: Vec<String>,
}

/// `papyrus/syntaxTree`: a debug dump of the syntax tree of a document.
pub(crate) enum SyntaxTree {}

impl Request for SyntaxTree {
    type Params = SyntaxTreeParams;
    type Result = String;
    const METHOD: &'static str = "papyrus/syntaxTree";
}

/// Parameters of [`SyntaxTree`].
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyntaxTreeParams {
    /// The document.
    pub(crate) text_document: TextDocumentIdentifier,
}
