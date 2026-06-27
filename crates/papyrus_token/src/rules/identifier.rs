//! Identifier rule.

use crate::{
    cursor::Cursor,
    keyword::classify,
    token::{RawToken, Token},
};

#[inline]
fn is_ident_start(ch: u8) -> bool {
    matches!(ch, b'a'..=b'z' | b'A'..=b'Z' | b'_')
}

#[inline]
fn is_ident_continue(ch: u8) -> bool {
    matches!(
        ch,
        b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'_'
    )
}

/// Parses an identifier.
///
/// Grammar:
///
/// ```text
/// identifier ::= [A-Za-z_][A-Za-z0-9_]*
/// ```
pub fn parse(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    let ch = cursor.peek()?;

    if !is_ident_start(ch) {
        return None;
    }

    cursor.bump_ascii();

    cursor.consume_while(is_ident_continue);

    let span = mark.span(cursor);

    Some(RawToken::Token(Token::new(
        classify(cursor.text(span)),
        mark.span(cursor),
    )))
}
