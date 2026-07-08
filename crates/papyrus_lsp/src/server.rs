//! LSP message loop.
//!
//! Follows the `lsp-server` crate's recommended pattern:
//! initialize → main loop → shutdown.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    InitializeParams, OneOf, SaveOptions, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    },
    request::{Formatting, Request as _},
};
use papyrus_fmt::FormatOptions;

use crate::{handlers, state::State};

pub fn run(connection: Connection) -> anyhow::Result<()> {
    // ── Initialization handshake ──────────────────────────────────────────────
    let server_capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                // Full-text sync: the client sends the entire file on every
                // change.  Incremental sync (sending only the changed ranges)
                // can be added later.
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(lsp_types::TextDocumentSyncSaveOptions::SaveOptions(
                    SaveOptions {
                        include_text: Some(true),
                    },
                )),
                ..Default::default()
            },
        )),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })?;

    let _init_params: InitializeParams =
        serde_json::from_value(connection.initialize(server_capabilities)?)?;

    log::info!("papyrus-lsp initialized");

    // ── Main loop ─────────────────────────────────────────────────────────────
    let mut state = State::new();
    let fmt_options = FormatOptions::default();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(&connection, &state, req, &fmt_options);
            }
            Message::Notification(notif) => {
                handle_notification(&connection, &mut state, notif);
            }
            Message::Response(_) => {
                // We don't send requests to the client yet.
            }
        }
    }

    Ok(())
}

// ── Request handlers ──────────────────────────────────────────────────────────

fn handle_request(conn: &Connection, state: &State, req: Request, fmt_options: &FormatOptions) {
    if let Ok((id, params)) = req.extract::<lsp_types::DocumentFormattingParams>(Formatting::METHOD)
    {
        let uri = &params.text_document.uri;
        let edits = match state.get(uri) {
            Some(file) => handlers::fmt::formatting(file, fmt_options),
            None => vec![],
        };
        respond(conn, id, edits);
    }

    // Unknown request — send an empty error response so the client doesn't hang.
}

// ── Notification handlers ─────────────────────────────────────────────────────

fn handle_notification(conn: &Connection, state: &mut State, notify: Notification) {
    if let Ok(params) = notify
        .clone()
        .extract::<lsp_types::DidOpenTextDocumentParams>(DidOpenTextDocument::METHOD)
    {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        state.open(uri.clone(), text);
        if let Some(file) = state.get(&uri) {
            handlers::diagnostics::publish(conn, uri, file);
        }
        return;
    }

    if let Ok(params) = notify
        .clone()
        .extract::<lsp_types::DidChangeTextDocumentParams>(DidChangeTextDocument::METHOD)
    {
        let uri = params.text_document.uri;
        // Full-text sync: exactly one change containing the whole document.
        if let Some(change) = params.content_changes.into_iter().next() {
            state.change(&uri, change.text);
        }
        if let Some(file) = state.get(&uri) {
            handlers::diagnostics::publish(conn, uri, file);
        }
        return;
    }

    if let Ok(params) =
        notify.extract::<lsp_types::DidCloseTextDocumentParams>(DidCloseTextDocument::METHOD)
    {
        let uri = params.text_document.uri;
        state.close(&uri);
        handlers::diagnostics::clear(conn, uri);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn respond<T: serde::Serialize>(conn: &Connection, id: RequestId, result: T) {
    let response = Response::new_ok(id, result);
    let _ = conn.sender.send(Message::Response(response));
}
