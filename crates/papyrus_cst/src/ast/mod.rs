mod block;
mod event;
mod expr;
mod file;
mod function;
mod name;
mod property;
mod script;
mod state;
mod stmt;
pub mod support;
mod type_;

pub use block::*;
pub use event::*;
pub use expr::*;
pub use file::*;
pub use function::*;
pub use name::*;
pub use property::*;
pub use script::*;
pub use state::*;
pub use stmt::*;
pub use type_::*;

use crate::cst::{Tree, TreeKind};

pub trait AstCast<'a>: Sized {
    fn cast(node: &'a Tree) -> Option<Self>;
}

pub trait AstNode<'a>: AstCast<'a> {
    const KIND: TreeKind;

    fn syntax(&self) -> &'a Tree;
}

#[macro_export]
macro_rules! ast_node {
    (
        $(#[$meta:meta])*
        $name:ident => $kind:ident {
            $(
                child $child_name:ident : $child_ty:ident;
            )*

            $(
                children $children_name:ident : $children_ty:ident;
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $name<'a> {
            syntax: &'a $crate::cst::Tree,
        }

        impl<'a> $crate::ast::AstCast<'a> for $name<'a> {
            #[inline]
            fn cast(node: &'a $crate::cst::Tree) -> Option<Self> {
                if node.kind == Self::KIND {
                    Some(Self { syntax: node })
                } else {
                    None
                }
            }
        }

        impl<'a> $crate::ast::AstNode<'a> for $name<'a> {
            const KIND: $crate::cst::TreeKind = $crate::cst::TreeKind::$kind;

            #[inline]
            fn syntax(&self) -> &'a $crate::cst::Tree {
                self.syntax
            }
        }

        impl<'a> $name<'a> {
            $(
                #[inline]
                pub fn $child_name(&self) -> Option<$child_ty<'a>> {
                    $crate::ast::support::child(self.syntax())
                }
            )*

            $(
                #[inline]
                pub fn $children_name(
                    &self,
                ) -> impl Iterator<Item = $children_ty<'a>> + 'a {
                    $crate::ast::support::children(self.syntax())
                }
            )*
        }
    };
}

#[macro_export]
macro_rules! ast_enum {
    (
        $name:ident {
            $(
                $variant:ident($ty:ident)
            ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum $name<'a> {
            $(
                $variant($ty<'a>),
            )*
        }

        impl<'a> AstCast<'a> for $name<'a> {
            fn cast(node: &'a Tree) -> Option<Self> {
                $(
                    if let Some(node) = $ty::cast(node) {
                        return Some(Self::$variant(node));
                    }
                )*

                None
            }
        }
    };
}
