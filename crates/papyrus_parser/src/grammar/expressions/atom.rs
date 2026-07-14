//! Parsing of atomic expressions.
//!
//! This module intentionally mirrors rust-analyzer's `expressions/atom.rs`.
//! Postfix operators (call, field, index, cast, ...) are parsed by the caller.

use self::types;
use super::*;

pub(super) const ATOM_FIRST: TokenSet = TokenSet::new(&[
    IDENT,
    INT_NUMBER,
    FLOAT_NUMBER,
    STRING,
    T![True],
    T![False],
    T![None],
    T![New],
    T!['('],
]);

/// Recovery tokens when parsing an expression.
///
/// Examples:
/// - fn
/// ```papyrus
/// Int x =
/// EndFunction
/// ```
///
/// - If
/// ```papyrus
/// If
/// EndIf
/// ```
///
/// - field
/// ```papyrus
/// Debug.Trace(, 1)
/// ```
///
/// - array
/// ```papyrus
/// arr[]
/// ```
pub(super) const EXPR_RECOVERY_SET: TokenSet = TokenSet::new(&[
    T![')'],
    T![']'],
    T![,],
    T![Else],
    T![ElseIf],
    T![EndIf],
    T![EndWhile],
    T![EndFunction],
    T![EndEvent],
    T![EndProperty],
    T![EndState],
]);

pub(super) fn atom_expr(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.at_ts(ATOM_FIRST) {
        p.err_recover("expected expression", EXPR_RECOVERY_SET);
        return None;
    }

    match p.current() {
        IDENT => name_ref_expr(p),
        INT_NUMBER | FLOAT_NUMBER | STRING | T![True] | T![False] | T![None] => literal(p),
        T![New] => Some(new_expr(p)),
        T!['('] => Some(paren_expr(p)),

        _ => {
            p.err_recover("expected expression", EXPR_RECOVERY_SET);
            None
        }
    }
}

fn name_ref_expr(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let p: &mut Parser<'_> = p;
    if matches!(p.current(), T![ident] | T![Self] | T![Parent]) {
        let m = p.start();
        p.bump_any();
        Some(m.complete(p, NameRef))
    } else {
        p.err_and_bump("expected identifier, `self` or `parent`");
        None
    }
}

// test expr_literals
// Function foo()
//     _ = true
//     _ = false
//     _ = 1
//     _ = 2.0
//     _ = "c"
// EndFunction
pub(crate) const LITERAL_FIRST: TokenSet =
    TokenSet::new(&[T![True], T![False], INT_NUMBER, FLOAT_NUMBER, STRING]);

pub(crate) fn literal(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if !p.at_ts(LITERAL_FIRST) {
        return None;
    }
    let m = p.start();
    p.bump_any();
    Some(m.complete(p, Literal))
}

fn paren_expr(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(T!['(']));

    let m = p.start();

    p.bump(T!['(']);

    expr(p);

    p.expect(T![')']);

    m.complete(p, ParenExpr)
}

fn new_expr(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(T![New]));

    let m = p.start();

    p.bump(T![New]);

    types::ty(p);

    m.complete(p, NewExpr)
}
