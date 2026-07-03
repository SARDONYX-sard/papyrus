use crate::ast::*;
use crate::ast_node;

ast_node! {
    Script => Script {

        child name: Name;

        child parent: Name;

        children flags: Flag;

        children imports: Import;

        children properties: Property;

        children functions: Function;

        children events: Event;

        children states: State;
    }
}

impl<'a> Script<'a> {
    #[inline]
    pub fn items(&self) -> impl Iterator<Item = Item<'a>> + 'a {
        self.syntax().child_trees().filter_map(Item::cast)
    }
}

ast_node! {
    Import => Import {

        child name: Name;
    }
}

ast_node! {
    Flag => Flag {
    }
}
