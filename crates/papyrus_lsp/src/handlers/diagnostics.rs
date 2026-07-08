//! `textDocument/publishDiagnostics` handler.

use lsp_server::Connection;
use lsp_types::{PublishDiagnosticsParams, Uri, notification::PublishDiagnostics};

use crate::{convert::parse_error_to_diagnostic, state::FileData};

/// Re-publish diagnostics for `uri` based on the current [`FileData`].
///
/// This is called every time a file is opened or changed.
pub fn publish(conn: &Connection, uri: Uri, file: &FileData) {
    let diagnostics = file
        .errors
        .iter()
        .map(|e| parse_error_to_diagnostic(&file.text, e))
        .collect();

    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };

    let notification = lsp_server::Notification::new(
        <PublishDiagnostics as lsp_types::notification::Notification>::METHOD.into(),
        params,
    );

    // Errors here mean the client disconnected; nothing useful to do.
    let _ = conn
        .sender
        .send(lsp_server::Message::Notification(notification));
}

/// Clear diagnostics for `uri` (called on `didClose`).
pub fn clear(conn: &Connection, uri: Uri) {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics: vec![],
        version: None,
    };

    let notification = lsp_server::Notification::new(
        <PublishDiagnostics as lsp_types::notification::Notification>::METHOD.into(),
        params,
    );

    let _ = conn
        .sender
        .send(lsp_server::Message::Notification(notification));
}
