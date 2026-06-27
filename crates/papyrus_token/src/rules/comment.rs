//! Comment rules.

use crate::{
    cursor::Cursor,
    token::{RawToken, Trivia, TriviaKind},
};

#[inline]
fn is_newline(ch: u8) -> bool {
    matches!(ch, b'\r' | b'\n')
}

/// ; line comment
pub fn line(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !cursor.consume(b';') {
        return None;
    }

    cursor.consume_until(is_newline);

    Some(RawToken::Trivia(Trivia::new(
        TriviaKind::LineComment,
        mark.span(cursor),
    )))
}

/// /* block comment */
///
/// Adjust this if Papyrus uses a different syntax.
pub fn block(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !cursor.consume_bytes(b"/*") {
        return None;
    }

    while !cursor.eof() {
        if cursor.consume_bytes(b"*/") {
            break;
        }

        if cursor.peek().unwrap().is_ascii() {
            cursor.bump_ascii();
        } else {
            cursor.bump_char();
        }
    }

    Some(RawToken::Trivia(Trivia::new(
        TriviaKind::BlockComment,
        mark.span(cursor),
    )))
}
