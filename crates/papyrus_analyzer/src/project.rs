//! `papyrus.toml` and the flags file.
//!
//! The manifest of the first workspace folder containing one (searching its
//! ancestors too) provides import directories, the flags file and the
//! compiler invocation. Its problems are published as diagnostics on the
//! manifest itself, live while it is edited; the project is reloaded, and the
//! workspace re-indexed when needed, once the manifest is saved.
//!
//! The flags file is, in order of preference: `[build] flags` of the manifest,
//! the `papyrus.flagsFile` setting, a `TESV_Papyrus_Flags.flg` found while
//! indexing, any other `.flg` found while indexing, or the built-in Skyrim
//! flags.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use lsp_types::{NumberOrString, PublishDiagnosticsParams, notification};
use papyrus_ide::{LineIndex, TextRange, TextSize};
use papyrus_project::{ManifestError, Project, Variables};

use crate::{
    conv::{self, Lines},
    ext::{CompileCommand, CompileCommandsParams, CompileCommandsResult},
    server::{Server, is_flags, is_manifest},
};

impl Server {
    /// The variables of this machine, with the editor values on top.
    pub(crate) fn vars(&self) -> Variables {
        Variables::new(self.config.var.clone()).with_workspace_folders(self.roots.clone())
    }

    /// Expands the variables of a path setting, relative to the first
    /// workspace folder.
    fn expand_setting(&self, value: &Path) -> Option<PathBuf> {
        let text = value.to_string_lossy();
        match self.vars().expand(&text, &mut BTreeMap::new()) {
            Ok(expanded) => {
                let root = self.roots.first().cloned().unwrap_or_default();
                Some(papyrus_project::resolve_path(&root, &expanded))
            }
            Err(err) => {
                log::warn!("setting `{text}`: {err}");
                None
            }
        }
    }

    /// Import directories: those of the manifest, then the configured ones.
    /// Without manifest, the game scripts of `SKYRIM_DIR` come last, so that
    /// a workspace resolves vanilla scripts with no configuration at all.
    pub(crate) fn import_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = self.project.as_ref().map(Project::import_dirs).unwrap_or_default();
        let configured =
            self.config.import_directories.iter().filter_map(|d| self.expand_setting(d));
        let game = match &self.project {
            None => self
                .skyrim_dir()
                .map(|(dir, _)| papyrus_project::game_script_dirs(&dir))
                .unwrap_or_default(),
            Some(_) => Vec::new(),
        };
        for dir in configured.chain(game) {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        dirs
    }

    /// The flags file setting, with variables expanded.
    pub(crate) fn flags_setting(&self) -> Option<PathBuf> {
        self.config.flags_file.as_deref().and_then(|f| self.expand_setting(f))
    }

    /// Known variables and their origin, for `papyrus/status`.
    pub(crate) fn variables_report(&self) -> BTreeMap<String, String> {
        let mut out: BTreeMap<String, String> = self
            .project
            .iter()
            .flat_map(|p| &p.variables)
            .map(|(k, (v, src))| (k.clone(), format!("{v} ({src})")))
            .collect();
        if self.project.is_none()
            && let Some((dir, src)) = self.skyrim_dir()
        {
            out.entry(papyrus_project::SKYRIM_DIR.to_owned())
                .or_insert(format!("{} ({src})", dir.display()));
        }
        out
    }

    /// `SKYRIM_DIR` for a workspace without manifest: the variable if set,
    /// else the detected installation.
    fn skyrim_dir(&self) -> Option<(PathBuf, String)> {
        if let Some((dir, src)) = self.vars().lookup(papyrus_project::SKYRIM_DIR) {
            return Some((PathBuf::from(dir), src.to_string()));
        }
        papyrus_project::detect_skyrim_dir().map(|d| (d, "detected".to_owned()))
    }

    /// The overall state shown by the client, with the problems: `error`,
    /// `warning`, `limited` (no `papyrus.toml`: navigation works, compiling
    /// does not) or `ok`.
    pub(crate) fn health(&self) -> (&'static str, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        if let Some(error) = &self.manifest_error {
            errors.push(format!("papyrus.toml: {error}"));
        }
        if let Some(project) = &self.project {
            warnings
                .extend(project.warnings.iter().map(|w| format!("papyrus.toml: {}", w.message)));
        }
        for dir in &self.config.import_directories {
            let text = dir.to_string_lossy();
            if let Err(err) = self.vars().expand(&text, &mut BTreeMap::new()) {
                errors.push(format!("papyrus.importDirectories `{text}`: {err}"));
            }
        }
        if !self.ws.flags().errors.is_empty() {
            warnings.push(format!(
                "{}: {} syntax errors",
                self.ws.flags().source_name(),
                self.ws.flags().errors.len()
            ));
        }
        let health = if !errors.is_empty() {
            "error"
        } else if !warnings.is_empty() {
            "warning"
        } else if self.manifest.is_none() {
            "limited"
        } else {
            "ok"
        };
        errors.extend(warnings);
        (health, errors)
    }

    /// Hover on a `${NAME}` reference or a `[var]` key of `papyrus.toml` or
    /// `papyrus.local.toml`: the resolved value and where it comes from.
    pub(crate) fn manifest_hover(
        &self,
        path: &Path,
        offset: TextSize,
    ) -> Option<(TextRange, String)> {
        let text = self.doc_or_disk(path)?;
        let (name, range) = papyrus_project::variable_at(&text, usize::from(offset))?;
        let manifest = path.with_file_name(papyrus_project::MANIFEST_NAME);
        let local_path = path.with_file_name(papyrus_project::LOCAL_MANIFEST_NAME);
        let (manifest_text, local_text) = if *path == manifest {
            (text.clone(), self.doc_or_disk(&local_path))
        } else {
            (self.doc_or_disk(&manifest).unwrap_or_default(), Some(text.clone()))
        };
        let vars = papyrus_project::manifest_variables(
            &manifest_text,
            local_text.as_deref(),
            &self.vars(),
        );
        let markdown = match vars.lookup(&name) {
            Some((value, source)) => format!("`${{{name}}}` = `{value}`\n\n*from {source}*"),
            None => match vars.expand(&format!("${{{name}}}"), &mut BTreeMap::new()) {
                Err(err) => format!("`${{{name}}}`: {err}"),
                Ok(value) => format!("`${{{name}}}` = `{value}`"),
            },
        };
        let range =
            TextRange::new(TextSize::new(range.start as u32), TextSize::new(range.end as u32));
        Some((range, markdown))
    }

    fn doc_or_disk(&self, path: &Path) -> Option<String> {
        match self.other_docs.get(path) {
            Some(doc) => Some(doc.text.clone()),
            None => std::fs::read_to_string(path).ok(),
        }
    }

    fn manifest_path(&self) -> Option<PathBuf> {
        self.roots.iter().find_map(|root| papyrus_project::find_manifest(root))
    }

    /// Loads the manifest, from the editor if it is open, and publishes its
    /// problems.
    pub(crate) fn load_project(&mut self) {
        let found = self.manifest_path();
        if found != self.manifest {
            self.manifest = found.clone();
            self.refresh_code_lenses();
        }
        self.manifest_error = None;
        let Some(path) = found else {
            self.project = None;
            return;
        };
        let Some(text) = self.doc_or_disk(&path) else {
            self.project = None;
            self.manifest_error = Some(format!("cannot read {}", path.display()));
            return;
        };
        let local = self.doc_or_disk(&path.with_file_name(papyrus_project::LOCAL_MANIFEST_NAME));
        let result = papyrus_project::parse(&path, &text, local.as_deref(), &self.vars());
        let problems = match &result {
            Ok(project) => project.warnings.clone(),
            Err(errors) => errors.clone(),
        };
        self.publish_manifest_diagnostics(&path, &text, &problems, result.is_err());
        match result {
            Ok(project) => {
                log::info!("loaded {}", path.display());
                self.project = Some(project);
            }
            Err(errors) => {
                log::warn!("invalid {}: {}", path.display(), errors[0]);
                self.manifest_error = Some(errors[0].message.clone());
                // Keep the previous project rather than losing all imports.
            }
        }
    }

    /// Asks the client to query code lenses again, e.g. after the manifest
    /// appeared or disappeared.
    pub(crate) fn refresh_code_lenses(&mut self) {
        let supported = self
            .caps
            .workspace
            .as_ref()
            .and_then(|w| w.code_lens.as_ref())
            .and_then(|c| c.refresh_support)
            .unwrap_or(false);
        if supported {
            self.request_client::<lsp_types::request::CodeLensRefresh>(());
        }
    }

    /// Reloads the manifest and re-indexes when its imports or flags changed.
    pub(crate) fn reload_project(&mut self) {
        let before = (self.import_dirs(), self.project.as_ref().and_then(|p| p.flags.clone()));
        self.load_project();
        let after = (self.import_dirs(), self.project.as_ref().and_then(|p| p.flags.clone()));
        if before != after {
            log::info!("project changed, re-indexing");
            self.reset_workspace();
            self.start_indexing();
        }
        self.publish_open_diagnostics();
    }

    fn publish_manifest_diagnostics(
        &self,
        path: &Path,
        text: &str,
        problems: &[ManifestError],
        fatal: bool,
    ) {
        let index = LineIndex::new(text);
        let lines = Lines { text: Arc::from(text), index: Arc::new(index) };
        let diagnostics = problems
            .iter()
            .map(|p| {
                let range = p.span.clone().map_or(TextRange::empty(TextSize::new(0)), |r| {
                    TextRange::new(TextSize::new(r.start as u32), TextSize::new(r.end as u32))
                });
                lsp_types::Diagnostic {
                    range: lines.range(range),
                    severity: Some(if fatal {
                        lsp_types::DiagnosticSeverity::ERROR
                    } else {
                        lsp_types::DiagnosticSeverity::WARNING
                    }),
                    code: Some(NumberOrString::String("manifest".to_owned())),
                    source: Some("papyrus".to_owned()),
                    message: p.message.clone(),
                    ..Default::default()
                }
            })
            .collect();
        self.notify::<notification::PublishDiagnostics>(PublishDiagnosticsParams {
            uri: conv::path_to_uri(path),
            diagnostics,
            version: self.other_docs.get(path).map(|d| d.version),
        });
    }

    /// A `.flg` or `papyrus.toml` document changed in the editor.
    pub(crate) fn other_doc_changed(&mut self, path: &Path) {
        let Some(doc) = self.other_docs.get(path) else { return };
        if is_manifest(path) {
            // Validate live; the project itself is reloaded on save.
            let manifest = path.with_file_name(papyrus_project::MANIFEST_NAME);
            if manifest != path {
                // `papyrus.local.toml`: problems are reported on the manifest.
                self.load_project();
                return;
            }
            let text = doc.text.clone();
            let local =
                self.doc_or_disk(&path.with_file_name(papyrus_project::LOCAL_MANIFEST_NAME));
            let result = papyrus_project::parse(path, &text, local.as_deref(), &self.vars());
            let (problems, fatal) = match &result {
                Ok(project) => (project.warnings.clone(), false),
                Err(errors) => (errors.clone(), true),
            };
            self.publish_manifest_diagnostics(path, &text, &problems, fatal);
        } else if is_flags(path) {
            // Only the active flags file (or any, without one) drives the scripts.
            if self.ws.is_flags_file(path) || self.ws.flags().path.is_none() {
                let text = Arc::from(doc.text.as_str());
                self.ws.set_flags_open(path.to_path_buf(), text);
                self.publish_flags_diagnostics();
                self.publish_open_diagnostics();
            }
        }
    }

    /// Chooses and loads the flags file. `found` are `.flg` files seen while
    /// indexing; empty to reload the current one.
    pub(crate) fn select_flags_file(&mut self, found: Vec<PathBuf>) {
        let preferred = found
            .iter()
            .find(|p| {
                p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("TESV_Papyrus_Flags.flg"))
            })
            .or(found.first())
            .cloned();
        let chosen = self
            .project
            .as_ref()
            .and_then(|p| p.flags.clone())
            .or_else(|| self.flags_setting())
            .or(preferred)
            .or_else(|| self.ws.flags().path.clone());
        let Some(path) = chosen else { return };
        if let Some(doc) = self.other_docs.get(&path) {
            let text = Arc::from(doc.text.as_str());
            self.ws.set_flags_open(path, text);
        } else if let Some(text) = papyrus_ide::read_file(&path) {
            self.ws.set_flags_file(path, Arc::from(text));
        } else {
            log::warn!("cannot read flags file {}", path.display());
            return;
        }
        log::info!(
            "flags: {} ({} flags)",
            self.ws.flags().source_name(),
            self.ws.flags().defs.len()
        );
        self.publish_flags_diagnostics();
    }

    fn publish_flags_diagnostics(&self) {
        let flags = self.ws.flags();
        let Some(path) = &flags.path else { return };
        let lines = Lines::flags(&self.ws);
        let diagnostics = papyrus_ide::flag_file_diagnostics(&self.ws)
            .into_iter()
            .map(|d| lsp_types::Diagnostic {
                range: lines.range(d.range),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(d.code.to_owned())),
                source: Some("papyrus".to_owned()),
                message: d.message,
                ..Default::default()
            })
            .collect();
        self.notify::<notification::PublishDiagnostics>(PublishDiagnosticsParams {
            uri: conv::path_to_uri(path),
            diagnostics,
            version: None,
        });
    }

    /// Handles `papyrus/compileCommands`.
    pub(crate) fn compile_commands(
        &mut self,
        params: CompileCommandsParams,
    ) -> Result<CompileCommandsResult, String> {
        // The manifest may have been edited since the last load.
        self.load_project();
        if let Some(error) = &self.manifest_error {
            return Err(format!("{} is invalid: {error}", papyrus_project::MANIFEST_NAME));
        }
        let project = self.project.as_ref().ok_or_else(|| {
            format!("no {} found in the workspace", papyrus_project::MANIFEST_NAME)
        })?;
        let mut docs = params.files;
        docs.extend(params.text_document);
        let commands = if params.all || docs.is_empty() {
            project.compile_all()?
        } else {
            docs.iter()
                .map(|doc| {
                    let path = conv::uri_to_path(&doc.uri).ok_or("not a file")?;
                    project.compile_file(&path)
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        Ok(CompileCommandsResult {
            manifest: project.manifest_path.display().to_string(),
            output: project.output.display().to_string(),
            commands: commands
                .into_iter()
                .map(|c| CompileCommand {
                    command_line: c.to_string(),
                    program: c.program.display().to_string(),
                    args: c.args,
                    cwd: c.cwd.display().to_string(),
                    output: c.output.display().to_string(),
                })
                .collect(),
        })
    }
}
