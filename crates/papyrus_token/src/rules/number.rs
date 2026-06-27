//! Number rule.

use crate::{
    cursor::Cursor,
    token::{RawToken, Token, TokenKind},
};

#[inline]
fn is_digit(ch: u8) -> bool {
    ch.is_ascii_digit()
}

#[inline]
fn is_hex(ch: u8) -> bool {
    ch.is_ascii_hexdigit()
}

pub fn parse(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !is_digit(cursor.peek()?) {
        return None;
    }

    // Integer part
    cursor.bump_ascii();

    // Hexadecimal
    if cursor.peek() == Some(b'x') || cursor.peek() == Some(b'X') {
        cursor.bump_ascii();
        cursor.consume_while(is_hex);

        return Some(RawToken::Token(Token::new(
            TokenKind::Number,
            mark.span(cursor),
        )));
    }

    cursor.consume_while(is_digit);

    // Fraction
    if cursor.peek() == Some(b'.') && cursor.peek_n(1).map(is_digit).unwrap_or(false) {
        cursor.bump_ascii();

        cursor.consume_while(is_digit);
    }

    // Scientific notation
    if matches!(cursor.peek(), Some(b'e') | Some(b'E')) {
        cursor.bump_ascii();

        if matches!(cursor.peek(), Some(b'+') | Some(b'-')) {
            cursor.bump_ascii();
        }

        cursor.consume_while(is_digit);
    }

    Some(RawToken::Token(Token::new(
        TokenKind::Number,
        mark.span(cursor),
    )))
}
