//! `textDocument/formatting` handler.

use lsp_types::{Position, Range, TextEdit};
use papyrus_fmt::{FormatOptions, format};

use crate::state::FileData;

/// Produce a list of [`TextEdit`]s that replace the entire file with the
/// formatted version.
///
/// We use a single whole-file replacement rather than a diff because
/// computing a minimal diff is complex and the LSP client can handle the
/// full replacement efficiently.
pub fn formatting(file: &FileData, options: &FormatOptions) -> Vec<TextEdit> {
    // If there's an error, We can't format it, so We'll ignore it.
    if !file.errors.is_empty() {
        return vec![];
    }

    let formatted = format(&file.text, &file.tree, options);

    // Only emit an edit when the output actually differs.
    if formatted == file.text {
        return vec![];
    }

    // Replace the entire document.
    let end = end_of_file(&file.text);

    vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end,
        },
        new_text: formatted,
    }]
}

/// Returns the [`Position`] just past the last character in `src`.
fn end_of_file(src: &str) -> Position {
    let mut line = 0u32;
    let mut last_line_start = 0usize;

    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            line += 1;
            last_line_start = i + 1;
        }
    }

    let character = src[last_line_start..]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();

    Position { line, character }
}
