//! Lexing `&str` into a sequence of Rust tokens.
//!
//! Note that strictly speaking the parser in this crate is not required to work
//! on tokens which originated from text. Macros, eg, can synthesize tokens out
//! of thin air. So, ideally, lexer should be an orthogonal crate. It is however
//! convenient to include a text-based lexer here!
//!
//! Note that these tokens, unlike the tokens we feed into the parser, do
//! include info about comments and whitespace.

use std::ops;

use rustc_literal_escaper::{EscapeError, Mode, unescape_str};

use crate::{
    SyntaxKind::{self, *},
    T,
};

pub struct LexedStr<'a> {
    text: &'a str,
    kind: Vec<SyntaxKind>,
    start: Vec<u32>,
    error: Vec<LexError>,
}

struct LexError {
    msg: String,
    token: u32,
}

impl<'a> LexedStr<'a> {
    pub fn new(text: &'a str) -> LexedStr<'a> {
        let _p = tracing::info_span!("LexedStr::new").entered();
        let mut conv = Converter::new(text);

        // Re-create the tokenizer from scratch every token because `GuardedStrPrefix` is one token in the lexer
        // but we want to split it to two in edition <2024.
        while let Some(token) = papyrus_lexer::tokenize(&text[conv.offset..]).next() {
            let token_text = &text[conv.offset..][..token.len as usize];

            conv.extend_token(&token.kind, token_text);
        }

        conv.finalize_with_eof()
    }

    pub fn single_token(text: &'a str) -> Option<(SyntaxKind, Option<String>)> {
        if text.is_empty() {
            return None;
        }

        let token = papyrus_lexer::tokenize(text).next()?;
        if token.len as usize != text.len() {
            return None;
        }

        let mut conv = Converter::new(text);
        conv.extend_token(&token.kind, text);
        match &*conv.res.kind {
            [kind] => Some((*kind, conv.res.error.pop().map(|it| it.msg))),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        self.text
    }

    pub fn len(&self) -> usize {
        self.kind.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn kind(&self, i: usize) -> SyntaxKind {
        assert!(i < self.len());
        self.kind[i]
    }

    pub fn text(&self, i: usize) -> &str {
        self.range_text(i..i + 1)
    }

    pub fn range_text(&self, r: ops::Range<usize>) -> &str {
        assert!(r.start < r.end && r.end <= self.len());

        let lo = self.start[r.start] as usize;
        let hi = self.start[r.end] as usize;
        &self.text[lo..hi]
    }

    // Naming is hard.
    pub fn text_range(&self, i: usize) -> ops::Range<usize> {
        assert!(i < self.len());
        let lo = self.start[i] as usize;
        let hi = self.start[i + 1] as usize;
        lo..hi
    }
    pub fn text_start(&self, i: usize) -> usize {
        assert!(i <= self.len());
        self.start[i] as usize
    }
    pub fn text_len(&self, i: usize) -> usize {
        assert!(i < self.len());
        let r = self.text_range(i);
        r.end - r.start
    }

    pub fn error(&self, i: usize) -> Option<&str> {
        assert!(i < self.len());
        let err = self.error.binary_search_by_key(&(i as u32), |i| i.token).ok()?;
        Some(self.error[err].msg.as_str())
    }

    pub fn errors(&self) -> impl Iterator<Item = (usize, &str)> + '_ {
        self.error.iter().map(|it| (it.token as usize, it.msg.as_str()))
    }

    fn push(&mut self, kind: SyntaxKind, offset: usize) {
        self.kind.push(kind);
        self.start.push(offset as u32);
    }
}

struct Converter<'a> {
    res: LexedStr<'a>,
    offset: usize,
}

impl<'a> Converter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            res: LexedStr {
                text,
                kind: Vec::with_capacity(text.len() / 3),
                start: Vec::with_capacity(text.len() / 3),
                error: Vec::new(),
            },
            offset: 0,
        }
    }

    /// Check for likely unterminated string by analyzing STRING token content
    fn has_likely_unterminated_string(&self) -> bool {
        let Some(last_idx) = self.res.kind.len().checked_sub(1) else { return false };

        for i in (0..=last_idx).rev().take(5) {
            if self.res.kind[i] == STRING {
                let start = self.res.start[i] as usize;
                let end = self.res.start.get(i + 1).map(|&s| s as usize).unwrap_or(self.offset);
                let content = &self.res.text[start..end];

                if content.contains('(') && (content.contains(";") || content.contains("\n")) {
                    return true;
                }
            }
        }
        false
    }

    fn finalize_with_eof(mut self) -> LexedStr<'a> {
        self.res.push(EOF, self.offset);
        self.res
    }

    fn push(&mut self, kind: SyntaxKind, len: usize, errors: Vec<String>) {
        self.res.push(kind, self.offset);
        self.offset += len;

        for msg in errors {
            if !msg.is_empty() {
                self.res.error.push(LexError { msg, token: self.res.len() as u32 });
            }
        }
    }

    fn extend_token(&mut self, kind: &papyrus_lexer::TokenKind, token_text: &str) {
        // A note on an intended tradeoff:
        // We drop some useful information here (see patterns with double dots `..`)
        // Storing that info in `SyntaxKind` is not possible due to its layout requirements of
        // being `u16` that come from `rowan::SyntaxKind`.
        let mut errors: Vec<String> = vec![];

        let syntax_kind = {
            match kind {
                papyrus_lexer::TokenKind::LineComment => COMMENT,
                papyrus_lexer::TokenKind::BlockComment { terminated } => {
                    if !terminated {
                        errors.push(
                            "Missing trailing `/;` symbols to terminate the block comment".into(),
                        );
                    }
                    COMMENT
                }
                papyrus_lexer::TokenKind::DocComment { terminated } => {
                    if !terminated {
                        errors.push(
                            "Missing trailing `}` symbols to terminate the doc comment".into(),
                        );
                    }
                    COMMENT
                }
                papyrus_lexer::TokenKind::BackSlash | papyrus_lexer::TokenKind::Whitespace => {
                    WHITESPACE
                }

                papyrus_lexer::TokenKind::Ident => {
                    SyntaxKind::from_keyword(token_text).unwrap_or(IDENT)
                }
                papyrus_lexer::TokenKind::InvalidIdent => {
                    errors.push("Ident contains invalid characters".into());
                    IDENT
                }

                papyrus_lexer::TokenKind::Literal { kind, .. } => {
                    self.extend_literal(token_text.len(), kind);
                    return;
                }

                papyrus_lexer::TokenKind::Comma => T![,],
                papyrus_lexer::TokenKind::Dot => T![.],
                papyrus_lexer::TokenKind::OpenParen => T!['('],
                papyrus_lexer::TokenKind::CloseParen => T![')'],
                papyrus_lexer::TokenKind::OpenBracket => T!['['],
                papyrus_lexer::TokenKind::CloseBracket => T![']'],
                papyrus_lexer::TokenKind::Eq => T![=],
                papyrus_lexer::TokenKind::Bang => T![!],
                papyrus_lexer::TokenKind::Lt => T![<],
                papyrus_lexer::TokenKind::Gt => T![>],
                papyrus_lexer::TokenKind::Minus => T![-],
                papyrus_lexer::TokenKind::And => T![&],
                papyrus_lexer::TokenKind::Or => T![|],
                papyrus_lexer::TokenKind::Plus => T![+],
                papyrus_lexer::TokenKind::Star => T![*],
                papyrus_lexer::TokenKind::Slash => T![/],
                papyrus_lexer::TokenKind::Percent => T![%],
                papyrus_lexer::TokenKind::Unknown => ERROR,
                papyrus_lexer::TokenKind::UnknownPrefix => {
                    let has_unterminated = self.has_likely_unterminated_string();

                    let error_msg = if has_unterminated {
                        format!(
                            "unknown literal prefix `{token_text}` (note: check for unterminated string literal)"
                        )
                    } else {
                        "unknown literal prefix".to_owned()
                    };
                    errors.push(error_msg);
                    IDENT
                }
                papyrus_lexer::TokenKind::Eof => EOF,
            }
        };

        self.push(syntax_kind, token_text.len(), errors);
    }

    fn extend_literal(&mut self, len: usize, kind: &papyrus_lexer::LiteralKind) {
        let mut errors = vec![];
        let mut no_end_quote = |c: char, kind: &str| {
            errors.push(format!("Missing trailing `{c}` symbol to terminate the {kind} literal"));
        };

        let syntax_kind = match *kind {
            papyrus_lexer::LiteralKind::Int { empty_int, base: _ } => {
                if empty_int {
                    errors.push("Missing digits after the integer base prefix".into());
                }
                INT_NUMBER
            }
            papyrus_lexer::LiteralKind::Float { empty_exponent, base: _ } => {
                if empty_exponent {
                    errors.push("Missing digits after the exponent symbol".into());
                }
                FLOAT_NUMBER
            }
            papyrus_lexer::LiteralKind::Str { terminated } => {
                if !terminated {
                    no_end_quote('"', "string");
                } else {
                    let text = &self.res.text[self.offset + 1..][..len - 1];
                    let text = &text[..text.rfind('"').unwrap()];
                    unescape_str(text, |_, res| {
                        if let Err(e) = res {
                            errors.push(err_to_msg(e, Mode::Str));
                        }
                    });
                }
                STRING
            }
        };

        self.push(syntax_kind, len, errors);
    }
}

fn err_to_msg(error: EscapeError, mode: Mode) -> String {
    match error {
        EscapeError::ZeroChars => "empty character literal",
        EscapeError::MoreThanOneChar => "character literal may only contain one codepoint",
        EscapeError::LoneSlash => "",
        EscapeError::InvalidEscape if mode == Mode::Byte || mode == Mode::ByteStr => {
            "unknown byte escape"
        }
        EscapeError::InvalidEscape => "unknown character escape",
        EscapeError::BareCarriageReturn => "",
        EscapeError::BareCarriageReturnInRawString => "",
        EscapeError::EscapeOnlyChar if mode == Mode::Byte => "byte constant must be escaped",
        EscapeError::EscapeOnlyChar => "character constant must be escaped",
        EscapeError::TooShortHexEscape => "numeric character escape is too short",
        EscapeError::InvalidCharInHexEscape => "invalid character in numeric character escape",
        EscapeError::OutOfRangeHexEscape => "out of range hex escape",
        EscapeError::NoBraceInUnicodeEscape => "incorrect unicode escape sequence",
        EscapeError::InvalidCharInUnicodeEscape => "invalid character in unicode escape",
        EscapeError::EmptyUnicodeEscape => "empty unicode escape",
        EscapeError::UnclosedUnicodeEscape => "unterminated unicode escape",
        EscapeError::LeadingUnderscoreUnicodeEscape => "invalid start of unicode escape",
        EscapeError::OverlongUnicodeEscape => "overlong unicode escape",
        EscapeError::LoneSurrogateUnicodeEscape => "invalid unicode character escape",
        EscapeError::OutOfRangeUnicodeEscape => "invalid unicode character escape",
        EscapeError::UnicodeEscapeInByte => "unicode escape in byte string",
        EscapeError::NonAsciiCharInByte if mode == Mode::Byte => {
            "non-ASCII character in byte literal"
        }
        EscapeError::NonAsciiCharInByte if mode == Mode::ByteStr => {
            "non-ASCII character in byte string literal"
        }
        EscapeError::NonAsciiCharInByte => "non-ASCII character in raw byte string literal",
        EscapeError::NulInCStr => "null character in C string literal",
        EscapeError::UnskippedWhitespaceWarning => "",
        EscapeError::MultipleSkippedLinesWarning => "",
    }
    .into()
}
