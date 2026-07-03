use crate::ast::*;
use crate::ast_node;

ast_node! {
    Event => Event {

        child name: Name;

        child params: ParamList;

        child body: Block;

        children flags: Flag;
    }
}

impl<'a> Event<'a> {
    #[inline]
    pub fn has_body(&self) -> bool {
        self.body().is_some()
    }
}
