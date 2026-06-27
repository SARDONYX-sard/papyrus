//! Lexical token definitions.

use crate::span::TextSpan;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    //
    // Literals
    //
    Identifier,
    Number,
    String,

    //
    // Primitive types
    //
    Bool,
    Int,
    Float,
    StringTy,

    //
    // Keywords
    //
    ScriptName,
    Extends,
    Import,

    Function,
    EndFunction,

    Event,
    EndEvent,

    Property,
    EndProperty,

    State,
    EndState,

    If,
    Else,
    ElseIf,
    EndIf,

    While,
    EndWhile,

    Return,

    Auto,
    AutoReadOnly,

    Native,
    Global,

    New,
    None,

    Self_,
    Parent,

    True,
    False,

    As,
    Is,

    Length,

    //
    // Delimiters
    //
    LParen, // (
    RParen, // )

    LBracket, // [
    RBracket, // ]

    LBrace, // comment block start: {
    RBrace, // comment block end: }

    Dot,
    Comma,
    Colon,
    Semicolon,

    //
    // Operators
    //
    Assign,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,

    EqEq,
    NotEq,

    Lt,
    LtEq,

    Gt,
    GtEq,

    And,
    Or,
    Bang,

    AndAnd,
    OrOr,

    //
    // Misc
    //
    Unknown,
    Eof,
}

impl TokenKind {
    #[inline]
    pub const fn is_keyword(self) -> bool {
        !matches!(
            self,
            Self::Identifier
                | Self::Number
                | Self::String
                | Self::LParen
                | Self::RParen
                | Self::LBracket
                | Self::RBracket
                | Self::LBrace
                | Self::RBrace
                | Self::Dot
                | Self::Comma
                | Self::Colon
                | Self::Semicolon
                | Self::Assign
                | Self::Plus
                | Self::Minus
                | Self::Star
                | Self::Slash
                | Self::Percent
                | Self::PlusAssign
                | Self::MinusAssign
                | Self::StarAssign
                | Self::SlashAssign
                | Self::PercentAssign
                | Self::EqEq
                | Self::NotEq
                | Self::Lt
                | Self::LtEq
                | Self::Gt
                | Self::GtEq
                | Self::And
                | Self::Or
                | Self::Bang
                | Self::AndAnd
                | Self::OrOr
                | Self::Unknown
                | Self::Eof
        )
    }

    #[inline]
    pub const fn is_literal(self) -> bool {
        matches!(
            self,
            Self::Identifier | Self::Number | Self::String | Self::True | Self::False | Self::None
        )
    }

    #[inline]
    pub const fn is_operator(self) -> bool {
        matches!(
            self,
            Self::Assign
                | Self::Plus
                | Self::Minus
                | Self::Star
                | Self::Slash
                | Self::Percent
                | Self::PlusAssign
                | Self::MinusAssign
                | Self::StarAssign
                | Self::SlashAssign
                | Self::PercentAssign
                | Self::EqEq
                | Self::NotEq
                | Self::Lt
                | Self::LtEq
                | Self::Gt
                | Self::GtEq
                | Self::And
                | Self::Or
                | Self::Bang
                | Self::AndAnd
                | Self::OrOr
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub span: TextSpan,

    /// Docs and other information preceding the function go here.
    pub leading_trivia: Vec<Trivia>,
    /// # Notes
    /// A space in the middle is treated as a leading space.
    ///
    /// In the case of `ScriptName Ident`, the space in the middle is treated as a leading trivia for `Ident`.
    pub trailing_trivia: Vec<Trivia>,
}

impl Token {
    #[inline]
    pub const fn new(kind: TokenKind, span: TextSpan) -> Self {
        Self {
            kind,
            span,
            leading_trivia: Vec::new(),  // Not yet: Build by TokenBuilder
            trailing_trivia: Vec::new(), // Not yet: Build by TokenBuilder
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriviaKind {
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    LineContinuation, // `\` immediately followed by `\n`
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub span: TextSpan,
}

impl Trivia {
    #[inline]
    pub const fn new(kind: TriviaKind, span: TextSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RawToken {
    Trivia(Trivia),
    Token(Token),
}
