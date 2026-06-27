//! String literal rule.

use crate::{
    cursor::Cursor,
    token::{RawToken, Token, TokenKind},
};

pub fn parse(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !cursor.consume(b'"') {
        return None;
    }

    while !cursor.eof() {
        match cursor.peek().unwrap() {
            b'"' => {
                cursor.bump_ascii();
                break;
            }

            b'\\' => {
                cursor.bump_ascii();

                if cursor.eof() {
                    break;
                }

                if cursor.peek().unwrap().is_ascii() {
                    cursor.bump_ascii();
                } else {
                    cursor.bump_char();
                }
            }

            ch if ch.is_ascii() => {
                cursor.bump_ascii();
            }

            _ => {
                cursor.bump_char();
            }
        }
    }

    Some(RawToken::Token(Token::new(
        TokenKind::String,
        mark.span(cursor),
    )))
}
