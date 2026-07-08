//! Editor state: open files, their parse results, and incremental re-parsing.

use std::collections::HashMap;

use lsp_types::Uri;
use papyrus_cst::parser::ParseError;
use papyrus_cst::{cst::Tree, parse_papyrus};

/// All data derived from a single open file.
pub struct FileData {
    /// Current source text (kept in sync with the editor via
    /// `textDocument/didOpen` and `textDocument/didChange`).
    pub text: String,
    /// Most recent parse tree.
    pub tree: Tree,
    /// Parse errors from the most recent parse.
    pub errors: Vec<ParseError>,
}

impl FileData {
    /// Parse `text` and return a new [`FileData`].
    pub fn new(text: String) -> Self {
        let (tree, errors) = parse_papyrus(&text);
        Self { text, tree, errors }
    }

    /// Replace the source text and re-parse.
    pub fn update(&mut self, text: String) {
        let (tree, errors) = parse_papyrus(&text);
        self.text = text;
        self.tree = tree;
        self.errors = errors;
    }
}

/// The global server state.
///
/// Every open document is represented by one [`FileData`] entry, keyed by
/// its canonical URI.  Files that are referenced via `Extends` or `Import`
/// but not explicitly opened by the editor are not tracked here; semantic
/// analysis (added later) will load them on demand.
#[derive(Default)]
pub struct State {
    files: HashMap<Uri, FileData>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called on `textDocument/didOpen`.
    pub fn open(&mut self, uri: Uri, text: String) {
        self.files.insert(uri, FileData::new(text));
    }

    /// Called on `textDocument/didChange` (full-text sync).
    pub fn change(&mut self, uri: &Uri, text: String) {
        match self.files.get_mut(uri) {
            Some(f) => f.update(text),
            None => {
                self.files.insert(uri.clone(), FileData::new(text));
            }
        }
    }

    /// Called on `textDocument/didClose`.
    pub fn close(&mut self, uri: &Uri) {
        self.files.remove(uri);
    }

    /// Returns the [`FileData`] for `uri`, if the file is open.
    pub fn get(&self, uri: &Uri) -> Option<&FileData> {
        self.files.get(uri)
    }
}
