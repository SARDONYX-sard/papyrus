//! A compact, lossless concrete syntax tree.
//!
//! The tree is stored as a single flat array of elements in pre-order. Each
//! element only records its kind, its byte range in the source, its parent and
//! the index one past its last descendant. No text is copied: the tree is
//! lifetime-free and owned, while the cursor types [`SyntaxNode`] and
//! [`SyntaxToken`] pair it with a borrowed `&'a str` of the source on demand.
//!
//! This layout makes the common operations cheap:
//! - descendants of a node are a contiguous index range;
//! - the next sibling of an element is the element right after its subtree;
//! - trees are `Send + Sync` and can be cached without self-references.

use std::fmt::{self, Write as _};

use text_size::{TextRange, TextSize};

use crate::SyntaxKind;

const NO_PARENT: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
struct Elem {
    kind: SyntaxKind,
    start: u32,
    end: u32,
    parent: u32,
    /// Index one past the last element of this element's subtree.
    subtree_end: u32,
}

/// The owned storage of a syntax tree, see the module documentation.
#[derive(Clone, Default)]
pub struct SyntaxTree {
    elems: Vec<Elem>,
}

impl SyntaxTree {
    /// Returns the root node, borrowing `text`, which must be the source this
    /// tree was built from.
    pub fn root<'a>(&'a self, text: &'a str) -> SyntaxNode<'a> {
        debug_assert!(!self.elems.is_empty(), "empty syntax tree");
        debug_assert_eq!(self.elems[0].end as usize, text.len(), "tree built from another text");
        SyntaxNode { tree: self, text, idx: 0 }
    }

    /// Number of elements (nodes and tokens) in the tree.
    pub fn len(&self) -> usize {
        self.elems.len()
    }
}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxTree").field("len", &self.elems.len()).finish()
    }
}

/// Incrementally builds a [`SyntaxTree`] in pre-order.
#[derive(Default)]
pub(crate) struct TreeBuilder {
    elems: Vec<Elem>,
    stack: Vec<u32>,
    offset: u32,
}

impl TreeBuilder {
    pub(crate) fn start_node(&mut self, kind: SyntaxKind) {
        let idx = self.elems.len() as u32;
        let parent = self.stack.last().copied().unwrap_or(NO_PARENT);
        self.elems.push(Elem {
            kind,
            start: self.offset,
            end: self.offset,
            parent,
            subtree_end: idx + 1,
        });
        self.stack.push(idx);
    }

    pub(crate) fn finish_node(&mut self) {
        let idx = self.stack.pop().expect("unbalanced finish_node") as usize;
        let len = self.elems.len() as u32;
        let elem = &mut self.elems[idx];
        elem.end = self.offset;
        elem.subtree_end = len;
    }

    pub(crate) fn token(&mut self, kind: SyntaxKind, len: u32) {
        let idx = self.elems.len() as u32;
        let parent = *self.stack.last().expect("token outside of any node");
        let start = self.offset;
        self.offset += len;
        self.elems.push(Elem { kind, start, end: self.offset, parent, subtree_end: idx + 1 });
    }

    pub(crate) fn is_inside_node(&self) -> bool {
        !self.stack.is_empty()
    }

    pub(crate) fn finish(self) -> SyntaxTree {
        assert!(self.stack.is_empty(), "unclosed nodes");
        SyntaxTree { elems: self.elems }
    }
}

/// A node of the syntax tree.
#[derive(Clone, Copy)]
pub struct SyntaxNode<'a> {
    tree: &'a SyntaxTree,
    text: &'a str,
    idx: u32,
}

/// A token (leaf) of the syntax tree.
#[derive(Clone, Copy)]
pub struct SyntaxToken<'a> {
    tree: &'a SyntaxTree,
    text: &'a str,
    idx: u32,
}

/// Either a [`SyntaxNode`] or a [`SyntaxToken`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxElement<'a> {
    /// An inner node.
    Node(SyntaxNode<'a>),
    /// A leaf token.
    Token(SyntaxToken<'a>),
}

macro_rules! common_impls {
    ($ty:ident) => {
        impl<'a> $ty<'a> {
            fn elem(&self) -> &'a Elem {
                &self.tree.elems[self.idx as usize]
            }

            /// The kind of this element.
            pub fn kind(&self) -> SyntaxKind {
                self.elem().kind
            }

            /// The byte range of this element in the source.
            pub fn text_range(&self) -> TextRange {
                let e = self.elem();
                TextRange::new(TextSize::new(e.start), TextSize::new(e.end))
            }

            /// The source text covered by this element, trivia included.
            pub fn text(&self) -> &'a str {
                let e = self.elem();
                &self.text[e.start as usize..e.end as usize]
            }

            /// The parent node, `None` for the root.
            pub fn parent(&self) -> Option<SyntaxNode<'a>> {
                let parent = self.elem().parent;
                (parent != NO_PARENT).then_some(SyntaxNode {
                    tree: self.tree,
                    text: self.text,
                    idx: parent,
                })
            }

            /// Iterates over the ancestors, starting with the parent.
            pub fn ancestors(&self) -> impl Iterator<Item = SyntaxNode<'a>> + use<'a> {
                std::iter::successors(self.parent(), |n| n.parent())
            }

            /// The next sibling element.
            pub fn next_sibling_or_token(&self) -> Option<SyntaxElement<'a>> {
                let e = self.elem();
                let next = e.subtree_end;
                let parent = e.parent;
                let next_elem = self.tree.elems.get(next as usize)?;
                (next_elem.parent == parent && parent != NO_PARENT)
                    .then(|| element(self.tree, self.text, next))
            }

            /// The previous sibling element.
            pub fn prev_sibling_or_token(&self) -> Option<SyntaxElement<'a>> {
                let parent = self.parent()?;
                let mut prev = None;
                for child in parent.children_with_tokens() {
                    if child.index() == self.idx {
                        return prev;
                    }
                    prev = Some(child);
                }
                None
            }

            /// The source text, used for diagnostics only.
            pub fn source(&self) -> &'a str {
                self.text
            }
        }

        impl PartialEq for $ty<'_> {
            fn eq(&self, other: &Self) -> bool {
                std::ptr::eq(self.tree, other.tree) && self.idx == other.idx
            }
        }

        impl Eq for $ty<'_> {}

        impl std::hash::Hash for $ty<'_> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.idx.hash(state);
            }
        }
    };
}

common_impls!(SyntaxNode);
common_impls!(SyntaxToken);

fn element<'a>(tree: &'a SyntaxTree, text: &'a str, idx: u32) -> SyntaxElement<'a> {
    if tree.elems[idx as usize].kind.is_token() {
        SyntaxElement::Token(SyntaxToken { tree, text, idx })
    } else {
        SyntaxElement::Node(SyntaxNode { tree, text, idx })
    }
}

impl<'a> SyntaxNode<'a> {
    /// The range of this node without leading and trailing trivia and newlines.
    pub fn trimmed_range(&self) -> TextRange {
        let mut tokens = self
            .descendant_tokens()
            .filter(|t| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE);
        match tokens.next() {
            Some(first) => {
                let last = tokens.last().unwrap_or(first);
                first.text_range().cover(last.text_range())
            }
            None => TextRange::empty(self.text_range().start()),
        }
    }

    /// Iterates over the direct children, nodes and tokens.
    pub fn children_with_tokens(&self) -> Children<'a> {
        let e = self.elem();
        Children { tree: self.tree, text: self.text, next: self.idx + 1, end: e.subtree_end }
    }

    /// Iterates over the direct child nodes.
    pub fn children(&self) -> impl Iterator<Item = SyntaxNode<'a>> + use<'a> {
        self.children_with_tokens().filter_map(SyntaxElement::into_node)
    }

    /// Iterates over the direct child tokens.
    pub fn child_tokens(&self) -> impl Iterator<Item = SyntaxToken<'a>> + use<'a> {
        self.children_with_tokens().filter_map(SyntaxElement::into_token)
    }

    /// The first direct child token of the given kind.
    pub fn child_token(&self, kind: SyntaxKind) -> Option<SyntaxToken<'a>> {
        self.child_tokens().find(|t| t.kind() == kind)
    }

    /// The first direct child node of the given kind.
    pub fn child_node(&self, kind: SyntaxKind) -> Option<SyntaxNode<'a>> {
        self.children().find(|n| n.kind() == kind)
    }

    /// Iterates over all descendants in pre-order, this node included.
    pub fn descendants_with_tokens(&self) -> impl Iterator<Item = SyntaxElement<'a>> + use<'a> {
        let (tree, text) = (self.tree, self.text);
        (self.idx..self.elem().subtree_end).map(move |i| element(tree, text, i))
    }

    /// Iterates over all descendant nodes in pre-order, this node included.
    pub fn descendants(&self) -> impl Iterator<Item = SyntaxNode<'a>> + use<'a> {
        self.descendants_with_tokens().filter_map(SyntaxElement::into_node)
    }

    /// Iterates over all descendant tokens in source order.
    pub fn descendant_tokens(&self) -> impl Iterator<Item = SyntaxToken<'a>> + use<'a> {
        self.descendants_with_tokens().filter_map(SyntaxElement::into_token)
    }

    /// The first token in this subtree.
    pub fn first_token(&self) -> Option<SyntaxToken<'a>> {
        self.descendant_tokens().next()
    }

    /// The last token in this subtree.
    pub fn last_token(&self) -> Option<SyntaxToken<'a>> {
        let (tree, text) = (self.tree, self.text);
        (self.idx..self.elem().subtree_end)
            .rev()
            .find(|&i| tree.elems[i as usize].kind.is_token())
            .map(|idx| SyntaxToken { tree, text, idx })
    }

    /// The next sibling node, skipping tokens.
    pub fn next_sibling(&self) -> Option<SyntaxNode<'a>> {
        let mut cur = self.next_sibling_or_token();
        while let Some(el) = cur {
            match el {
                SyntaxElement::Node(n) => return Some(n),
                SyntaxElement::Token(t) => cur = t.next_sibling_or_token(),
            }
        }
        None
    }

    /// Finds the token covering `offset`.
    ///
    /// When `offset` lies exactly between two tokens, both are returned, left
    /// first, so callers can pick the more interesting one.
    pub fn token_at_offset(&self, offset: TextSize) -> TokenAtOffset<SyntaxToken<'a>> {
        let range = self.text_range();
        if offset < range.start() || offset > range.end() {
            return TokenAtOffset::None;
        }

        let off = u32::from(offset);
        let elems = &self.tree.elems[self.idx as usize..self.elem().subtree_end as usize];
        // In pre-order, `start` is monotonic, and tokens are never empty, so the
        // token covering `offset` is the last token starting at or before it.
        let n = elems.partition_point(|e| e.start <= off);
        let Some(i) = (0..n).rev().find(|&i| elems[i].kind.is_token()) else {
            return TokenAtOffset::None;
        };
        let tok = SyntaxToken { tree: self.tree, text: self.text, idx: self.idx + i as u32 };
        if elems[i].start == off
            && let Some(prev) = tok.prev_token().filter(|p| p.idx > self.idx)
        {
            return TokenAtOffset::Between(prev, tok);
        }
        TokenAtOffset::Single(tok)
    }
    /// Returns the deepest node whose range covers `range`.
    pub fn covering_node(&self, range: TextRange) -> SyntaxNode<'a> {
        let mut node = *self;
        'outer: loop {
            for child in node.children() {
                if child.text_range().contains_range(range) {
                    node = child;
                    continue 'outer;
                }
            }
            return node;
        }
    }

    /// Renders the subtree as an indented, human readable dump, used in tests.
    pub fn debug_dump(&self) -> String {
        let mut out = String::new();
        let mut depth = 0usize;
        let mut stack: Vec<u32> = Vec::new();
        for el in self.descendants_with_tokens() {
            let idx = el.index();
            while let Some(&top) = stack.last() {
                if idx >= self.tree.elems[top as usize].subtree_end {
                    stack.pop();
                    depth -= 1;
                } else {
                    break;
                }
            }
            let _ = write!(out, "{:indent$}", "", indent = depth * 2);
            match el {
                SyntaxElement::Node(n) => {
                    let _ = writeln!(out, "{:?}@{:?}", n.kind(), n.text_range());
                    stack.push(idx);
                    depth += 1;
                }
                SyntaxElement::Token(t) => {
                    let _ = writeln!(out, "{:?}@{:?} {:?}", t.kind(), t.text_range(), t.text());
                }
            }
        }
        out
    }
}

impl<'a> SyntaxToken<'a> {
    /// The next token in source order, crossing node boundaries.
    pub fn next_token(&self) -> Option<SyntaxToken<'a>> {
        let tree = self.tree;
        (self.idx as usize + 1..tree.elems.len())
            .find(|&i| tree.elems[i].kind.is_token())
            .map(|i| SyntaxToken { tree, text: self.text, idx: i as u32 })
    }

    /// The previous token in source order, crossing node boundaries.
    pub fn prev_token(&self) -> Option<SyntaxToken<'a>> {
        let tree = self.tree;
        (0..self.idx as usize).rev().find(|&i| tree.elems[i].kind.is_token()).map(|i| SyntaxToken {
            tree,
            text: self.text,
            idx: i as u32,
        })
    }
}

impl<'a> SyntaxElement<'a> {
    /// The kind of this element.
    pub fn kind(&self) -> SyntaxKind {
        match self {
            SyntaxElement::Node(n) => n.kind(),
            SyntaxElement::Token(t) => t.kind(),
        }
    }

    /// The byte range of this element.
    pub fn text_range(&self) -> TextRange {
        match self {
            SyntaxElement::Node(n) => n.text_range(),
            SyntaxElement::Token(t) => t.text_range(),
        }
    }

    /// Returns the node, if this is one.
    pub fn into_node(self) -> Option<SyntaxNode<'a>> {
        match self {
            SyntaxElement::Node(n) => Some(n),
            SyntaxElement::Token(_) => None,
        }
    }

    /// Returns the token, if this is one.
    pub fn into_token(self) -> Option<SyntaxToken<'a>> {
        match self {
            SyntaxElement::Node(_) => None,
            SyntaxElement::Token(t) => Some(t),
        }
    }

    fn index(&self) -> u32 {
        match self {
            SyntaxElement::Node(n) => n.idx,
            SyntaxElement::Token(t) => t.idx,
        }
    }
}

/// Iterator over the direct children of a node.
#[derive(Clone)]
pub struct Children<'a> {
    tree: &'a SyntaxTree,
    text: &'a str,
    next: u32,
    end: u32,
}

impl<'a> Iterator for Children<'a> {
    type Item = SyntaxElement<'a>;

    fn next(&mut self) -> Option<SyntaxElement<'a>> {
        if self.next >= self.end {
            return None;
        }
        let idx = self.next;
        self.next = self.tree.elems[idx as usize].subtree_end;
        Some(element(self.tree, self.text, idx))
    }
}

/// The result of [`SyntaxNode::token_at_offset`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenAtOffset<T> {
    /// The offset is outside of the tree.
    None,
    /// The offset is strictly inside one token.
    Single(T),
    /// The offset is on the boundary between two tokens.
    Between(T, T),
}

impl<T> TokenAtOffset<T> {
    /// Picks the right token on a boundary.
    pub fn right_biased(self) -> Option<T> {
        match self {
            TokenAtOffset::None => None,
            TokenAtOffset::Single(t) | TokenAtOffset::Between(_, t) => Some(t),
        }
    }

    /// Picks the left token on a boundary.
    pub fn left_biased(self) -> Option<T> {
        match self {
            TokenAtOffset::None => None,
            TokenAtOffset::Single(t) | TokenAtOffset::Between(t, _) => Some(t),
        }
    }

    /// Picks the token with the highest `priority`, the left one on ties.
    pub fn max_by_key<K: Ord>(self, mut priority: impl FnMut(&T) -> K) -> Option<T> {
        match self {
            TokenAtOffset::None => None,
            TokenAtOffset::Single(t) => Some(t),
            TokenAtOffset::Between(l, r) => {
                if priority(&r) > priority(&l) {
                    Some(r)
                } else {
                    Some(l)
                }
            }
        }
    }
}

impl fmt::Debug for SyntaxNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}@{:?}", self.kind(), self.text_range())
    }
}

impl fmt::Debug for SyntaxToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}@{:?} {:?}", self.kind(), self.text_range(), self.text())
    }
}

impl fmt::Display for SyntaxNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text())
    }
}

impl fmt::Display for SyntaxToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text())
    }
}
