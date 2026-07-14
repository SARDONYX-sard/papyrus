// SPDX-License-Identifier: Apache-2.0 OR MIT
// ref: ra-ap-rustc_lexer-0.165.0 lib.rs

// tidy-alphabetical-start
// We want to be able to build this crate with a stable compiler,
// so no `#![feature]` attributes should be added.
#![deny(unstable_features)]
// tidy-alphabetical-end

#[cfg(test)]
mod tests;

use std::str::Chars;

use LiteralKind::*;
use TokenKind::*;
pub use unicode_ident::UNICODE_VERSION;
use unicode_properties::UnicodeEmoji;

// Make sure that the Unicode version of the dependencies is the same.
const _: () = {
    let properties = unicode_properties::UNICODE_VERSION;
    let ident = unicode_ident::UNICODE_VERSION;

    if properties.0 != ident.0 as u64
        || properties.1 != ident.1 as u64
        || properties.2 != ident.2 as u64
    {
        panic!(
            "unicode-properties and unicode-ident must use the same Unicode version, \
            `unicode_properties::UNICODE_VERSION` and `unicode_ident::UNICODE_VERSION` are \
            different."
        );
    }
};

/// Parsed token.
/// It doesn't contain information about data that has been parsed,
/// only the type of the token and its size.
#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub len: u32,
}

impl Token {
    fn new(kind: TokenKind, len: u32) -> Token {
        Token { kind, len }
    }
}

/// Enum representing common lexeme types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// A line comment, e.g. `; comment`.
    LineComment,

    /// A block comment, e.g. `;/ block comment /;`.
    ///
    /// Block comments can be recursive, so a sequence like `;/ ;/ /;`
    /// will not be considered terminated and will result in a parsing error.
    BlockComment { terminated: bool },

    /// A document block comment, e.g. `{ document block comment }`.
    ///
    /// Block comments can be recursive, so a sequence like `{ { }`
    /// will not be considered terminated and will result in a parsing error.
    DocComment { terminated: bool },

    /// Any whitespace character sequence.
    Whitespace,

    /// An identifier or keyword, e.g. `ident` or `continue`.
    Ident,

    /// An identifier that is invalid because it contains emoji.
    InvalidIdent,

    /// An unknown literal prefix, like `foo#`, `foo'`, `foo"`. Excludes
    /// literal prefixes that contain emoji, which are considered "invalid".
    UnknownPrefix,

    /// Literals, e.g. `12u8`, `1.0e-40`, `"123"`. Note that `_` is an invalid
    /// suffix, but may be present here on string and float literals. Users of
    /// this type will need to check for and reject that case.
    ///
    /// See [LiteralKind] for more details.
    Literal { kind: LiteralKind, suffix_start: u32 },

    /// `;`
    Semi,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `[`
    OpenBracket,
    /// `]`
    CloseBracket,
    /// `@`
    At,
    /// `#`
    Pound,
    /// `~`
    Tilde,
    /// `?`
    Question,
    /// `:`
    Colon,
    /// `$`
    Dollar,
    /// `=`
    Eq,
    /// `!`
    Bang,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `-`
    Minus,
    /// `&`
    And,
    /// `|`
    Or,
    /// `+`
    Plus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `^`
    Caret,
    /// `%`
    Percent,

    /// Unknown token, not expected by the lexer, e.g. "№"
    Unknown,

    /// End of input.
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocStyle {
    /// `;\ \;`
    Outer,
    /// `{ doc }`
    Inner,
}

/// Enum representing the literal types supported by the lexer.
///
/// Note that the suffix is *not* considered when deciding the `LiteralKind` in
/// this type. This means that float literals like `1f32` are classified by this
/// type as `Int`. (Compare against `rustc_ast::token::LitKind` and
/// `rustc_ast::ast::LitKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LiteralKind {
    /// `12`, `0x100`, `0b120i99`.
    Int { base: Base, empty_int: bool },
    /// `12.34`, `1e3`, but not `1f32`.
    Float { base: Base, empty_exponent: bool },
    /// `"abc"`, `"abc`
    Str { terminated: bool },
}

/// Base of numeric literal encoding according to its prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Base {
    /// Literal starts with "0b".
    Binary = 2,
    /// Literal starts with "0o".
    Octal = 8,
    /// Literal doesn't contain a prefix.
    Decimal = 10,
    /// Literal starts with "0x".
    Hexadecimal = 16,
}

/// Creates an iterator that produces tokens from the input string.
///
/// When parsing a full Papyrus document,
pub fn tokenize(input: &str) -> impl Iterator<Item = Token> {
    let mut cursor = Cursor::new(input);
    std::iter::from_fn(move || {
        let token = cursor.advance_token();
        if token.kind != TokenKind::Eof { Some(token) } else { None }
    })
}

/// True if `c` is considered a whitespace according to Rust language definition.
/// See [Rust language reference](https://doc.rust-lang.org/reference/whitespace.html)
/// for definitions of these classes.
pub fn is_whitespace(c: char) -> bool {
    // This is Pattern_White_Space.
    //
    // Note that this set is stable (ie, it doesn't change with different
    // Unicode versions), so it's ok to just hard-code the values.

    matches!(
        c,
        // End-of-line characters
        | '\u{000A}' // line feed (\n)
        | '\u{000B}' // vertical tab
        | '\u{000C}' // form feed
        | '\u{000D}' // carriage return (\r)
        | '\u{0085}' // next line (from latin1)
        | '\u{2028}' // LINE SEPARATOR
        | '\u{2029}' // PARAGRAPH SEPARATOR

        // `Default_Ignorable_Code_Point` characters
        | '\u{200E}' // LEFT-TO-RIGHT MARK
        | '\u{200F}' // RIGHT-TO-LEFT MARK

        // Horizontal space characters
        | '\u{0009}'   // tab (\t)
        | '\u{0020}' // space
    )
}

/// True if `c` is considered horizontal whitespace according to Rust language definition.
pub fn is_horizontal_whitespace(c: char) -> bool {
    // This is the horizontal space subset of `Pattern_White_Space` as
    // categorized by UAX #31, Section 4.1.
    //
    // Note that this set is stable (ie, it doesn't change with different
    // Unicode versions), so it's ok to just hard-code the values.

    matches!(
        c,
        // Horizontal space characters
        '\u{0009}'   // tab (\t)
        | '\u{0020}' // space
    )
}

/// True if `c` is valid as a first character of an identifier.
/// See [Papyrus language reference](https://ck.uesp.net/wiki/Identifier_Reference) for
/// a formal definition of valid identifier name.
pub fn is_id_start(c: char) -> bool {
    // This is XID_Start OR '_' (which formally is not a XID_Start).
    c == '_' || c.is_ascii_alphabetic()
}

/// True if `c` is valid as a non-first character of an identifier.
/// See [Papyrus language reference](https://ck.uesp.net/wiki/Identifier_Reference) for
/// a formal definition of valid identifier name.
pub fn is_id_continue(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// The passed string is lexically an identifier.
pub fn is_ident(string: &str) -> bool {
    let mut chars = string.chars();
    if let Some(start) = chars.next() {
        is_id_start(start) && chars.all(is_id_continue)
    } else {
        false
    }
}

/// Peekable iterator over a char sequence.
///
/// Next characters can be peeked via `first` method,
/// and position can be shifted forward via `bump` method.
pub struct Cursor<'a> {
    len_remaining: usize,
    /// Iterator over chars. Slightly faster than a &str.
    chars: Chars<'a>,
    #[cfg(debug_assertions)]
    prev: char,
}

const EOF_CHAR: char = '\0';

impl<'a> Cursor<'a> {
    pub fn new(input: &'a str) -> Cursor<'a> {
        Cursor {
            len_remaining: input.len(),
            chars: input.chars(),
            #[cfg(debug_assertions)]
            prev: EOF_CHAR,
        }
    }

    pub fn as_str(&self) -> &'a str {
        self.chars.as_str()
    }

    /// Returns the last eaten symbol (or `'\0'` in release builds).
    /// (For debug assertions only.)
    pub(crate) fn prev(&self) -> char {
        #[cfg(debug_assertions)]
        {
            self.prev
        }

        #[cfg(not(debug_assertions))]
        {
            EOF_CHAR
        }
    }

    /// Peeks the next symbol from the input stream without consuming it.
    /// If requested position doesn't exist, `EOF_CHAR` is returned.
    /// However, getting `EOF_CHAR` doesn't always mean actual end of file,
    /// it should be checked with `is_eof` method.
    pub fn first(&self) -> char {
        // `.next()` optimizes better than `.nth(0)`
        self.chars.clone().next().unwrap_or(EOF_CHAR)
    }

    /// Peeks the second symbol from the input stream without consuming it.
    pub(crate) fn second(&self) -> char {
        // `.next()` optimizes better than `.nth(1)`
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().unwrap_or(EOF_CHAR)
    }

    /// Peeks the third symbol from the input stream without consuming it.
    pub fn third(&self) -> char {
        // `.next()` optimizes better than `.nth(2)`
        let mut iter = self.chars.clone();
        iter.next();
        iter.next();
        iter.next().unwrap_or(EOF_CHAR)
    }

    /// Checks if there is nothing more to consume.
    pub(crate) fn is_eof(&self) -> bool {
        self.chars.as_str().is_empty()
    }

    /// Returns amount of already consumed symbols.
    pub(crate) fn pos_within_token(&self) -> u32 {
        (self.len_remaining - self.chars.as_str().len()) as u32
    }

    /// Resets the number of bytes consumed to 0.
    pub(crate) fn reset_pos_within_token(&mut self) {
        self.len_remaining = self.chars.as_str().len();
    }

    /// Moves to the next character.
    pub(crate) fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;

        #[cfg(debug_assertions)]
        {
            self.prev = c;
        }

        Some(c)
    }

    /// Eats symbols while predicate returns true or until the end of file is reached.
    pub(crate) fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        // It was tried making optimized version of this for eg. line comments, but
        // LLVM can inline all of this and compile it down to fast iteration over bytes.
        while predicate(self.first()) && !self.is_eof() {
            self.bump();
        }
    }

    pub(crate) fn eat_until(&mut self, byte: u8) {
        self.chars = match memchr::memchr(byte, self.as_str().as_bytes()) {
            Some(index) => self.as_str()[index..].chars(),
            None => "".chars(),
        }
    }

    /// Parses a token from the input string.
    pub fn advance_token(&mut self) -> Token {
        let Some(first_char) = self.bump() else {
            return Token::new(TokenKind::Eof, 0);
        };

        let token_kind = match first_char {
            // Slash, comment(`;`) or block comment(`;/ \;` or `{ }`).
            ';' => match self.first() {
                '/' => self.block_comment(),
                _ => self.line_comment(),
            },
            '{' => self.doc_comment(),

            // Whitespace sequence.
            c if is_whitespace(c) => self.whitespace(),

            // Identifier (this should be checked after other variant that can
            // start as identifier).
            c if is_id_start(c) => self.ident_or_unknown_prefix(),

            // Numeric literal.
            c @ '0'..='9' => {
                let literal_kind = self.number(c);
                let suffix_start = self.pos_within_token();
                self.eat_literal_suffix();
                TokenKind::Literal { kind: literal_kind, suffix_start }
            }

            // One-symbol tokens.
            // ';' => Semi,
            ',' => Comma,
            '.' => Dot,
            '(' => OpenParen,
            ')' => CloseParen,
            // '{' => OpenBrace,
            '}' => CloseBrace,
            '[' => OpenBracket,
            ']' => CloseBracket,
            '@' => At,
            '#' => Pound,
            '~' => Tilde,
            '?' => Question,
            ':' => Colon,
            '$' => Dollar,
            '=' => Eq,
            '!' => Bang,
            '<' => Lt,
            '>' => Gt,
            '-' => Minus,
            '&' => And,
            '|' => Or,
            '+' => Plus,
            '*' => Star,
            '^' => Caret,
            '%' => Percent,

            // String literal.
            '"' => {
                let terminated = self.double_quoted_string();
                let suffix_start = self.pos_within_token();
                if terminated {
                    self.eat_literal_suffix();
                }
                let kind = Str { terminated };
                Literal { kind, suffix_start }
            }
            // Identifier starting with an emoji. Only lexed for graceful error recovery.
            c if !c.is_ascii() && c.is_emoji_char() => self.invalid_ident(),
            _ => Unknown,
        };

        let res = Token::new(token_kind, self.pos_within_token());
        self.reset_pos_within_token();
        res
    }

    fn line_comment(&mut self) -> TokenKind {
        debug_assert!(self.prev() == ';' && self.first() != '/'); // non block comment
        self.bump();

        self.eat_until(b'\n');
        LineComment
    }

    fn block_comment(&mut self) -> TokenKind {
        debug_assert!(self.prev() == ';' && self.first() == '/');
        self.bump();

        let mut depth = 1usize;
        while let Some(c) = self.bump() {
            match c {
                ';' if self.first() == '/' => {
                    self.bump();
                    depth += 1;
                }
                '/' if self.first() == ';' => {
                    self.bump();
                    depth -= 1;
                    if depth == 0 {
                        // This block comment is closed, so for a construction like "/* */ */"
                        // there will be a successfully parsed block comment "/* */"
                        // and " */" will be processed separately.
                        break;
                    }
                }
                _ => (),
            }
        }

        BlockComment { terminated: depth == 0 }
    }

    fn doc_comment(&mut self) -> TokenKind {
        debug_assert!(self.prev() == '{');
        self.bump();

        let mut depth = 1usize;
        while let Some(c) = self.bump() {
            match c {
                '{' => {
                    self.bump();
                    depth += 1;
                }
                '}' => {
                    self.bump();
                    depth -= 1;
                    if depth == 0 {
                        // This block comment is closed, so for a construction like "/* */ */"
                        // there will be a successfully parsed block comment "/* */"
                        // and " */" will be processed separately.
                        break;
                    }
                }
                _ => (),
            }
        }

        DocComment { terminated: depth == 0 }
    }

    fn whitespace(&mut self) -> TokenKind {
        debug_assert!(is_whitespace(self.prev()));
        self.eat_while(is_whitespace);
        Whitespace
    }

    fn ident_or_unknown_prefix(&mut self) -> TokenKind {
        debug_assert!(is_id_start(self.prev()));
        // Start is already eaten, eat the rest of identifier.
        self.eat_while(is_id_continue);
        // Known prefixes must have been handled earlier. So if
        // we see a prefix here, it is definitely an unknown prefix.
        match self.first() {
            '#' | '"' | '\'' => UnknownPrefix,
            c if !c.is_ascii() && c.is_emoji_char() => self.invalid_ident(),
            _ => Ident,
        }
    }

    fn invalid_ident(&mut self) -> TokenKind {
        // Start is already eaten, eat the rest of identifier.
        self.eat_while(|c| {
            const ZERO_WIDTH_JOINER: char = '\u{200d}';
            is_id_continue(c) || (!c.is_ascii() && c.is_emoji_char()) || c == ZERO_WIDTH_JOINER
        });
        // An invalid identifier followed by '#' or '"' or '\'' could be
        // interpreted as an invalid literal prefix. We don't bother doing that
        // because the treatment of invalid identifiers and invalid prefixes
        // would be the same.
        InvalidIdent
    }

    fn number(&mut self, first_digit: char) -> LiteralKind {
        debug_assert!('0' <= self.prev() && self.prev() <= '9');
        let mut base = Base::Decimal;
        if first_digit == '0' {
            // Attempt to parse encoding base.
            match self.first() {
                'b' => {
                    base = Base::Binary;
                    self.bump();
                    if !self.eat_decimal_digits() {
                        return Int { base, empty_int: true };
                    }
                }
                'o' => {
                    base = Base::Octal;
                    self.bump();
                    if !self.eat_decimal_digits() {
                        return Int { base, empty_int: true };
                    }
                }
                'x' => {
                    base = Base::Hexadecimal;
                    self.bump();
                    if !self.eat_hexadecimal_digits() {
                        return Int { base, empty_int: true };
                    }
                }
                // Not a base prefix; consume additional digits.
                '0'..='9' | '_' => {
                    self.eat_decimal_digits();
                }

                // Also not a base prefix; nothing more to do here.
                '.' | 'e' | 'E' => {}

                // Just a 0.
                _ => return Int { base, empty_int: false },
            }
        } else {
            // No base prefix, parse number in the usual way.
            self.eat_decimal_digits();
        }

        match self.first() {
            // Don't be greedy if this is actually an
            // integer literal followed by field/method access or a range pattern
            // (`0..2` and `12.foo()`)
            '.' if self.second() != '.' && !is_id_start(self.second()) => {
                // might have stuff after the ., and if it does, it needs to start
                // with a number
                self.bump();
                let mut empty_exponent = false;
                if self.first().is_ascii_digit() {
                    self.eat_decimal_digits();
                    match self.first() {
                        'e' | 'E' => {
                            self.bump();
                            empty_exponent = !self.eat_float_exponent();
                        }
                        _ => (),
                    }
                }
                Float { base, empty_exponent }
            }
            'e' | 'E' => {
                self.bump();
                let empty_exponent = !self.eat_float_exponent();
                Float { base, empty_exponent }
            }
            _ => Int { base, empty_int: false },
        }
    }

    /// Eats double-quoted string and returns true
    /// if string is terminated.
    fn double_quoted_string(&mut self) -> bool {
        debug_assert!(self.prev() == '"');
        while let Some(c) = self.bump() {
            match c {
                '"' => {
                    return true;
                }
                '\\' if self.first() == '\\' || self.first() == '"' => {
                    // Bump again to skip escaped character.
                    self.bump();
                }
                _ => (),
            }
        }
        // End of file reached.
        false
    }

    fn eat_decimal_digits(&mut self) -> bool {
        let mut has_digits = false;
        loop {
            match self.first() {
                '_' => {
                    self.bump();
                }
                '0'..='9' => {
                    has_digits = true;
                    self.bump();
                }
                _ => break,
            }
        }
        has_digits
    }

    fn eat_hexadecimal_digits(&mut self) -> bool {
        let mut has_digits = false;
        loop {
            match self.first() {
                '_' => {
                    self.bump();
                }
                '0'..='9' | 'a'..='f' | 'A'..='F' => {
                    has_digits = true;
                    self.bump();
                }
                _ => break,
            }
        }
        has_digits
    }

    /// Eats the float exponent. Returns true if at least one digit was met,
    /// and returns false otherwise.
    fn eat_float_exponent(&mut self) -> bool {
        debug_assert!(self.prev() == 'e' || self.prev() == 'E');
        if self.first() == '-' || self.first() == '+' {
            self.bump();
        }
        self.eat_decimal_digits()
    }

    // Eats the suffix of the literal, e.g. "u8".
    fn eat_literal_suffix(&mut self) {
        self.eat_identifier();
    }

    // Eats the identifier. Note: succeeds on `_`, which isn't a valid
    // identifier.
    fn eat_identifier(&mut self) {
        if !is_id_start(self.first()) {
            return;
        }
        self.bump();

        self.eat_while(is_id_continue);
    }
}
