use crate::ast::*;
use crate::{ast_enum, ast_node};

ast_node! {
    File => File {

        child script: Script;
    }
}

impl<'a> File<'a> {
    #[inline]
    pub fn items(&self) -> impl Iterator<Item = Item<'a>> + 'a + use<'a> {
        self.script()
            .into_iter()
            .flat_map(|s| s.items().collect::<Vec<_>>())
    }
}

ast_enum! {
    Item {
        Import(Import),
        Property(Property),
        Function(Function),
        Event(Event),
        State(State),
    }
}
