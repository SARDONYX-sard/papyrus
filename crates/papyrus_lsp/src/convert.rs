//! Conversions between papyrus types and LSP types.

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use papyrus_cst::parser::ParseError;

/// Convert a [`ParseError`] to an LSP [`Diagnostic`].
///
/// `src` is the source text of the file; it is used to convert byte offsets
/// to LSP line/character positions.
pub fn parse_error_to_diagnostic(src: &str, error: &ParseError) -> Diagnostic {
    let range = span_to_range(src, error.span.start as usize, error.span.end as usize);

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("papyrus".into()),
        message: error.message.clone(),
        ..Default::default()
    }
}

/// Convert a byte-offset range to an LSP [`Range`].
///
/// LSP uses 0-based line/character (UTF-16 code unit) positions.
/// Papyrus source is ASCII-dominant so we treat each byte as one character
/// for simplicity; a future revision can add proper UTF-16 handling.
pub fn span_to_range(src: &str, start: usize, end: usize) -> Range {
    Range {
        start: offset_to_position(src, start),
        end: offset_to_position(src, end),
    }
}

/// Convert a byte offset to an LSP [`Position`].
pub fn offset_to_position(src: &str, offset: usize) -> Position {
    let offset = offset.min(src.len());
    let line = src[..offset].bytes().filter(|&b| b == b'\n').count();
    let line_start = src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    // LSP character offsets are UTF-16 code units.
    let character = src[line_start..offset]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();
    Position {
        line: line as u32,
        character,
    }
}

/// Convert an LSP [`Position`] to a byte offset in `src`.
#[expect(unused)]
pub fn position_to_offset(src: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut offset = 0usize;

    for (i, b) in src.bytes().enumerate() {
        if line == pos.line {
            // Walk UTF-16 characters to find the column.
            let col: u32 = src[offset..]
                .chars()
                .scan(0u32, |acc, c| {
                    let prev = *acc;
                    *acc += c.len_utf16() as u32;
                    Some(prev)
                })
                .take_while(|&col| col < pos.character)
                .count() as u32;
            // Advance offset by that many bytes.
            return offset
                + src[offset..]
                    .char_indices()
                    .nth(col as usize)
                    .map(|(i, _)| i)
                    .unwrap_or(src.len() - offset);
        }
        if b == b'\n' {
            line += 1;
            offset = i + 1;
        }
    }

    src.len()
}
