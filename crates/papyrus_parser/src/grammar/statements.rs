//! Statements inside function and event bodies.
//!
//! ```text
//! Block      = (Stmt | NEWLINE)*
//! Stmt       = VarDecl | AssignStmt | ExprStmt | ReturnStmt | IfStmt | WhileStmt
//! AssignStmt = Expr ('=' | '+=' | '-=' | '*=' | '/=' | '%=') Expr
//! ReturnStmt = 'Return' Expr?
//! IfStmt     = 'If' Expr Block ElseIfClause* ElseClause? 'EndIf'
//! ElseIfClause = 'ElseIf' Expr Block
//! ElseClause = 'Else' Block
//! WhileStmt  = 'While' Expr Block 'EndWhile'
//! ```

use super::{expressions, *};

/// Parses statements until a block terminator (see [`at_block_end`]).
pub(super) fn block(p: &mut Parser<'_>) {
    let m = p.open();
    loop {
        if p.eat(NEWLINE) {
            continue;
        }
        if at_block_end(p) {
            break;
        }
        stmt(p);
    }
    p.close(m, BLOCK);
}

fn stmt(p: &mut Parser<'_>) {
    match p.current() {
        IF_KW => if_stmt(p),
        WHILE_KW => while_stmt(p),
        RETURN_KW => return_stmt(p),
        _ if at_local_var_decl(p) => local_var_decl(p),
        _ => expr_or_assign_stmt(p),
    }
}

/// `Type name`, `Type[] name`, or a primitive type keyword, which cannot start
/// an expression.
fn at_local_var_decl(p: &Parser<'_>) -> bool {
    if p.current().is_primitive_type() {
        return true;
    }
    type_len_at(p, 0).is_some_and(|n| p.nth_at(n, IDENT))
}

fn local_var_decl(p: &mut Parser<'_>) {
    let m = p.open();
    ty(p);
    name(p);
    if p.at(EQ) {
        initializer(p);
    }
    eol(p);
    p.close(m, VAR_DECL);
}

fn expr_or_assign_stmt(p: &mut Parser<'_>) {
    let m = p.open();
    let start = p.position();
    if expressions::expr(p).is_none() {
        // The expression parser already reported the problem; make progress.
        if p.position() == start && !p.at_eol() {
            p.advance();
        }
        skip_to_eol(p);
        p.close(m, ERROR);
        return;
    }
    let kind = if p.current().is_assign_op() {
        p.advance();
        expressions::expr(p);
        ASSIGN_STMT
    } else {
        EXPR_STMT
    };
    eol(p);
    p.close(m, kind);
}

fn return_stmt(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance();
    if !p.at_eol() {
        expressions::expr(p);
    }
    eol(p);
    p.close(m, RETURN_STMT);
}

fn if_stmt(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance();
    condition(p);
    block(p);
    while p.at(ELSEIF_KW) {
        let clause = p.open();
        p.advance();
        condition(p);
        block(p);
        p.close(clause, ELSE_IF_CLAUSE);
    }
    if p.at(ELSE_KW) {
        let clause = p.open();
        p.advance();
        eol(p);
        block(p);
        p.close(clause, ELSE_CLAUSE);
    }
    if p.eat(ENDIF_KW) {
        eol(p);
    } else {
        p.error_expected("`EndIf`");
    }
    p.close(m, IF_STMT);
}

fn while_stmt(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance();
    condition(p);
    block(p);
    if p.eat(ENDWHILE_KW) {
        eol(p);
    } else {
        p.error_expected("`EndWhile`");
    }
    p.close(m, WHILE_STMT);
}

fn condition(p: &mut Parser<'_>) {
    if p.at_eol() {
        p.error_expected("a condition");
        return;
    }
    if expressions::expr(p).is_none() {
        skip_to_eol(p);
        return;
    }
    if p.at(EQ) {
        p.error_at_current("`=` is an assignment, use `==` to compare");
        skip_to_eol(p);
        return;
    }
    eol(p);
}
