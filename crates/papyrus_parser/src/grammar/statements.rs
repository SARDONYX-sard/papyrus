//! Parsing of statements and blocks.

use super::*;

pub(crate) fn block(p: &mut Parser<'_>) {
    let m = p.start();

    while !at_block_end(p) && !p.at(EOF) {
        stmt(p);
    }

    m.complete(p, Block);
}

fn at_block_end(p: &Parser<'_>) -> bool {
    matches!(
        p.current(),
        T![EndIf]
            | T![Else]
            | T![ElseIf]
            | T![EndWhile]
            | T![EndFunction]
            | T![EndEvent]
            | T![EndProperty]
            | T![EndState]
    )
}

pub(crate) fn stmt(p: &mut Parser<'_>) {
    let m = p.start();

    match p.current() {
        T![If] => if_stmt(p, m),
        T![While] => while_stmt(p, m),
        T![Return] => return_stmt(p, m),

        _ if at_var_decl(p) => var_decl_stmt(p, m),

        _ => expr_stmt(p, m),
    }
}

fn at_var_decl(p: &Parser<'_>) -> bool {
    if !types::at_type(p) {
        return false;
    }

    p.nth_at(1, IDENT)
}

pub(super) fn var_decl_stmt(p: &mut Parser<'_>, m: Marker) {
    types::ty(p);

    name(p);

    if p.at(T![=]) {
        initializer(p);
    }

    m.complete(p, VarDeclStmt);
}

fn expr_stmt(p: &mut Parser<'_>, m: Marker) {
    expressions::expr(p);

    m.complete(p, ExprStmt);
}

fn return_stmt(p: &mut Parser<'_>, m: Marker) {
    p.bump(T![Return]);

    if expressions::is_expr_start(p) {
        expressions::expr(p);
    }

    m.complete(p, ReturnStmt);
}

fn if_stmt(p: &mut Parser<'_>, m: Marker) {
    assert!(p.at(T![If]));
    p.bump(T![If]);

    expressions::expr(p);

    block(p);

    while p.at(T![ElseIf]) {
        elseif_branch(p);
    }

    if p.at(T![Else]) {
        else_branch(p);
    }

    p.expect(T![EndIf]);

    m.complete(p, IfStmt);
}

fn elseif_branch(p: &mut Parser<'_>) {
    assert!(p.at(T![ElseIf]));
    let m = p.start();

    p.bump(T![ElseIf]);

    expressions::expr(p);

    block(p);

    m.complete(p, ElseIfBranch);
}

fn else_branch(p: &mut Parser<'_>) {
    assert!(p.at(T![Else]));
    let m = p.start();

    p.bump(T![Else]);

    block(p);

    m.complete(p, ElseBranch);
}

fn while_stmt(p: &mut Parser<'_>, m: Marker) {
    assert!(p.at(T![While]));
    p.bump(T![While]);

    expressions::expr(p);

    block(p);

    p.expect(T![EndWhile]);

    m.complete(p, WhileStmt);
}
