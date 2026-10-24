//! Conversion between byte offsets and `(line, column)` positions.
//!
//! The Language Server Protocol counts columns in UTF-16 code units by default,
//! while the syntax tree works with UTF-8 byte offsets.

use papyrus_parser::{TextRange, TextSize};

/// A zero-based line and UTF-16 column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineCol {
    /// Zero-based line.
    pub line: u32,
    /// Zero-based column, in UTF-16 code units.
    pub col: u32,
}

/// Maps offsets of one text to positions and back.
#[derive(Clone, Debug)]
pub struct LineIndex {
    /// Byte offset of the start of each line.
    line_starts: Vec<u32>,
    /// Lines containing non-ASCII characters need a slow path.
    non_ascii_lines: Vec<bool>,
    len: u32,
}

impl LineIndex {
    /// Builds the index of `text`.
    pub fn new(text: &str) -> LineIndex {
        let mut line_starts = vec![0];
        let mut non_ascii_lines = vec![false];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i as u32 + 1);
                non_ascii_lines.push(false);
            } else if b >= 0x80 {
                *non_ascii_lines.last_mut().unwrap() = true;
            }
        }
        LineIndex { line_starts, non_ascii_lines, len: text.len() as u32 }
    }

    /// Number of lines.
    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }

    /// Converts a byte offset into a position. `text` must be the indexed text.
    pub fn line_col(&self, text: &str, offset: TextSize) -> LineCol {
        let offset = u32::from(offset).min(self.len);
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let start = self.line_starts[line];
        let col = if self.non_ascii_lines[line] {
            let mut end = offset as usize;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text[start as usize..end].encode_utf16().count() as u32
        } else {
            offset - start
        };
        LineCol { line: line as u32, col }
    }

    /// Converts a position into a byte offset, clamping out-of-range values.
    pub fn offset(&self, text: &str, pos: LineCol) -> TextSize {
        let Some(&start) = self.line_starts.get(pos.line as usize) else {
            return TextSize::new(self.len);
        };
        let end = self.line_starts.get(pos.line as usize + 1).map_or(self.len, |&next| next);
        if !self.non_ascii_lines[pos.line as usize] {
            return TextSize::new((start + pos.col).min(end));
        }
        let mut utf16 = 0;
        for (i, c) in text[start as usize..end as usize].char_indices() {
            if utf16 >= pos.col {
                return TextSize::new(start + i as u32);
            }
            utf16 += c.len_utf16() as u32;
        }
        TextSize::new(end)
    }

    /// Converts a byte range into a pair of positions.
    pub fn range(&self, text: &str, range: TextRange) -> (LineCol, LineCol) {
        (self.line_col(text, range.start()), self.line_col(text, range.end()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_roundtrip() {
        let text = "ab\ncd\r\nef";
        let idx = LineIndex::new(text);
        assert_eq!(idx.line_col(text, 4.into()), LineCol { line: 1, col: 1 });
        assert_eq!(idx.offset(text, LineCol { line: 2, col: 1 }), TextSize::new(8));
        assert_eq!(idx.offset(text, LineCol { line: 9, col: 0 }), TextSize::new(9));
    }

    #[test]
    fn utf16_columns() {
        let text = "; 日本\nx";
        let idx = LineIndex::new(text);
        // `日` is 3 bytes but 1 UTF-16 unit.
        assert_eq!(idx.line_col(text, 5.into()), LineCol { line: 0, col: 3 });
        assert_eq!(idx.offset(text, LineCol { line: 0, col: 3 }), TextSize::new(5));
        assert_eq!(idx.offset(text, LineCol { line: 1, col: 0 }), TextSize::new(9));
    }
}
