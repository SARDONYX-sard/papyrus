use crate::ast::*;
use crate::ast_node;

ast_node! {
    Function => Function {

        child return_type: Type;

        child name: Name;

        child params: ParamList;

        child body: Block;

        children flags: Flag;
    }
}

ast_node! {
    ParamList => ParamList {

        children params: Param;
    }
}

ast_node! {
    Param => Param {

        child type_: Type;

        child name: Name;

        children flags: Flag;
    }
}

impl<'a> Function<'a> {
    #[inline]
    pub fn has_return_type(&self) -> bool {
        self.return_type().is_some()
    }

    #[inline]
    pub fn has_body(&self) -> bool {
        self.body().is_some()
    }

    #[inline]
    pub fn parameter_count(&self) -> usize {
        self.params().map(|p| p.params().count()).unwrap_or(0)
    }
}
