use crate::{
    ast::{AstNode, Statement},
    ast_node,
};

ast_node! {
    /// Statement block.
    Block => Block {
        children statements: Statement;
    }
}

impl<'a> Block<'a> {
    /// Returns true if the block contains no statements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.statements().next().is_none()
    }
}
