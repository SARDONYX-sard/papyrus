use crate::ast::*;
use crate::ast_node;

ast_node! {
    Property => Property {

        child type_: Type;

        child name: Name;

        child body: Block;

        children flags: Flag;
    }
}

impl<'a> Property<'a> {
    #[inline]
    pub fn has_body(&self) -> bool {
        self.body().is_some()
    }

    #[inline]
    pub fn is_auto(&self) -> bool {
        self.flags().next().is_some()
    }
}
