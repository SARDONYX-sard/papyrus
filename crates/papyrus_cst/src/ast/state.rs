use crate::ast::*;
use crate::ast_node;

ast_node! {
    State => State {

        child name: Name;

        children functions: Function;

        children events: Event;
    }
}

impl<'a> State<'a> {
    #[inline]
    pub fn items(&self) -> impl Iterator<Item = Item<'a>> + 'a {
        self.syntax().child_trees().filter_map(Item::cast)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items().next().is_none()
    }
}
