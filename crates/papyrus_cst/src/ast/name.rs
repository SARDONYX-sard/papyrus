use crate::ast::*;
use crate::ast_node;

ast_node! {
    Name => ExprName {
    }
}

impl<'a> Name<'a> {
    /// Returns the identifier token if present.
    #[inline]
    pub fn ident(&self) -> Option<&'a papyrus_token::token::Token> {
        support::first_token(self.syntax())
    }

    #[inline]
    pub fn is_missing(&self) -> bool {
        self.ident().is_none()
    }
}
