//! Operator rule.

use crate::{
    cursor::Cursor,
    token::{RawToken, Token, TokenKind},
};

macro_rules! op {
    ($cursor:expr, $mark:expr, $bytes:expr, $kind:expr) => {
        if $cursor.consume_bytes($bytes) {
            return Some(RawToken::Token(Token::new($kind, $mark.span($cursor))));
        }
    };
}

pub fn parse(cursor: &mut Cursor<'_>) -> Option<RawToken> {
    let mark = cursor.mark();

    //
    // longest match first
    //

    op!(cursor, mark, b"==", TokenKind::EqEq);
    op!(cursor, mark, b"!=", TokenKind::NotEq);

    op!(cursor, mark, b"<=", TokenKind::LtEq);
    op!(cursor, mark, b">=", TokenKind::GtEq);

    op!(cursor, mark, b"+=", TokenKind::PlusAssign);
    op!(cursor, mark, b"-=", TokenKind::MinusAssign);
    op!(cursor, mark, b"*=", TokenKind::StarAssign);
    op!(cursor, mark, b"/=", TokenKind::SlashAssign);
    op!(cursor, mark, b"%=", TokenKind::PercentAssign);

    op!(cursor, mark, b"&&", TokenKind::AndAnd);
    op!(cursor, mark, b"||", TokenKind::OrOr);

    //
    // one character
    //

    let kind = match cursor.peek()? {
        b'(' => TokenKind::LParen,
        b')' => TokenKind::RParen,

        b'[' => TokenKind::LBracket,
        b']' => TokenKind::RBracket,

        b'{' => TokenKind::LBrace,
        b'}' => TokenKind::RBrace,

        b',' => TokenKind::Comma,
        b'.' => TokenKind::Dot,
        b':' => TokenKind::Colon,
        b';' => TokenKind::Semicolon,

        b'+' => TokenKind::Plus,
        b'-' => TokenKind::Minus,
        b'*' => TokenKind::Star,
        b'/' => TokenKind::Slash,
        b'%' => TokenKind::Percent,

        b'=' => TokenKind::Assign,

        b'<' => TokenKind::Lt,
        b'>' => TokenKind::Gt,

        b'!' => TokenKind::Bang,

        b'&' => TokenKind::And,
        b'|' => TokenKind::Or,

        _ => return None,
    };

    cursor.bump_ascii();

    Some(RawToken::Token(Token::new(kind, mark.span(cursor))))
}
