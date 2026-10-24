//! Zero-copy lexer for the Papyrus scripting language (Skyrim dialect).
//!
//! The lexer never allocates: every [`Token`] borrows its text from the source
//! string, and the token stream covers the input losslessly, i.e. concatenating
//! the text of all tokens yields the original source.
//!
//! Papyrus is a line-oriented language: a statement or a declaration header ends
//! at the end of the line. Because of that, line breaks are *not* treated as
//! whitespace but are emitted as dedicated [`TokenKind::Newline`] tokens. A
//! backslash at the end of a line joins it with the next one and is emitted as a
//! single [`TokenKind::LineContinuation`] token (backslash, trailing blanks and
//! the line break), which the parser treats as trivia.
//!
//! Keywords are recognized case-insensitively, as the Papyrus compiler does.
//!
//! # Example
//!
//! ```
//! use papyrus_lexer::{Lexer, TokenKind};
//!
//! let kinds: Vec<_> = Lexer::new("ScriptName Foo\n").map(|t| t.kind).collect();
//! assert_eq!(
//!     kinds,
//!     [TokenKind::ScriptNameKw, TokenKind::Whitespace, TokenKind::Ident, TokenKind::Newline]
//! );
//! ```

#[cfg(test)]
mod tests;

use std::fmt;

/// The kind of a lexical token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TokenKind {
    // === Trivia ===
    /// Horizontal whitespace (spaces, tabs, lone carriage returns, ...).
    Whitespace,
    /// A line break: `\n` or `\r\n`. Significant: it terminates statements.
    Newline,
    /// `; comment` up to, but excluding, the line break.
    LineComment,
    /// `;/ comment /;`, possibly spanning multiple lines.
    BlockComment,
    /// `{ documentation }`, possibly spanning multiple lines.
    DocComment,
    /// `\` followed by optional blanks and a line break.
    LineContinuation,

    // === Literals and names ===
    /// An identifier that is not a keyword.
    Ident,
    /// An integer literal: `42`, `0x1F`.
    Int,
    /// A float literal: `1.5`, `2.0e3`.
    Float,
    /// A string literal: `"text"`.
    String,

    // === Punctuation ===
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `=`
    Eq,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `+`
    Plus,
    /// `+=`
    PlusEq,
    /// `-`
    Minus,
    /// `-=`
    MinusEq,
    /// `*`
    Star,
    /// `*=`
    StarEq,
    /// `/`
    Slash,
    /// `/=`
    SlashEq,
    /// `%`
    Percent,
    /// `%=`
    PercentEq,
    /// `!`
    Bang,
    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,

    // === Keywords ===
    /// `As`
    AsKw,
    /// `Auto`
    AutoKw,
    /// `AutoReadOnly`
    AutoReadOnlyKw,
    /// `Bool`
    BoolKw,
    /// `Else`
    ElseKw,
    /// `ElseIf`
    ElseIfKw,
    /// `EndEvent`
    EndEventKw,
    /// `EndFunction`
    EndFunctionKw,
    /// `EndIf`
    EndIfKw,
    /// `EndProperty`
    EndPropertyKw,
    /// `EndState`
    EndStateKw,
    /// `EndWhile`
    EndWhileKw,
    /// `Event`
    EventKw,
    /// `Extends`
    ExtendsKw,
    /// `False`
    FalseKw,
    /// `Float`
    FloatKw,
    /// `Function`
    FunctionKw,
    /// `Global`
    GlobalKw,
    /// `If`
    IfKw,
    /// `Import`
    ImportKw,
    /// `Int`
    IntKw,
    /// `Native`
    NativeKw,
    /// `New`
    NewKw,
    /// `None`
    NoneKw,
    /// `Parent`
    ParentKw,
    /// `Property`
    PropertyKw,
    /// `Return`
    ReturnKw,
    /// `ScriptName`
    ScriptNameKw,
    /// `Self`
    SelfKw,
    /// `State`
    StateKw,
    /// `String`
    StringKw,
    /// `True`
    TrueKw,
    /// `While`
    WhileKw,

    /// A character sequence that is not part of the language, e.g. `#`, `&`
    /// or a backslash that does not end a line.
    Unknown,
}

impl TokenKind {
    /// Returns `true` for tokens the parser skips: whitespace, comments and line
    /// continuations. Note that [`TokenKind::Newline`] is **not** trivia.
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            TokenKind::Whitespace
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::DocComment
                | TokenKind::LineContinuation
        )
    }

    /// Returns `true` for comment tokens.
    pub fn is_comment(self) -> bool {
        matches!(self, TokenKind::LineComment | TokenKind::BlockComment | TokenKind::DocComment)
    }

    /// Returns `true` for reserved words.
    pub fn is_keyword(self) -> bool {
        (self as u8) >= (TokenKind::AsKw as u8) && (self as u8) <= (TokenKind::WhileKw as u8)
    }
}

/// A recoverable lexical error attached to a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexError {
    /// A string literal reached the end of the line or file without a closing `"`.
    UnterminatedString,
    /// A `;/` block comment reached the end of file without a closing `/;`.
    UnterminatedBlockComment,
    /// A `{` documentation comment reached the end of file without a closing `}`.
    UnterminatedDocComment,
    /// A hexadecimal literal `0x` without any digit.
    EmptyHexLiteral,
    /// A float exponent without any digit, e.g. `1.0e`.
    EmptyExponent,
    /// A character that cannot start any token.
    UnknownChar,
    /// A backslash that is not followed by a line break.
    StrayBackslash,
}

impl LexError {
    /// Human readable message describing the error.
    pub fn message(self) -> &'static str {
        match self {
            LexError::UnterminatedString => "unterminated string literal",
            LexError::UnterminatedBlockComment => "unterminated block comment, expected `/;`",
            LexError::UnterminatedDocComment => "unterminated documentation comment, expected `}`",
            LexError::EmptyHexLiteral => "expected hexadecimal digits after `0x`",
            LexError::EmptyExponent => "expected digits in float exponent",
            LexError::UnknownChar => "unknown character",
            LexError::StrayBackslash => {
                "a line continuation `\\` must be the last character on the line"
            }
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// A token borrowing its text from the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token<'a> {
    /// The kind of this token.
    pub kind: TokenKind,
    /// The exact source text of this token.
    pub text: &'a str,
    /// Byte offset of the token in the source.
    pub offset: u32,
    /// The lexical error of this token, if it is malformed.
    pub error: Option<LexError>,
}

impl Token<'_> {
    /// Byte length of the token.
    pub fn len(&self) -> u32 {
        self.text.len() as u32
    }
}

/// An iterator over the [`Token`]s of a source string.
///
/// The source must be smaller than 4 GiB, since offsets are stored as `u32`.
#[derive(Clone)]
pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer at the start of `src`.
    pub fn new(src: &'a str) -> Lexer<'a> {
        assert!(u32::try_from(src.len()).is_ok(), "source files must be smaller than 4 GiB");
        Lexer { src, pos: 0 }
    }

    fn bytes(&self) -> &'a [u8] {
        self.src.as_bytes()
    }

    fn peek(&self, n: usize) -> u8 {
        self.bytes().get(self.pos + n).copied().unwrap_or(0)
    }

    fn eat_while(&mut self, mut pred: impl FnMut(u8) -> bool) {
        while self.pos < self.src.len() && pred(self.bytes()[self.pos]) {
            self.pos += 1;
        }
    }

    /// Lexes one token starting at `self.pos`, which must not be at the end.
    fn lex(&mut self) -> (TokenKind, Option<LexError>) {
        use TokenKind as T;

        let b = self.peek(0);
        let next = self.peek(1);
        self.pos += 1;

        let kind = match b {
            b'\n' => T::Newline,
            b'\r' if next == b'\n' => {
                self.pos += 1;
                T::Newline
            }
            b' ' | b'\t' | b'\r' | 0x0B | 0x0C => return (self.whitespace(), None),

            b';' if next == b'/' => return self.block_comment(),
            b';' => {
                self.eat_while(|b| b != b'\n' && b != b'\r');
                T::LineComment
            }
            b'{' => return self.doc_comment(),
            b'\\' => return self.backslash(),

            b'"' => return self.string(),
            b'0'..=b'9' => return self.number(b),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => return (self.ident_or_keyword(), None),

            b'(' => T::LParen,
            b')' => T::RParen,
            b'[' => T::LBracket,
            b']' => T::RBracket,
            b',' => T::Comma,
            b'.' => T::Dot,
            b'=' => self.with_eq(T::Eq, T::EqEq),
            b'!' => self.with_eq(T::Bang, T::NotEq),
            b'<' => self.with_eq(T::Lt, T::LtEq),
            b'>' => self.with_eq(T::Gt, T::GtEq),
            b'+' => self.with_eq(T::Plus, T::PlusEq),
            b'-' => self.with_eq(T::Minus, T::MinusEq),
            b'*' => self.with_eq(T::Star, T::StarEq),
            b'/' => self.with_eq(T::Slash, T::SlashEq),
            b'%' => self.with_eq(T::Percent, T::PercentEq),
            b'&' if next == b'&' => {
                self.pos += 1;
                T::AmpAmp
            }
            b'|' if next == b'|' => {
                self.pos += 1;
                T::PipePipe
            }

            _ => {
                // Consume the whole UTF-8 character so that offsets stay on char
                // boundaries.
                self.pos -= 1;
                let c = self.src[self.pos..].chars().next().unwrap_or('\0');
                self.pos += c.len_utf8();
                if c != '\u{FEFF}' && c.is_whitespace() {
                    return (self.whitespace(), None);
                }
                if c == '\u{FEFF}' {
                    return (T::Whitespace, None);
                }
                return (T::Unknown, Some(LexError::UnknownChar));
            }
        };
        (kind, None)
    }

    fn with_eq(&mut self, single: TokenKind, with_eq: TokenKind) -> TokenKind {
        if self.peek(0) == b'=' {
            self.pos += 1;
            with_eq
        } else {
            single
        }
    }

    fn whitespace(&mut self) -> TokenKind {
        loop {
            match self.peek(0) {
                b' ' | b'\t' | 0x0B | 0x0C => self.pos += 1,
                b'\r' if self.peek(1) != b'\n' => self.pos += 1,
                b if b >= 0x80 => {
                    let c = self.src[self.pos..].chars().next().unwrap_or('\0');
                    if c.is_whitespace() && c != '\u{FEFF}' {
                        self.pos += c.len_utf8();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        TokenKind::Whitespace
    }

    fn block_comment(&mut self) -> (TokenKind, Option<LexError>) {
        // `;` already eaten, eat `/`.
        self.pos += 1;
        match find_block_comment_end(&self.bytes()[self.pos..]) {
            Some(end) => {
                self.pos += end + 2;
                (TokenKind::BlockComment, None)
            }
            None => {
                self.pos = self.src.len();
                (TokenKind::BlockComment, Some(LexError::UnterminatedBlockComment))
            }
        }
    }

    fn doc_comment(&mut self) -> (TokenKind, Option<LexError>) {
        match memchr::memchr(b'}', &self.bytes()[self.pos..]) {
            Some(end) => {
                self.pos += end + 1;
                (TokenKind::DocComment, None)
            }
            None => {
                self.pos = self.src.len();
                (TokenKind::DocComment, Some(LexError::UnterminatedDocComment))
            }
        }
    }

    fn backslash(&mut self) -> (TokenKind, Option<LexError>) {
        let start = self.pos;
        self.eat_while(|b| b == b' ' || b == b'\t');
        match (self.peek(0), self.peek(1)) {
            (b'\n', _) => self.pos += 1,
            (b'\r', b'\n') => self.pos += 2,
            // A continuation on the very last line joins nothing, but is harmless.
            _ if self.pos == self.src.len() => {}
            _ => {
                self.pos = start;
                return (TokenKind::Unknown, Some(LexError::StrayBackslash));
            }
        }
        (TokenKind::LineContinuation, None)
    }

    fn string(&mut self) -> (TokenKind, Option<LexError>) {
        loop {
            match self.peek(0) {
                b'"' => {
                    self.pos += 1;
                    return (TokenKind::String, None);
                }
                b'\\' if matches!(self.peek(1), b'"' | b'\\') => self.pos += 2,
                b'\n' | b'\r' => return (TokenKind::String, Some(LexError::UnterminatedString)),
                _ if self.pos >= self.src.len() => {
                    return (TokenKind::String, Some(LexError::UnterminatedString));
                }
                _ => self.pos += 1,
            }
        }
    }

    fn number(&mut self, first: u8) -> (TokenKind, Option<LexError>) {
        if first == b'0' && matches!(self.peek(0), b'x' | b'X') {
            self.pos += 1;
            let start = self.pos;
            self.eat_while(|b| b.is_ascii_hexdigit());
            let error = (self.pos == start).then_some(LexError::EmptyHexLiteral);
            return (TokenKind::Int, error);
        }

        self.eat_while(|b| b.is_ascii_digit());
        if self.peek(0) != b'.' || !self.peek(1).is_ascii_digit() {
            return (TokenKind::Int, None);
        }

        self.pos += 1;
        self.eat_while(|b| b.is_ascii_digit());
        let mut error = None;
        if matches!(self.peek(0), b'e' | b'E') {
            self.pos += 1;
            if matches!(self.peek(0), b'+' | b'-') {
                self.pos += 1;
            }
            let start = self.pos;
            self.eat_while(|b| b.is_ascii_digit());
            if self.pos == start {
                error = Some(LexError::EmptyExponent);
            }
        }
        (TokenKind::Float, error)
    }

    fn ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos - 1;
        self.eat_while(|b| b.is_ascii_alphanumeric() || b == b'_');
        keyword(&self.src[start..self.pos]).unwrap_or(TokenKind::Ident)
    }
}

/// Finds the first `/;` in `haystack`, returning the index of the `/`.
fn find_block_comment_end(haystack: &[u8]) -> Option<usize> {
    memchr::memmem::find(haystack, b"/;")
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        if self.pos >= self.src.len() {
            return None;
        }
        let start = self.pos;
        let (kind, error) = self.lex();
        Some(Token { kind, text: &self.src[start..self.pos], offset: start as u32, error })
    }
}

/// Convenience function returning all tokens of `src`.
pub fn tokenize(src: &str) -> Vec<Token<'_>> {
    Lexer::new(src).collect()
}

/// Returns the keyword spelled by `ident` (ignoring case), if any.
pub fn keyword(ident: &str) -> Option<TokenKind> {
    use TokenKind as T;

    const MAX_KEYWORD_LEN: usize = "autoreadonly".len();
    if ident.len() < 2 || ident.len() > MAX_KEYWORD_LEN {
        return None;
    }
    let mut buf = [0u8; MAX_KEYWORD_LEN];
    let buf = &mut buf[..ident.len()];
    buf.copy_from_slice(ident.as_bytes());
    buf.make_ascii_lowercase();

    let kw = match &*buf {
        b"as" => T::AsKw,
        b"auto" => T::AutoKw,
        b"autoreadonly" => T::AutoReadOnlyKw,
        b"bool" => T::BoolKw,
        b"else" => T::ElseKw,
        b"elseif" => T::ElseIfKw,
        b"endevent" => T::EndEventKw,
        b"endfunction" => T::EndFunctionKw,
        b"endif" => T::EndIfKw,
        b"endproperty" => T::EndPropertyKw,
        b"endstate" => T::EndStateKw,
        b"endwhile" => T::EndWhileKw,
        b"event" => T::EventKw,
        b"extends" => T::ExtendsKw,
        b"false" => T::FalseKw,
        b"float" => T::FloatKw,
        b"function" => T::FunctionKw,
        b"global" => T::GlobalKw,
        b"if" => T::IfKw,
        b"import" => T::ImportKw,
        b"int" => T::IntKw,
        b"native" => T::NativeKw,
        b"new" => T::NewKw,
        b"none" => T::NoneKw,
        b"parent" => T::ParentKw,
        b"property" => T::PropertyKw,
        b"return" => T::ReturnKw,
        b"scriptname" => T::ScriptNameKw,
        b"self" => T::SelfKw,
        b"state" => T::StateKw,
        b"string" => T::StringKw,
        b"true" => T::TrueKw,
        b"while" => T::WhileKw,
        _ => return None,
    };
    Some(kw)
}

/// Returns `true` if `s` is lexically a valid identifier (keywords included).
pub fn is_ident(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Decodes the content of a string literal token (including its quotes),
/// resolving the escape sequences `\n`, `\t`, `\\` and `\"`.
///
/// Returns a borrowed slice when the literal contains no escapes.
pub fn unescape_string(token_text: &str) -> std::borrow::Cow<'_, str> {
    let inner = token_text.strip_prefix('"').unwrap_or(token_text);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    if !inner.contains('\\') {
        return std::borrow::Cow::Borrowed(inner);
    }

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    std::borrow::Cow::Owned(out)
}
