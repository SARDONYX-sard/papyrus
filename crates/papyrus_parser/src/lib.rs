//! Resilient parser for the Papyrus scripting language (Skyrim dialect).
//!
//! [`parse`] turns source text into a lossless [`SyntaxTree`]: every byte of the
//! input, comments and whitespace included, belongs to exactly one token, and
//! the parser never gives up: malformed input produces `ERROR` nodes plus
//! [`SyntaxError`]s, while the surrounding code still gets a sensible tree.
//!
//! The pipeline is:
//!
//! 1. [`papyrus_lexer`] splits the text into borrowed tokens;
//! 2. trivia is filtered out, newlines are kept (Papyrus is line oriented);
//! 3. the grammar emits open/close/advance events;
//! 4. the events are replayed together with the trivia into a flat tree.
//!
//! The [`ast`] module offers typed accessors on top of the untyped tree.
//!
//! # Example
//!
//! ```
//! use papyrus_parser::{ast::AstNode, parse};
//!
//! let parse = parse("ScriptName Foo extends Quest\nInt Function Bar() Global\nEndFunction\n");
//! assert!(parse.errors().is_empty());
//! let file = parse.source_file();
//! assert_eq!(file.header().unwrap().name().unwrap().text(), "Foo");
//! ```

pub mod ast;
mod grammar;
mod parser;
mod syntax_kind;
mod tree;

#[cfg(test)]
mod tests;

use std::fmt;

pub use text_size::{TextRange, TextSize};

pub use crate::{
    syntax_kind::SyntaxKind,
    tree::{Children, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, TokenAtOffset},
};

use crate::{
    parser::{Event, Parser, Tok},
    tree::TreeBuilder,
};

/// A syntax or lexical error with its location.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxError {
    message: Box<str>,
    range: TextRange,
}

impl SyntaxError {
    /// Creates a new error.
    pub fn new(message: impl Into<Box<str>>, range: TextRange) -> SyntaxError {
        SyntaxError { message: message.into(), range }
    }

    /// The error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The location of the error. May be empty, e.g. for a missing token.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.range, self.message)
    }
}

/// The result of parsing: a syntax tree borrowing its source, plus errors.
#[derive(Clone)]
pub struct Parse<'a> {
    text: &'a str,
    tree: SyntaxTree,
    errors: Vec<SyntaxError>,
}

impl<'a> Parse<'a> {
    /// The source text.
    pub fn text(&self) -> &'a str {
        self.text
    }

    /// The untyped root node.
    pub fn syntax_node(&self) -> SyntaxNode<'_> {
        self.tree.root(self.text)
    }

    /// The typed root node.
    pub fn source_file(&self) -> ast::SourceFile<'_> {
        ast::SourceFile::from_root(self.syntax_node())
    }

    /// Lexical and syntax errors, ordered by position.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// The tree storage, which can outlive the borrowed `Parse`.
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }

    /// Splits the parse into its owned tree and errors.
    pub fn into_parts(self) -> (SyntaxTree, Vec<SyntaxError>) {
        (self.tree, self.errors)
    }

    /// Renders the tree followed by the errors, used in tests.
    pub fn debug_dump(&self) -> String {
        let mut out = self.syntax_node().debug_dump();
        for e in &self.errors {
            out.push_str(&format!("error {:?}: {}\n", e.range, e.message));
        }
        out
    }
}

/// Parses a whole Papyrus script.
///
/// The text must be smaller than 4 GiB.
pub fn parse(text: &str) -> Parse<'_> {
    let mut all: Vec<(SyntaxKind, u32)> = Vec::with_capacity(text.len() / 4);
    let mut significant: Vec<Tok> = Vec::with_capacity(text.len() / 6);
    let mut errors = Vec::new();

    for token in papyrus_lexer::Lexer::new(text) {
        let kind = SyntaxKind::from_token(token.kind);
        let range = TextRange::at(token.offset.into(), token.len().into());
        if let Some(err) = token.error {
            errors.push(SyntaxError::new(err.message(), range));
        }
        all.push((kind, token.len()));
        if !kind.is_trivia() {
            significant.push(Tok { kind, range });
        }
    }

    let mut p = Parser::new(text, &significant);
    grammar::source_file(&mut p);
    let Parser { events, errors: parse_errors, .. } = p;

    errors.extend(parse_errors);
    errors.sort_by_key(|e| (e.range.start(), e.range.end()));

    let tree = build_tree(&all, events);
    Parse { text, tree, errors }
}

/// Replays parser `events` over all tokens, re-inserting trivia.
///
/// Trivia preceding a node is attached to the node's parent rather than to
/// the node itself, so that node ranges start at their first significant
/// token. Trailing trivia ends up in the innermost node open at that point.
fn build_tree(tokens: &[(SyntaxKind, u32)], events: Vec<Event>) -> SyntaxTree {
    let mut builder = TreeBuilder::default();
    let mut next = 0usize;
    let mut depth = 0usize;

    let flush_trivia = |builder: &mut TreeBuilder, next: &mut usize| {
        while let Some(&(kind, len)) = tokens.get(*next) {
            if !kind.is_trivia() {
                break;
            }
            builder.token(kind, len);
            *next += 1;
        }
    };

    for event in events {
        match event {
            Event::Open { kind } => {
                if builder.is_inside_node() {
                    flush_trivia(&mut builder, &mut next);
                }
                builder.start_node(kind);
                depth += 1;
            }
            Event::Close => {
                if depth == 1 {
                    // Closing the root: everything left is trailing trivia.
                    while let Some(&(kind, len)) = tokens.get(next) {
                        builder.token(kind, len);
                        next += 1;
                    }
                }
                builder.finish_node();
                depth -= 1;
            }
            Event::Advance => {
                flush_trivia(&mut builder, &mut next);
                let (kind, len) = tokens[next];
                builder.token(kind, len);
                next += 1;
            }
        }
    }
    debug_assert_eq!(next, tokens.len(), "not all tokens were consumed");
    builder.finish()
}
