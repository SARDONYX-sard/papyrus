//! Expressions, parsed with precedence climbing.
//!
//! From loosest to tightest binding:
//!
//! | Operators                      | Associativity |
//! |--------------------------------|---------------|
//! | `\|\|`                         | left          |
//! | `&&`                           | left          |
//! | `==` `!=` `<` `<=` `>` `>=`    | left          |
//! | `+` `-`                        | left          |
//! | `*` `/` `%`                    | left          |
//! | unary `-` `!`                  | prefix        |
//! | `As`                           | postfix       |
//! | `.` `()` `[]`                  | postfix       |

use super::*;
use crate::{SyntaxKind, parser::MarkClosed};

/// Parses an expression. Returns `None` (after reporting an error) when no
/// expression starts at the current token.
pub(super) fn expr(p: &mut Parser<'_>) -> Option<MarkClosed> {
    expr_bp(p, 0)
}

fn infix_binding_power(kind: SyntaxKind) -> u8 {
    match kind {
        PIPE2 => 1,
        AMP2 => 2,
        EQ2 | NEQ | LT | LTEQ | GT | GTEQ => 3,
        PLUS | MINUS => 4,
        STAR | SLASH | PERCENT => 5,
        _ => 0,
    }
}

fn expr_bp(p: &mut Parser<'_>, min_bp: u8) -> Option<MarkClosed> {
    let mut lhs = prefix_expr(p)?;
    loop {
        let bp = infix_binding_power(p.current());
        if bp == 0 || bp <= min_bp {
            break;
        }
        let m = p.open_before(lhs);
        p.advance();
        if p.at_eol() {
            p.error_expected("an expression after the operator");
        } else {
            expr_bp(p, bp);
        }
        lhs = p.close(m, BIN_EXPR);
    }
    Some(lhs)
}

fn prefix_expr(p: &mut Parser<'_>) -> Option<MarkClosed> {
    if p.at(MINUS) || p.at(BANG) {
        let m = p.open();
        p.advance();
        prefix_expr(p);
        return Some(p.close(m, PREFIX_EXPR));
    }
    cast_expr(p)
}

fn cast_expr(p: &mut Parser<'_>) -> Option<MarkClosed> {
    let mut lhs = postfix_expr(p)?;
    while p.at(AS_KW) {
        let m = p.open_before(lhs);
        p.advance();
        ty(p);
        lhs = p.close(m, CAST_EXPR);
    }
    Some(lhs)
}

fn postfix_expr(p: &mut Parser<'_>) -> Option<MarkClosed> {
    let mut lhs = atom(p)?;
    loop {
        match p.current() {
            DOT => {
                let m = p.open_before(lhs);
                p.advance();
                if p.at(IDENT) {
                    name_ref(p);
                } else {
                    p.error_expected("a member name");
                }
                lhs = p.close(m, FIELD_EXPR);
            }
            L_PAREN => {
                let m = p.open_before(lhs);
                arg_list(p);
                lhs = p.close(m, CALL_EXPR);
            }
            L_BRACK => {
                let m = p.open_before(lhs);
                p.advance();
                expr(p);
                p.expect(R_BRACK);
                lhs = p.close(m, INDEX_EXPR);
            }
            _ => return Some(lhs),
        }
    }
}

fn atom(p: &mut Parser<'_>) -> Option<MarkClosed> {
    let m = p.open();
    let kind = match p.current() {
        INT_NUMBER | FLOAT_NUMBER | STRING | TRUE_KW | FALSE_KW | NONE_KW => {
            p.advance();
            LITERAL
        }
        IDENT | SELF_KW | PARENT_KW => {
            p.advance();
            NAME_REF
        }
        L_PAREN => {
            p.advance();
            expr(p);
            p.expect(R_PAREN);
            PAREN_EXPR
        }
        NEW_KW => {
            p.advance();
            base_ty(p);
            if p.expect(L_BRACK) {
                expr(p);
                p.expect(R_BRACK);
            }
            NEW_EXPR
        }
        _ => {
            p.error_expected("an expression");
            // Leave delimiters and line ends to the enclosing construct.
            let at_delimiter =
                p.at_eol() || p.at_any(&[R_PAREN, R_BRACK, COMMA]) || p.current().is_assign_op();
            if !at_delimiter && !at_block_end(p) {
                p.advance();
            }
            p.close(m, ERROR);
            return None;
        }
    };
    Some(p.close(m, kind))
}

/// `'(' (Arg (',' Arg)*)? ')'` where `Arg = (NameRef '=')? Expr`.
fn arg_list(p: &mut Parser<'_>) {
    let m = p.open();
    p.advance(); // `(`
    while !p.at(R_PAREN) && !p.at_eol() {
        let arg = p.open();
        if p.at(IDENT) && p.nth_at(1, EQ) {
            name_ref(p);
            p.advance();
        }
        let ok = expr(p).is_some();
        p.close(arg, ARG);
        if p.eat(COMMA) {
            continue;
        }
        if p.at(R_PAREN) || p.at_eol() {
            break;
        }
        if ok {
            p.error_expected("`,` or `)`");
        }
        // Skip garbage up to the next argument or the closing parenthesis.
        let err = p.open();
        while !p.at_eol() && !p.at(R_PAREN) && !p.at(COMMA) {
            p.advance();
        }
        p.close(err, ERROR);
        p.eat(COMMA);
    }
    p.expect(R_PAREN);
    p.close(m, ARG_LIST);
}
