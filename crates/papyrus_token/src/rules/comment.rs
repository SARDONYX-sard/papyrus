//! Comment rules.

use crate::{
    cursor::Cursor,
    token::{RawToken, Trivia, TriviaKind},
};

#[inline]
fn is_newline(ch: u8) -> bool {
    matches!(ch, b'\r' | b'\n')
}

/// Line continuation: `\` followed by optional spaces/tabs and then `\n` or `\r\n`.
///
/// If no newline is found after the `\` and optional whitespace, the cursor
/// is reset and `None` is returned so the caller can emit an `Unknown` token.
pub fn line_continuation(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !cursor.consume(b'\\') {
        return None;
    }

    cursor.consume_until(is_newline);

    Some(RawToken::Trivia(Trivia::new(
        TriviaKind::LineContinuation,
        mark.span(cursor),
    )))
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

/// /; block comment /;
///
/// Adjust this if Papyrus uses a different syntax.
pub fn block(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !cursor.consume_bytes(b";/") {
        return None;
    }

    while !cursor.eof() {
        if cursor.consume_bytes(b"/;") {
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

/// { block comment };
///
/// Adjust this if Papyrus uses a different syntax.
pub fn bracket(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    if !cursor.consume_bytes(b"{") {
        return None;
    }

    while !cursor.eof() {
        if cursor.consume_bytes(b"}") {
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
