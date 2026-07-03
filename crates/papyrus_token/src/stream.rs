use crate::{
    scanner::Scanner,
    token::{RawToken, Token, TokenKind, Trivia},
};

#[derive(Debug, Clone)]
pub struct TokenStream {
    tokens: Vec<Token>,
    /// token's index
    pos: usize,
}

impl TokenStream {
    pub fn new(mut scanner: Scanner) -> Self {
        let mut builder = TokenBuilder::new();
        let mut tokens = Vec::new();

        while let Some(raw) = scanner.next_raw() {
            if let Some(tok) = builder.push(raw) {
                tokens.push(tok);
            }
        }

        if let Some(last) = builder.finish() {
            tokens.push(last);
        }

        Self { tokens, pos: 0 }
    }

    #[inline]
    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    #[inline]
    pub fn peek_n(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// Get the token for the current location and move to the next location.
    #[inline]
    pub fn bump(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok.cloned()
    }

    /// Is the token at the current position the expected token?
    #[inline]
    pub fn at(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|t| t.kind == kind)
    }

    #[inline]
    pub fn is_eof(&self) -> bool {
        self.peek().is_none_or(|t| t.kind == TokenKind::Eof)
    }

    #[inline]
    pub fn into_tokens(self) -> Vec<Token> {
        self.tokens
    }
}

impl From<&str> for TokenStream {
    #[inline]
    fn from(src: &str) -> Self {
        TokenStream::new(Scanner::new(src))
    }
}

/// This is used to determine the Trivia's affiliation.
/// This adds an extra loop for this purpose.
#[derive(Default)]
pub struct TokenBuilder {
    pending_trivia: Vec<Trivia>,
    last: Option<Token>,
}

impl TokenBuilder {
    #[inline]
    const fn new() -> Self {
        Self {
            pending_trivia: Vec::new(),
            last: None,
        }
    }

    fn push(&mut self, raw: RawToken) -> Option<Token> {
        match raw {
            RawToken::Trivia(t) => {
                self.pending_trivia.push(t);
                None
            }

            RawToken::Token(core) => {
                let token = Token {
                    kind: core.kind,
                    span: core.span,
                    leading_trivia: std::mem::take(&mut self.pending_trivia),
                    trailing_trivia: vec![],
                };

                // flush previous token ONLY ONCE
                if let Some(prev) = self.last.take() {
                    self.last = Some(token);
                    return Some(prev);
                }

                self.last = Some(token);
                None
            }
        }
    }

    pub fn finish(&mut self) -> Option<Token> {
        self.last.take()
    }
}

#[cfg(test)]
mod tests {
    use crate::stream::TokenStream;

    #[test]
    fn debug_tokens() {
        let src = include_str!("../../../tests/simple/test.psc");
        let tokens = TokenStream::from(src).into_tokens();
        std::fs::write("../../target/tokens.log", format!("{tokens:#?}")).unwrap();
    }
}
