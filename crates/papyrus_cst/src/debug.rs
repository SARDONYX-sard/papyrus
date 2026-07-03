use std::fmt::Write;

use papyrus_token::{
    span::TextSpan,
    token::{Token, Trivia},
};

use crate::cst::{Child, Tree};

/// Pretty-print a CST together with the corresponding source text.
///
/// Example:
///
/// ```text
/// File [0..42] "Scriptname Test\n..."
///   Script [0..42] "Scriptname Test\n..."
///     Token(Scriptname) [0..10] "Scriptname"
///     Token(Identifier) [11..15] "Test"
///     Function [16..42] "Function Foo()..."
/// ```
pub fn dump_tree(tree: &Tree, source: &str) -> String {
    let mut out = String::new();
    dump_tree_inner(tree, source, 0, &mut out);
    out
}

fn dump_tree_inner(tree: &Tree, source: &str, indent: usize, out: &mut String) {
    let text = span_text(source, tree.span);

    let _ = writeln!(
        out,
        "{:indent$}{:?} [{}..{}] {:?}",
        "",
        tree.kind,
        tree.span.start,
        tree.span.end,
        text,
        indent = indent * 2,
    );

    for child in &tree.children {
        match child {
            Child::Tree(tree) => {
                dump_tree_inner(tree, source, indent + 1, out);
            }

            Child::Token(token) => {
                let text = span_text(source, token.span);

                let _ = writeln!(
                    out,
                    "{:indent$}{:?} [{}..{}] {:?}",
                    "",
                    token.kind,
                    token.span.start,
                    token.span.end,
                    text,
                    indent = (indent + 1) * 2,
                );
            }
        }
    }
}

/// Dump a CST.
///
/// Tree nodes show:
/// - kind
/// - span
///
/// Tokens show:
/// - kind
/// - span
/// - source text
///
/// Trivia attached to a token is also shown.
pub fn dump_tree_tokens_with_trivia(tree: &Tree, source: &str) -> String {
    let mut out = String::new();
    dump_tree_tokens_inner(tree, source, 0, true, &mut out);
    out
}

/// Dump a CST without trivia.
pub fn dump_tree_tokens(tree: &Tree, source: &str) -> String {
    let mut out = String::new();
    dump_tree_tokens_inner(tree, source, 0, false, &mut out);
    out
}

fn dump_tree_tokens_inner(
    tree: &Tree,
    source: &str,
    indent: usize,
    show_trivia: bool,
    out: &mut String,
) {
    let _ = writeln!(
        out,
        "{:indent$}{:?}(tree) [{}..{}]",
        "",
        tree.kind,
        tree.span.start,
        tree.span.end,
        indent = indent * 2,
    );

    for child in &tree.children {
        match child {
            Child::Tree(tree) => {
                dump_tree_tokens_inner(tree, source, indent + 1, show_trivia, out);
            }

            Child::Token(token) => {
                dump_token(token, source, indent + 1, show_trivia, out);
            }
        }
    }
}

fn dump_token(token: &Token, source: &str, indent: usize, show_trivia: bool, out: &mut String) {
    if show_trivia {
        for trivia in &token.leading_trivia {
            dump_trivia(trivia, source, indent, "L", out);
        }
    }

    let _ = writeln!(
        out,
        "{:indent$}{:?}(token) [{}..{}] {:?}",
        "",
        token.kind,
        token.span.start,
        token.span.end,
        span_text(source, token.span),
        indent = indent * 2,
    );

    if show_trivia {
        for trivia in &token.trailing_trivia {
            dump_trivia(trivia, source, indent, "T", out);
        }
    }
}

fn dump_trivia(trivia: &Trivia, source: &str, indent: usize, label: &str, out: &mut String) {
    let _ = writeln!(
        out,
        "{:indent$}<{}:{:?}> [{}..{}] {:?}",
        "",
        label,
        trivia.kind,
        trivia.span.start,
        trivia.span.end,
        span_text(source, trivia.span),
        indent = indent * 2,
    );
}

fn span_text(source: &str, span: TextSpan) -> &str {
    let start = span.start as usize;
    let end = span.end as usize;

    if start <= end && end <= source.len() {
        &source[start..end]
    } else {
        "<invalid span>"
    }
}
