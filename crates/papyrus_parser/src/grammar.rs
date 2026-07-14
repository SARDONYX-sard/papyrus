//! This is the actual "grammar" of the Papyrus language.
//!
//! Each function in this module and its children corresponds
//! to a production of the formal grammar. Submodules roughly
//! correspond to different *areas* of the grammar. By convention,
//! each submodule starts with `use super::*` import and exports
//! "public" productions via `pub(super)`.
//!
//! See docs for [`Parser`] to learn about API,
//! available to the grammar, and see docs for [`Event`](super::event::Event)
//! to learn how this actually manages to produce parse trees.
//!
//! Code in this module also contains inline tests, which start with
//! `// test name-of-the-test` comment and look like this:
//!
//! ```text
//! // test function_with_zero_parameters
//! // fn foo() {}
//! ```
//!
//! After adding a new inline-test, run `cargo test -p xtask` to
//! extract it as a standalone text-fixture into
//! `crates/syntax/test_data/parser/`, and run `cargo test` once to
//! create the "gold" value.
//!
//! Coding convention: rules like `where_clause` always produce either a
//! node or an error, rules like `opt_where_clause` may produce nothing.
//! Non-opt rules typically start with `assert!(p.at(FIRST_TOKEN))`, the
//! caller is responsible for branching on the first token.

pub(crate) mod expressions;
mod flags;
mod header;
pub(crate) mod items;
mod params;
pub(crate) mod statements;
pub(crate) mod types;

use crate::{
    SyntaxKind::{self, *},
    T, TokenSet,
    parser::{CompletedMarker, Marker, Parser},
};

/// If a return type is defined, return true.
fn opt_return_type(p: &mut Parser<'_>) -> bool {
    if types::at_type(p) {
        let m = p.start();
        types::ty(p);
        m.complete(p, ReturnType);
        true
    } else {
        false
    }
}

/// define ident + err recovery
fn name_r(p: &mut Parser<'_>, recovery: TokenSet) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, Name);
    } else {
        p.err_recover("expected a name", recovery);
    }
}

/// Define identifier
fn name(p: &mut Parser<'_>) {
    name_r(p, TokenSet::EMPTY);
}

/// method access
fn name_ref_or_self(p: &mut Parser<'_>) {
    if matches!(p.current(), T![ident] | T![Self] | T![Parent]) {
        let m = p.start();
        p.bump_any();
        m.complete(p, NameRef);
    } else {
        p.err_and_bump("expected identifier, `self` or `parent`");
    }
}

/// `= <Expr>`
fn initializer(p: &mut Parser<'_>) {
    let m = p.start();

    p.expect(T![=]);
    expressions::expr(p);

    m.complete(p, Initializer);
}

/// The `parser` passed this is required to at least consume one token if it returns `true`.
/// If the `parser` returns false, parsing will stop.
fn delimited(
    p: &mut Parser<'_>,
    bra: SyntaxKind,
    ket: SyntaxKind,
    delim: SyntaxKind,
    unexpected_delim_message: impl Fn() -> String,
    first_set: TokenSet,
    mut parser: impl FnMut(&mut Parser<'_>) -> bool,
) {
    p.bump(bra);
    while !p.at(ket) && !p.at(EOF) {
        if p.at(delim) {
            // Recover if an argument is missing and only got a delimiter,
            // e.g. `(a, , b)`.

            // Wrap the erroneous delimiter in an error node so that fixup logic gets rid of it.
            // FIXME: Ideally this should be handled in fixup in a structured way, but our list
            // nodes currently have no concept of a missing node between two delimiters.
            // So doing it this way is easier.
            let m = p.start();
            p.error(unexpected_delim_message());
            p.bump(delim);
            m.complete(p, ERROR);
            continue;
        }
        if !parser(p) {
            break;
        }
        if !p.eat(delim) {
            if p.at_ts(first_set) {
                p.error(format!("expected {delim:?}"));
            } else {
                break;
            }
        }
    }
    p.expect(ket);
}
