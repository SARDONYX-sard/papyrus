//! The server state and main loop.
//!
//! Everything runs on the main thread except workspace indexing, which happens
//! on a background thread and reports back through a channel. Requests are
//! answered synchronously: queries on a single file take microseconds to a few
//! milliseconds.
//!
//! # Restart
//!
//! An editor extension cannot restart itself once activated, so the server
//! implements the custom `papyrus/restart` request: it drops every cached index,
//! re-reads the settings sent with the request, re-indexes the workspace and
//! republishes diagnostics, while keeping the documents currently open in the
//! editor (the client does not send them again).

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use crossbeam_channel::{Receiver, Sender, select};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    ClientCapabilities, InitializeParams, NumberOrString, ProgressParams, ProgressParamsValue,
    PublishDiagnosticsParams, Uri, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressCreateParams, WorkDoneProgressEnd, WorkDoneProgressReport, notification,
    request,
};
use papyrus_ide::{FileId, Origin, ScriptIndex, Workspace};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    config::{CliOptions, Config},
    conv::{self, Lines},
};

/// Messages from the indexing thread.
pub(crate) enum IndexEvent {
    /// `done` of `total` files are indexed.
    Progress { generation: u64, done: usize, total: usize },
    /// Indexing finished. `flag_files` are the `.flg` files found on the way.
    Done {
        generation: u64,
        files: Vec<(PathBuf, ScriptIndex, Origin)>,
        flag_files: Vec<PathBuf>,
        elapsed_ms: u128,
    },
}

/// A document open in the editor that is not a script: the flags file or the
/// project manifest.
pub(crate) struct OtherDoc {
    pub(crate) text: String,
    pub(crate) version: i32,
}

/// The language server.
pub(crate) struct Server {
    sender: Sender<Message>,
    pub(crate) ws: Workspace,
    pub(crate) config: Config,
    cli: CliOptions,
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) caps: ClientCapabilities,
    /// The loaded `papyrus.toml`.
    pub(crate) project: Option<papyrus_project::Project>,
    /// The `papyrus.toml` found in the workspace, valid or not.
    pub(crate) manifest: Option<PathBuf>,
    /// Why `manifest` could not be loaded.
    pub(crate) manifest_error: Option<String>,
    /// Open `.flg` and `papyrus.toml` documents, by path.
    pub(crate) other_docs: rustc_hash::FxHashMap<PathBuf, OtherDoc>,
    /// Incremented on every (re)index; results of older runs are dropped.
    generation: u64,
    indexing: bool,
    index_tx: Sender<IndexEvent>,
    index_rx: Receiver<IndexEvent>,
    next_request_id: i32,
}

type HandlerResult = Result<Value, (ErrorCode, String)>;

impl Server {
    pub(crate) fn new(sender: Sender<Message>, init: &InitializeParams, cli: CliOptions) -> Server {
        let mut roots: Vec<PathBuf> = init
            .workspace_folders
            .iter()
            .flatten()
            .filter_map(|f| conv::uri_to_path(&f.uri))
            .collect();
        #[allow(deprecated)]
        if roots.is_empty() {
            roots.extend(init.root_uri.as_ref().and_then(conv::uri_to_path));
        }
        let (index_tx, index_rx) = crossbeam_channel::unbounded();
        Server {
            sender,
            ws: Workspace::new(),
            config: Config::from_json(init.initialization_options.clone()).with_cli(&cli),
            cli,
            roots,
            caps: init.capabilities.clone(),
            project: None,
            manifest: None,
            manifest_error: None,
            other_docs: Default::default(),
            generation: 0,
            indexing: false,
            index_tx,
            index_rx,
            next_request_id: 0,
        }
    }

    /// Runs until the client asks for shutdown.
    pub(crate) fn run(mut self, connection: &Connection) -> anyhow::Result<()> {
        self.load_project();
        self.start_indexing();
        loop {
            select! {
                recv(connection.receiver) -> msg => {
                    let Ok(msg) = msg else { return Ok(()) };
                    match msg {
                        Message::Request(req) => {
                            if connection.handle_shutdown(&req)? {
                                return Ok(());
                            }
                            self.on_request(req);
                        }
                        Message::Notification(not) => self.on_notification(not),
                        Message::Response(_) => {}
                    }
                }
                recv(self.index_rx) -> event => {
                    if let Ok(event) = event {
                        self.on_index_event(event);
                    }
                }
            }
        }
    }

    // === Messaging ===

    pub(crate) fn send(&self, msg: impl Into<Message>) {
        if self.sender.send(msg.into()).is_err() {
            log::error!("client connection closed");
        }
    }

    pub(crate) fn notify<N: notification::Notification>(&self, params: N::Params) {
        self.send(Notification::new(N::METHOD.to_owned(), params));
    }

    pub(crate) fn request_client<R: request::Request>(&mut self, params: R::Params) {
        self.next_request_id += 1;
        let id = RequestId::from(format!("papyrus-{}", self.next_request_id));
        self.send(Request::new(id, R::METHOD.to_owned(), params));
    }

    // === Requests ===

    fn on_request(&mut self, req: Request) {
        let Request { id, method, params } = req;
        let start = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| self.dispatch(&method, params)))
            .unwrap_or_else(|panic| {
                let msg = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                log::error!("request `{method}` panicked: {msg}");
                Err((ErrorCode::InternalError, format!("internal error: {msg}")))
            });
        log::debug!("{method} took {:?}", start.elapsed());
        let response = match result {
            Ok(value) => Response::new_ok(id, value),
            Err((code, message)) => Response::new_err(id, code as i32, message),
        };
        self.send(response);
    }

    fn dispatch(&mut self, method: &str, params: Value) -> HandlerResult {
        use lsp_types::request::*;
        match method {
            HoverRequest::METHOD => handle::<HoverRequest>(self, params, Server::hover),
            request::CodeActionRequest::METHOD => {
                handle::<request::CodeActionRequest>(self, params, Server::code_actions)
            }
            request::GotoDeclaration::METHOD => {
                handle::<request::GotoDeclaration>(self, params, Server::goto_declaration)
            }
            GotoDefinition::METHOD => {
                handle::<GotoDefinition>(self, params, Server::goto_definition)
            }
            References::METHOD => handle::<References>(self, params, Server::references),
            DocumentHighlightRequest::METHOD => {
                handle::<DocumentHighlightRequest>(self, params, Server::document_highlight)
            }
            Completion::METHOD => handle::<Completion>(self, params, Server::completion),
            ResolveCompletionItem::METHOD => {
                handle::<ResolveCompletionItem>(self, params, Server::resolve_completion)
            }
            SignatureHelpRequest::METHOD => {
                handle::<SignatureHelpRequest>(self, params, Server::signature_help)
            }
            DocumentSymbolRequest::METHOD => {
                handle::<DocumentSymbolRequest>(self, params, Server::document_symbols)
            }
            WorkspaceSymbolRequest::METHOD => {
                handle::<WorkspaceSymbolRequest>(self, params, Server::workspace_symbols)
            }
            CodeLensRequest::METHOD => handle::<CodeLensRequest>(self, params, Server::code_lens),
            Formatting::METHOD => handle::<Formatting>(self, params, Server::formatting),
            InlayHintRequest::METHOD => {
                handle::<InlayHintRequest>(self, params, Server::inlay_hints)
            }
            FoldingRangeRequest::METHOD => {
                handle::<FoldingRangeRequest>(self, params, Server::folding_ranges)
            }
            SemanticTokensFullRequest::METHOD => {
                handle::<SemanticTokensFullRequest>(self, params, Server::semantic_tokens_full)
            }
            SemanticTokensRangeRequest::METHOD => {
                handle::<SemanticTokensRangeRequest>(self, params, Server::semantic_tokens_range)
            }
            crate::ext::Restart::METHOD => {
                handle::<crate::ext::Restart>(self, params, Server::restart)
            }
            crate::ext::SyntaxTree::METHOD => {
                handle::<crate::ext::SyntaxTree>(self, params, Server::syntax_tree)
            }
            crate::ext::Status::METHOD => {
                handle::<crate::ext::Status>(self, params, Server::status)
            }
            crate::ext::ManifestTemplate::METHOD => {
                handle::<crate::ext::ManifestTemplate>(self, params, |_, ()| {
                    papyrus_project::TEMPLATE.to_owned()
                })
            }
            crate::ext::Decompile::METHOD => {
                let params = serde_json::from_value(params)
                    .map_err(|e| (ErrorCode::InvalidParams, format!("invalid params: {e}")))?;
                let text = self.decompile(params).map_err(|m| (ErrorCode::RequestFailed, m))?;
                Ok(Value::String(text))
            }
            crate::ext::DecompileFiles::METHOD => {
                handle::<crate::ext::DecompileFiles>(self, params, Server::decompile_files)
            }
            crate::ext::CompileCommands::METHOD => {
                let params = serde_json::from_value(params)
                    .map_err(|e| (ErrorCode::InvalidParams, format!("invalid params: {e}")))?;
                let result = self
                    .compile_commands(params)
                    .map_err(|message| (ErrorCode::RequestFailed, message))?;
                serde_json::to_value(result).map_err(|e| (ErrorCode::InternalError, e.to_string()))
            }
            _ => Err((ErrorCode::MethodNotFound, format!("unknown request `{method}`"))),
        }
    }

    // === Notifications ===

    fn on_notification(&mut self, not: Notification) {
        use lsp_types::notification::*;
        let method = not.method.clone();
        let result = catch_unwind(AssertUnwindSafe(|| match not.method.as_str() {
            DidOpenTextDocument::METHOD => {
                notify::<DidOpenTextDocument>(not, |p| self.did_open(p));
            }
            DidChangeTextDocument::METHOD => {
                notify::<DidChangeTextDocument>(not, |p| self.did_change(p));
            }
            DidCloseTextDocument::METHOD => {
                notify::<DidCloseTextDocument>(not, |p| self.did_close(p));
            }
            DidSaveTextDocument::METHOD => {
                notify::<DidSaveTextDocument>(not, |p| self.did_save(p));
            }
            DidChangeWatchedFiles::METHOD => {
                notify::<DidChangeWatchedFiles>(not, |p| self.did_change_watched_files(p));
            }
            DidChangeConfiguration::METHOD => {
                notify::<DidChangeConfiguration>(not, |p| {
                    let config = Config::from_json(Some(p.settings)).with_cli(&self.cli);
                    self.set_config(config);
                });
            }
            _ => {}
        }));
        if result.is_err() {
            log::error!("notification `{method}` panicked");
        }
    }

    fn did_open(&mut self, params: lsp_types::DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let Some(path) = conv::uri_to_path(&doc.uri) else { return };
        if !is_script(&path) {
            self.other_docs.insert(path.clone(), OtherDoc { text: doc.text, version: doc.version });
            self.other_doc_changed(&path);
            return;
        }
        self.ws.set_open(path, Arc::from(doc.text), doc.version);
        self.publish_open_diagnostics();
    }

    fn did_change(&mut self, params: lsp_types::DidChangeTextDocumentParams) {
        let Some(path) = conv::uri_to_path(&params.text_document.uri) else { return };
        let version = params.text_document.version;
        if let Some(doc) = self.other_docs.get_mut(&path) {
            apply_changes(&mut doc.text, params.content_changes);
            doc.version = version;
            self.other_doc_changed(&path);
            return;
        }
        let Some(file) = self.ws.file_id(&path) else { return };
        let Some(doc) = &self.ws.file(file).open else { return };

        let mut text = doc.text.to_string();
        apply_changes(&mut text, params.content_changes);
        self.ws.set_open(path, Arc::from(text), version);
        self.publish_open_diagnostics();
    }

    fn did_save(&mut self, params: lsp_types::DidSaveTextDocumentParams) {
        let Some(path) = conv::uri_to_path(&params.text_document.uri) else { return };
        if is_manifest(&path) {
            self.reload_project();
        }
    }

    fn did_close(&mut self, params: lsp_types::DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let path = conv::uri_to_path(&uri);
        if let Some(path) = &path
            && self.other_docs.remove(path).is_some()
        {
            if self.ws.is_flags_file(path) {
                self.ws.close_flags();
            }
            if is_manifest(path) {
                // Keep showing problems of the file on disk.
                self.reload_project();
                return;
            }
        } else if let Some(file) = path.and_then(|p| self.ws.file_id(&p)) {
            self.ws.close(file);
        }
        self.notify::<notification::PublishDiagnostics>(PublishDiagnosticsParams {
            uri,
            diagnostics: Vec::new(),
            version: None,
        });
        self.publish_open_diagnostics();
    }

    fn did_change_watched_files(&mut self, params: lsp_types::DidChangeWatchedFilesParams) {
        for change in params.changes {
            let Some(path) = conv::uri_to_path(&change.uri) else { continue };
            if is_manifest(&path) {
                if !self.other_docs.contains_key(&path) {
                    self.reload_project();
                }
                continue;
            }
            if is_flags(&path) {
                if self.ws.is_flags_file(&path) && !self.other_docs.contains_key(&path) {
                    self.select_flags_file(Vec::new());
                }
                continue;
            }
            if !is_script(&path) {
                continue;
            }
            let existing = self.ws.file_id(&path);
            if existing.is_some_and(|f| self.ws.file(f).open.is_some()) {
                // The editor contents are authoritative.
                continue;
            }
            match papyrus_ide::read_file(&path) {
                Some(text) => {
                    let origin = self.origin_of(&path);
                    self.ws.set_disk_index(path, papyrus_ide::index_text(&text), origin);
                }
                None => {
                    if let Some(file) = existing {
                        self.ws.remove(file);
                    }
                }
            }
        }
        self.publish_open_diagnostics();
    }

    fn set_config(&mut self, config: Config) {
        if config == self.config {
            return;
        }
        let reload = config.import_directories != self.config.import_directories
            || config.flags_file != self.config.flags_file
            || config.var != self.config.var;
        let refresh_hints = config.inlay_hints != self.config.inlay_hints;
        self.config = config;
        if refresh_hints
            && self
                .caps
                .workspace
                .as_ref()
                .and_then(|w| w.inlay_hint.as_ref())
                .and_then(|i| i.refresh_support)
                .unwrap_or(false)
        {
            self.request_client::<request::InlayHintRefreshRequest>(());
        }
        if reload {
            // Variables may change the paths of papyrus.toml too.
            self.load_project();
            self.reset_workspace();
            self.start_indexing();
        }
        self.publish_open_diagnostics();
    }

    fn origin_of(&self, path: &Path) -> Origin {
        let in_root = self.roots.iter().any(|r| path.starts_with(r));
        match self.import_dirs().iter().position(|d| path.starts_with(d)) {
            Some(i) if !in_root => Origin::Import(i as u16),
            _ => Origin::Workspace,
        }
    }

    // === Restart ===

    /// Drops all indexes, keeping the open documents.
    pub(crate) fn reset_workspace(&mut self) {
        let open: Vec<(PathBuf, Arc<str>, i32)> = self
            .ws
            .open_files()
            .filter_map(|(_, f)| {
                f.open.as_ref().map(|d| (f.path.clone(), d.text.clone(), d.version))
            })
            .collect();
        self.ws = Workspace::new();
        for (path, text, version) in open {
            self.ws.set_open(path, text, version);
        }
        let open_flags: Vec<(PathBuf, String)> = self
            .other_docs
            .iter()
            .filter(|(p, _)| is_flags(p))
            .map(|(p, d)| (p.clone(), d.text.clone()))
            .collect();
        for (path, text) in open_flags {
            self.ws.set_flags_open(path, Arc::from(text));
        }
    }

    /// Handles `papyrus/restart`.
    fn restart(&mut self, params: Option<crate::ext::RestartParams>) -> crate::ext::StatusResult {
        log::info!("restarting the server");
        if let Some(settings) = params.and_then(|p| p.settings) {
            self.config = Config::from_json(Some(settings)).with_cli(&self.cli);
        }
        self.load_project();
        self.reset_workspace();
        self.start_indexing();
        self.publish_open_diagnostics();
        self.refresh_code_lenses();
        self.status(())
    }

    fn status(&mut self, _: ()) -> crate::ext::StatusResult {
        let (health, problems) = self.health();
        crate::ext::StatusResult {
            health: health.to_owned(),
            problems,
            files: self.ws.len(),
            open_files: self.ws.open_files().count(),
            indexing: self.indexing,
            generation: self.generation,
            roots: self.roots.iter().map(|p| p.display().to_string()).collect(),
            import_directories: self
                .import_dirs()
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            manifest: self.manifest.as_ref().map(|p| p.display().to_string()),
            manifest_error: self.manifest_error.clone(),
            flags_file: self.ws.flags().path.as_ref().map(|p| p.display().to_string()),
            variables: self.variables_report(),
            build_dirs: self.build_dirs(),
            sources: self
                .project
                .iter()
                .flat_map(|p| &p.sources)
                .map(|p| p.display().to_string())
                .collect(),
        }
    }

    // === Indexing ===

    pub(crate) fn start_indexing(&mut self) {
        self.generation += 1;
        self.indexing = true;
        let generation = self.generation;
        let mut dirs: Vec<(PathBuf, Origin)> =
            self.roots.iter().map(|r| (r.clone(), Origin::Workspace)).collect();
        dirs.extend(
            self.import_dirs()
                .into_iter()
                .enumerate()
                // Directories inside a workspace folder are already scanned.
                .filter(|(_, d)| !self.roots.iter().any(|r| d.starts_with(r)))
                .map(|(i, d)| (d, Origin::Import(i as u16))),
        );
        for (dir, _) in &dirs {
            if !dir.is_dir() {
                log::warn!("not a directory: {}", dir.display());
            }
        }

        if self.progress_supported() {
            let token = progress_token(generation);
            self.request_client::<request::WorkDoneProgressCreate>(WorkDoneProgressCreateParams {
                token: token.clone(),
            });
            self.notify::<notification::Progress>(ProgressParams {
                token,
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: "Indexing Papyrus scripts".to_owned(),
                        cancellable: Some(false),
                        message: None,
                        percentage: Some(0),
                    },
                )),
            });
        }

        // Build outputs hold copies of scripts (decompiled ones): not sources.
        let skipped: Vec<PathBuf> = self
            .build_dirs()
            .into_iter()
            .flat_map(|b| [PathBuf::from(b.compile), PathBuf::from(b.decompile)])
            .chain(self.roots.iter().map(|r| papyrus_project::default_build_dir(r)))
            .collect();
        let tx = self.index_tx.clone();
        let spawned = std::thread::Builder::new()
            .name("papyrus-indexer".to_owned())
            .spawn(move || index_workspace(generation, &dirs, &skipped, &tx));
        if let Err(err) = spawned {
            log::error!("cannot spawn the indexer: {err}");
            self.indexing = false;
        }
    }

    fn on_index_event(&mut self, event: IndexEvent) {
        match event {
            IndexEvent::Progress { generation, done, total } => {
                if generation != self.generation || !self.progress_supported() {
                    return;
                }
                self.notify::<notification::Progress>(ProgressParams {
                    token: progress_token(generation),
                    value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                        WorkDoneProgressReport {
                            cancellable: None,
                            message: Some(format!("{done}/{total}")),
                            percentage: Some((done * 100 / total.max(1)) as u32),
                        },
                    )),
                });
            }
            IndexEvent::Done { generation, files, flag_files, elapsed_ms } => {
                if generation != self.generation {
                    return;
                }
                let count = files.len();
                for (path, index, origin) in files {
                    self.ws.set_disk_index(path, index, origin);
                }
                self.select_flags_file(flag_files);
                self.indexing = false;
                log::info!("indexed {count} scripts in {elapsed_ms} ms");
                if self.progress_supported() {
                    self.notify::<notification::Progress>(ProgressParams {
                        token: progress_token(generation),
                        value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(
                            WorkDoneProgressEnd { message: Some(format!("{count} scripts")) },
                        )),
                    });
                }
                self.publish_open_diagnostics();
            }
        }
    }

    fn progress_supported(&self) -> bool {
        self.caps.window.as_ref().and_then(|w| w.work_done_progress).unwrap_or(false)
    }

    // === Diagnostics ===

    pub(crate) fn publish_open_diagnostics(&self) {
        let open: Vec<FileId> = self.ws.open_files().map(|(id, _)| id).collect();
        for file in open {
            self.publish_diagnostics(file);
        }
    }

    fn publish_diagnostics(&self, file: FileId) {
        let data = self.ws.file(file);
        let Some(doc) = &data.open else { return };
        let Some(lines) = Lines::of(&self.ws, file) else { return };
        let diagnostics = papyrus_ide::diagnostics(&self.ws, file)
            .into_iter()
            .filter(|d| self.config.semantic_diagnostics || !d.needs_workspace())
            .chain(papyrus_lint::lint(&self.ws, file, &self.lint_config()))
            .map(|d| lsp_types::Diagnostic {
                range: lines.range(d.range),
                severity: Some(match d.severity {
                    papyrus_ide::Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
                    papyrus_ide::Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                }),
                code: Some(NumberOrString::String(d.code.to_owned())),
                source: Some("papyrus".to_owned()),
                message: d.message,
                ..Default::default()
            })
            .collect();
        self.notify::<notification::PublishDiagnostics>(PublishDiagnosticsParams {
            uri: conv::path_to_uri(&data.path),
            diagnostics,
            version: Some(doc.version),
        });
    }

    /// Resolves a document URI to a known file.
    pub(crate) fn file(&self, uri: &Uri) -> Result<FileId, (ErrorCode, String)> {
        conv::uri_to_path(uri).and_then(|p| self.ws.file_id(&p)).ok_or_else(|| {
            (ErrorCode::InvalidParams, format!("unknown document: {}", uri.as_str()))
        })
    }
}

fn progress_token(generation: u64) -> NumberOrString {
    NumberOrString::String(format!("papyrus/indexing/{generation}"))
}

fn handle<R>(
    server: &mut Server,
    params: Value,
    f: impl FnOnce(&mut Server, R::Params) -> R::Result,
) -> HandlerResult
where
    R: request::Request,
    R::Params: DeserializeOwned,
    R::Result: Serialize,
{
    let params: R::Params = serde_json::from_value(params)
        .map_err(|e| (ErrorCode::InvalidParams, format!("invalid params: {e}")))?;
    serde_json::to_value(f(server, params)).map_err(|e| (ErrorCode::InternalError, e.to_string()))
}

fn notify<N>(not: Notification, f: impl FnOnce(N::Params))
where
    N: notification::Notification,
    N::Params: DeserializeOwned,
{
    match serde_json::from_value(not.params) {
        Ok(params) => f(params),
        Err(err) => log::warn!("invalid `{}` params: {err}", N::METHOD),
    }
}

pub(crate) fn is_script(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("psc"))
}

pub(crate) fn is_flags(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("flg"))
}

/// `papyrus.toml` or its machine local `papyrus.local.toml`.
pub(crate) fn is_manifest(path: &Path) -> bool {
    path.file_name().is_some_and(|n| {
        n.eq_ignore_ascii_case(papyrus_project::MANIFEST_NAME)
            || n.eq_ignore_ascii_case(papyrus_project::LOCAL_MANIFEST_NAME)
    })
}

/// Applies editor changes to `text`.
fn apply_changes(text: &mut String, changes: Vec<lsp_types::TextDocumentContentChangeEvent>) {
    for change in changes {
        match change.range {
            Some(range) => {
                // Positions refer to the text after the previous change.
                let lines = Lines {
                    index: Arc::new(papyrus_ide::LineIndex::new(text)),
                    text: Arc::from(text.as_str()),
                };
                let range = lines.text_range(range);
                text.replace_range(
                    usize::from(range.start())..usize::from(range.end()),
                    &change.text,
                );
            }
            None => *text = change.text,
        }
    }
}

/// Finds and indexes every script below `dirs`, in parallel.
fn index_workspace(
    generation: u64,
    dirs: &[(PathBuf, Origin)],
    skipped: &[PathBuf],
    tx: &Sender<IndexEvent>,
) {
    let start = Instant::now();
    let mut paths = Vec::new();
    let mut flag_files = Vec::new();
    for (dir, origin) in dirs {
        collect_scripts(dir, *origin, skipped, &mut paths, &mut flag_files);
    }
    let total = paths.len();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = total.div_ceil(threads).max(1);
    let done = std::sync::atomic::AtomicUsize::new(0);

    let files: Vec<(PathBuf, ScriptIndex, Origin)> = std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|chunk| {
                let done = &done;
                s.spawn(move || {
                    let mut out = Vec::with_capacity(chunk.len());
                    for (path, origin) in chunk {
                        if let Some(text) = papyrus_ide::read_file(path) {
                            out.push((path.clone(), papyrus_ide::index_text(&text), *origin));
                        }
                        let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if n.is_multiple_of(500) {
                            let _ = tx.send(IndexEvent::Progress { generation, done: n, total });
                        }
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });

    let elapsed_ms = start.elapsed().as_millis();
    let _ = tx.send(IndexEvent::Done { generation, files, flag_files, elapsed_ms });
}

fn collect_scripts(
    dir: &Path,
    origin: Origin,
    skipped: &[PathBuf],
    out: &mut Vec<(PathBuf, Origin)>,
    flag_files: &mut Vec<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ty) = entry.file_type() else { continue };
        if ty.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || skipped.contains(&path)
            {
                continue;
            }
            collect_scripts(&path, origin, skipped, out, flag_files);
        } else if is_script(&path) {
            out.push((path, origin));
        } else if is_flags(&path) {
            flag_files.push(path);
        }
    }
}
