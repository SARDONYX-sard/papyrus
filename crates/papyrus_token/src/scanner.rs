//! Token scanner.

use crate::{
    cursor::Cursor,
    rules::RULES,
    token::{RawToken, Token, TokenKind},
};

#[derive(Debug)]
pub struct Scanner<'src> {
    cursor: Cursor<'src>,
}

impl<'src> Scanner<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            cursor: Cursor::new(source),
        }
    }

    pub fn next_raw(&mut self) -> Option<RawToken> {
        if self.cursor.eof() {
            return None;
        }

        for rule in RULES {
            if let Some(tok) = rule(&mut self.cursor) {
                return Some(tok);
            }
        }

        let mark = self.cursor.mark();
        self.cursor.bump_ascii();

        Some(RawToken::Token(Token::new(
            TokenKind::Unknown,
            mark.span(&self.cursor),
        )))
    }
}

impl<'src> Iterator for Scanner<'src> {
    type Item = RawToken;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_raw()
    }
}
