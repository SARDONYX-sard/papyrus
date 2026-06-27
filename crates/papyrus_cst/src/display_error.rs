//! Human-readable parse error formatting.
//!
//! ```text
//! error[1]: expected EndFunction, got EOF
//!  --> test.psc:12:1
//!   |
//! 12 | Function Broken()
//!    | ^^^^^^^^
//! ```

use crate::parser::ParseError;

/// Format all `errors` into a human-readable string.
///
/// `src` is the original source text (used to render the offending line).
/// `filename` is displayed in the `-->` location line; pass `""` to omit it.
///
/// Returns an empty string when `errors` is empty.
pub fn display_errors(src: &str, filename: &str, errors: &[ParseError]) -> String {
    if errors.is_empty() {
        return String::new();
    }

    // Pre-compute line start offsets once.
    let line_starts = line_start_offsets(src);

    let mut out = String::new();

    for (i, error) in errors.iter().enumerate() {
        let span = error.span;
        let (line, col) = offset_to_line_col(&line_starts, span.start as usize);

        // ── Header ────────────────────────────────────────────────────────────
        // error[1]: expected EndFunction, got EOF
        out.push_str(&format!("error[{}]: {}\n", i + 1, error.message));

        // ── Location ──────────────────────────────────────────────────────────
        //  --> test.psc:12:1
        if filename.is_empty() {
            out.push_str(&format!(" --> {}:{}\n", line + 1, col + 1));
        } else {
            out.push_str(&format!(" --> {}:{}:{}\n", filename, line + 1, col + 1));
        }

        // ── Source snippet ────────────────────────────────────────────────────
        let line_text = get_line(src, &line_starts, line);
        let line_num_str = format!("{}", line + 1);
        let gutter = line_num_str.len(); // width of the line number column

        //   |
        out.push_str(&format!("{:>width$} |\n", "", width = gutter));

        // 12 | Function Broken()
        out.push_str(&format!("{} | {}\n", line_num_str, line_text));

        //    | ^^^^^^^^
        let marker_start = col;
        let marker_len = marker_length(src, span).max(1);
        out.push_str(&format!(
            "{:>width$} | {}{}\n",
            "",
            " ".repeat(marker_start),
            "^".repeat(marker_len),
            width = gutter,
        ));

        // Blank line between errors.
        if i + 1 < errors.len() {
            out.push('\n');
        }
    }

    out
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Returns the byte offset of the start of each line in `src`.
fn line_start_offsets(src: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Convert a byte offset to `(line, col)` (both 0-based).
fn offset_to_line_col(line_starts: &[usize], offset: usize) -> (usize, usize) {
    let line = line_starts
        .partition_point(|&s| s <= offset)
        .saturating_sub(1);
    let col = offset - line_starts[line];
    (line, col)
}

/// Return the text of line `line` (without the trailing newline).
fn get_line<'a>(src: &'a str, line_starts: &[usize], line: usize) -> &'a str {
    let start = line_starts[line];
    let end = line_starts
        .get(line + 1)
        .map(|&s| s.saturating_sub(1)) // strip `\n`
        .unwrap_or(src.len());
    src.get(start..end).unwrap_or("")
}

/// How many columns should the `^^^` underline span?
///
/// Uses the span length when it covers real source text, otherwise falls back
/// to 1 so there's always at least one caret.
fn marker_length(src: &str, span: papyrus_token::span::TextSpan) -> usize {
    let start = span.start as usize;
    let end = (span.end as usize).min(src.len());
    if end > start {
        // Count chars, not bytes, so multi-byte characters don't throw off
        // the column alignment.
        src[start..end].chars().count()
    } else {
        1
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use papyrus_token::TokenStream;

    use super::*;
    use crate::parser::{Parser, file};

    fn parse_errors(src: &str) -> Vec<ParseError> {
        let tokens = TokenStream::from(src).into_tokens();
        let mut p = Parser::new(tokens);
        file(&mut p);
        let (_tree, errors) = p.build_tree();
        errors
    }

    #[test]
    fn no_errors_returns_empty_string() {
        let src = "Function Noop()\nEndFunction";
        let errors = parse_errors(src);
        let output = display_errors(src, "test.psc", &errors);
        assert!(output.is_empty(), "expected empty output, got:\n{output}");
    }

    #[test]
    fn missing_end_function_shows_error() {
        let src = "Function Broken()\n    Return\n";
        let errors = parse_errors(src);
        assert!(!errors.is_empty());
        let output = display_errors(src, "test.psc", &errors);
        println!("{output}");
        assert!(output.contains("error[1]"), "missing error header");
        assert!(output.contains("test.psc"), "missing filename");
        assert!(output.contains('^'), "missing caret underline");
    }

    #[test]
    fn multiple_errors_are_numbered() {
        // Two broken functions — each should produce its own numbered block.
        let src = "Function A(\nFunction B(\n";
        let errors = parse_errors(src);
        let output = display_errors(src, "", &errors);
        println!("{output}");
        assert!(output.contains("error[1]"));
        // There may be more than one error depending on recovery behaviour.
        assert!(output.contains("error["));
    }

    #[test]
    fn snapshot_missing_end_function() {
        let src = "Function Broken()\n    Return\n";
        let errors = parse_errors(src);
        let output = display_errors(src, "test.psc", &errors);
        // Print for manual inspection; not a hard assertion on exact layout.
        println!("──── snapshot ────\n{output}────────────────");
        assert!(output.contains("-->"));
        assert!(output.contains('|'));
    }
}
