//! Typed AST support utilities.
//!
//! This module contains the only CST traversal helpers used by the AST view.
//! Higher-level AST nodes should avoid touching `Tree::children` directly.
// #![expect(unused)]

use papyrus_token::token::{Token, TokenKind};

use crate::{
    ast::AstCast,
    cst::{Child, Tree, TreeKind},
};

/// Returns the first child tree of the specified kind.
#[inline]
pub fn child<'a, T: AstCast<'a>>(tree: &'a Tree) -> Option<T> {
    tree.children.iter().find_map(|c| match c {
        Child::Tree(t) => T::cast(t),
        _ => None,
    })
}

/// Returns every direct child tree of the specified kind.
#[inline]
pub fn children<'a, T: AstCast<'a>>(tree: &'a Tree) -> impl Iterator<Item = T> + 'a {
    tree.children.iter().filter_map(|c| match c {
        Child::Tree(t) => T::cast(t),
        _ => None,
    })
}

/// Returns the first direct child token of the specified kind.
#[inline]
pub fn token(node: &Tree, kind: TokenKind) -> Option<&Token> {
    node.children.iter().find_map(|child| match child {
        Child::Token(token) if token.kind == kind => Some(token),
        _ => None,
    })
}

/// Returns every direct child token of the specified kind.
#[inline]
pub fn tokens(node: &Tree, kind: TokenKind) -> impl Iterator<Item = &Token> {
    node.children.iter().filter_map(move |child| match child {
        Child::Token(token) if token.kind == kind => Some(token),
        _ => None,
    })
}

/// Returns the first child tree matching a predicate.
#[inline]
pub fn child_by<F>(node: &Tree, mut pred: F) -> Option<&Tree>
where
    F: FnMut(&Tree) -> bool,
{
    node.children.iter().find_map(|child| match child {
        Child::Tree(tree) if pred(tree) => Some(tree),
        _ => None,
    })
}

/// Returns every child tree matching a predicate.
#[inline]
pub fn children_by<'a, F>(node: &'a Tree, mut pred: F) -> impl Iterator<Item = &'a Tree>
where
    F: FnMut(&Tree) -> bool + 'a,
{
    node.children.iter().filter_map(move |child| match child {
        Child::Tree(tree) if pred(tree) => Some(tree),
        _ => None,
    })
}

/// Returns the first child node regardless of kind.
#[inline]
pub fn first_tree(node: &Tree) -> Option<&Tree> {
    node.children.iter().find_map(|child| match child {
        Child::Tree(tree) => Some(tree),
        _ => None,
    })
}

/// Returns the first child node regardless of kind.
#[inline]
pub fn first_token(node: &Tree) -> Option<&Token> {
    node.children.iter().find_map(|child| match child {
        Child::Token(token) => Some(token),
        _ => None,
    })
}

/// Returns the last child node regardless of kind.
#[inline]
pub fn last_tree(node: &Tree) -> Option<&Tree> {
    node.children.iter().rev().find_map(|child| match child {
        Child::Tree(tree) => Some(tree),
        _ => None,
    })
}

/// Finds the first child expression.
#[inline]
pub fn expr(node: &Tree) -> Option<&Tree> {
    child_by(node, |tree| {
        matches!(
            tree.kind,
            TreeKind::ExprName
                | TreeKind::ExprLiteral
                | TreeKind::ExprParen
                | TreeKind::ExprUnary
                | TreeKind::ExprBinary
                | TreeKind::ExprCall
                | TreeKind::ExprMember
                | TreeKind::ExprIndex
        )
    })
}

/// Returns every child expression.
#[inline]
pub fn exprs(node: &Tree) -> impl Iterator<Item = &Tree> {
    children_by(node, |tree| {
        matches!(
            tree.kind,
            TreeKind::ExprName
                | TreeKind::ExprLiteral
                | TreeKind::ExprParen
                | TreeKind::ExprUnary
                | TreeKind::ExprBinary
                | TreeKind::ExprCall
                | TreeKind::ExprMember
                | TreeKind::ExprIndex
        )
    })
}

/// Returns the first statement.
#[inline]
pub fn stmt(node: &Tree) -> Option<&Tree> {
    child_by(node, |tree| {
        matches!(
            tree.kind,
            TreeKind::StmtExpr
                | TreeKind::StmtIf
                | TreeKind::StmtElseIf
                | TreeKind::StmtElse
                | TreeKind::StmtWhile
                | TreeKind::StmtReturn
        )
    })
}

/// Returns every statement.
#[inline]
pub fn stmts(node: &Tree) -> impl Iterator<Item = &Tree> {
    children_by(node, |tree| {
        matches!(
            tree.kind,
            TreeKind::StmtExpr
                | TreeKind::StmtIf
                | TreeKind::StmtElseIf
                | TreeKind::StmtElse
                | TreeKind::StmtWhile
                | TreeKind::StmtReturn
        )
    })
}
