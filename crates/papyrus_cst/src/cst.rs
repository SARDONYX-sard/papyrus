//! Concrete Syntax Tree
//!
//! Token-preserving CST.
//! Similar to matklad's resilient LL parser, but stores [`TextSpan`]s instead
//! of source text, and uses the `forward_parent` offset trick (as in
//! rust-analyzer) rather than `Vec::insert` for left-recursive rewrapping.

use papyrus_token::{span::TextSpan, token::Token};

// ── TreeKind ──────────────────────────────────────────────────────────────────

/// Kind of a syntax tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeKind {
    // ── Errors ────────────────────────────────────────────────────────────────
    Error,

    // ── Top-level items ───────────────────────────────────────────────────────
    File,
    Import,
    Script,
    State,
    Property,
    Function,
    Event,

    // ── Types ────────────────────────────────────────────────────────────────
    /// A type reference, e.g. `Int`, `String`, `Actor`, or `Int[]`.
    Type,

    // ── Flags ────────────────────────────────────────────────────────────────
    /// Settings such as automatically performing initialization.
    /// Optionally sets at the far right of variable declarations, Script, Property, and StmtExpr. e.g., `auto`, `hidden`
    Flag,

    // ── Function / event pieces ───────────────────────────────────────────────
    ParamList,
    Param,

    // ── Statements ───────────────────────────────────────────────────────────
    Block,
    StmtIf,
    StmtElseIf,
    StmtElse,
    StmtWhile,
    StmtReturn,
    StmtExpr,

    // ── Expressions ───────────────────────────────────────────────────────────
    ExprName,
    ExprLiteral,
    ExprParen,
    ExprUnary,
    ExprBinary,
    ExprCall,
    ExprMember,
    ExprIndex,

    // ── Call / index pieces ───────────────────────────────────────────────────
    ArgList,
    Arg,
}

// ── Tree ─────────────────────────────────────────────────────────────────────

/// A node in the concrete syntax tree.
///
/// Every node owns its children.  Tokens are preserved exactly as the lexer
/// emitted them (including whitespace and error tokens stored as trivia).
#[derive(Debug)]
pub struct Tree {
    pub kind: TreeKind,
    /// Byte span in the source file that this node covers.
    ///
    /// Starts at `TextSpan::empty(0)` and is grown incrementally as children
    /// are attached during `build_tree`.
    pub span: TextSpan,
    pub children: Vec<Child>,
}

impl Tree {
    pub fn new(kind: TreeKind, span: TextSpan) -> Self {
        Self {
            kind,
            span,
            children: Vec::new(),
        }
    }

    pub fn push_tree(&mut self, tree: Tree) {
        self.children.push(Child::Tree(tree));
    }

    pub fn push_token(&mut self, token: Token) {
        self.children.push(Child::Token(token));
    }

    // ── Traversal ─────────────────────────────────────────────────────────────

    /// Iterate over all direct child *trees*, skipping tokens.
    pub fn child_trees(&self) -> impl Iterator<Item = &Tree> {
        self.children.iter().filter_map(|c| match c {
            Child::Tree(t) => Some(t),
            Child::Token(_) => None,
        })
    }

    /// Iterate over all direct child *tokens*, skipping sub-trees.
    pub fn child_tokens(&self) -> impl Iterator<Item = &Token> {
        self.children.iter().filter_map(|c| match c {
            Child::Token(t) => Some(t),
            Child::Tree(_) => None,
        })
    }

    /// Return the first direct child tree with `kind`, if any.
    pub fn find_child(&self, kind: TreeKind) -> Option<&Tree> {
        self.child_trees().find(|t| t.kind == kind)
    }

    /// Return all direct child trees with `kind`.
    pub fn find_children(&self, kind: TreeKind) -> impl Iterator<Item = &Tree> {
        self.child_trees().filter(move |t| t.kind == kind)
    }
}

// ── Child ─────────────────────────────────────────────────────────────────────

/// One child of a [`Tree`]: either a nested sub-tree or a leaf token.
#[derive(Debug)]
pub enum Child {
    Token(Token),
    Tree(Tree),
}

impl Child {
    /// The byte span covered by this child.
    pub fn span(&self) -> TextSpan {
        match self {
            Child::Token(tok) => tok.span,
            Child::Tree(tree) => tree.span,
        }
    }
}
