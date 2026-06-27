//! Scanner rule.

use crate::{cursor::Cursor, token::RawToken};

pub type Rule = fn(&mut Cursor<'_>) -> Option<RawToken>;

pub mod comment;
pub mod identifier;
pub mod number;
pub mod operator;
pub mod string;
pub mod trivia;

pub const RULES: &[Rule] = &[
    trivia::whitespace,
    trivia::newline,
    comment::line_continuation,
    comment::bracket,
    comment::block,
    comment::line,
    identifier::parse,
    number::parse,
    string::parse,
    operator::parse,
];
