use crate::ast::*;
use crate::ast_node;

ast_node! {
    Type => Type {
    }
}

impl<'a> Type<'a> {
    #[inline]
    pub fn is_array(&self) -> bool {
        // TODO: Check for [] tokens once token helpers are implemented.
        false
    }

    #[inline]
    pub fn name(&self) -> Option<Name<'a>> {
        support::child(self.syntax())
    }
}
