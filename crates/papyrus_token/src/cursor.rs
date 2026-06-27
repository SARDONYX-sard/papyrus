//! Source cursor.

use crate::span::{TextSize, TextSpan};

#[derive(Debug, Clone)]
pub struct Cursor<'src> {
    source: &'src str,
    bytes: &'src [u8],
    offset: TextSize,
}

impl<'src> Cursor<'src> {
    #[inline]
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            offset: 0,
        }
    }

    #[expect(unused)]
    #[inline]
    pub fn source(&self) -> &'src str {
        self.source
    }

    #[inline]
    pub fn remaining(&self) -> &'src str {
        &self.source[self.offset as usize..]
    }

    #[expect(unused)]
    #[inline]
    pub fn offset(&self) -> TextSize {
        self.offset
    }

    #[inline]
    pub fn eof(&self) -> bool {
        self.offset as usize >= self.bytes.len()
    }

    /// crate span start with current offset.
    #[inline]
    pub fn mark(&self) -> Mark {
        Mark { start: self.offset }
    }

    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset as usize).copied()
    }

    #[inline]
    pub fn peek_n(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.offset as usize + n).copied()
    }

    /// Advances one ASCII byte.
    ///
    /// # Panics
    ///
    /// Panics if the current byte is not ASCII.
    #[inline]
    pub fn bump_ascii(&mut self) {
        let ch = self.peek().expect("unexpected eof");

        debug_assert!(ch.is_ascii());

        self.offset += 1;
    }

    /// Advances one UTF-8 code point.
    ///
    /// # Panics
    ///
    /// Panics at EOF.
    #[inline]
    pub fn bump_char(&mut self) {
        let ch = self.remaining().chars().next().expect("unexpected eof");

        self.offset += ch.len_utf8() as TextSize;
    }

    #[inline]
    pub fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.bump_ascii();
            true
        } else {
            false
        }
    }

    #[expect(unused)]
    #[inline]
    pub fn consume_if(&mut self, pred: impl FnOnce(u8) -> bool) -> bool {
        match self.peek() {
            Some(ch) if pred(ch) => {
                self.bump_ascii();
                true
            }
            _ => false,
        }
    }

    #[inline]
    pub fn consume_bytes(&mut self, bytes: &[u8]) -> bool {
        if self.remaining().as_bytes().starts_with(bytes) {
            self.offset += bytes.len() as TextSize;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn consume_while(&mut self, mut pred: impl FnMut(u8) -> bool) {
        while let Some(ch) = self.peek() {
            if !pred(ch) {
                break;
            }

            self.bump_ascii();
        }
    }

    #[inline]
    pub fn consume_until(&mut self, mut pred: impl FnMut(u8) -> bool) {
        while let Some(ch) = self.peek() {
            if pred(ch) {
                break;
            }

            if ch.is_ascii() {
                self.bump_ascii();
            } else {
                self.bump_char();
            }
        }
    }

    #[inline]
    pub fn span_from(&self, start: TextSize) -> TextSpan {
        TextSpan::new(start, self.offset)
    }

    #[inline]
    pub fn text(&self, span: TextSpan) -> &'src str {
        &self.source[span.as_range()]
    }
}

/// Start span
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    start: TextSize,
}

impl Mark {
    #[expect(unused)]
    #[inline]
    pub fn start(self) -> TextSize {
        self.start
    }

    #[inline]
    pub fn span(self, cursor: &Cursor<'_>) -> TextSpan {
        cursor.span_from(self.start)
    }
}
