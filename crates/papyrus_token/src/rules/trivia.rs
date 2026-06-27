//! Trivia rules.

use crate::{
    cursor::Cursor,
    token::{RawToken, Trivia, TriviaKind},
};

#[inline]
fn is_whitespace(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t')
}

/// Parses spaces and tabs.
///
/// Does not consume newlines.
pub fn whitespace(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    if !is_whitespace(cursor.peek()?) {
        return None;
    }

    let mark = cursor.mark();

    cursor.bump_ascii();
    cursor.consume_while(is_whitespace);

    Some(RawToken::Trivia(Trivia::new(
        TriviaKind::Whitespace,
        mark.span(cursor),
    )))
}

/// Parses LF or CRLF.
///
/// A CR not followed by LF is also treated as a newline.
pub fn newline(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    match cursor.peek()? {
        b'\n' => {
            cursor.bump_ascii();
        }

        b'\r' => {
            cursor.bump_ascii();

            cursor.consume(b'\n');
        }

        _ => return None,
    }

    Some(RawToken::Trivia(Trivia::new(
        TriviaKind::Newline,
        mark.span(cursor),
    )))
}
