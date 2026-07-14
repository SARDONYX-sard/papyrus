//! Pratt parser for expressions.
//!
//! This module intentionally follows the structure of rust-analyzer's
//! `grammar/expressions.rs`.

mod atom;

use crate::grammar::params::arg_list;

use super::*;

const PREFIX_BP: u8 = 255;

pub(crate) fn expr(p: &mut Parser<'_>) {
    expr_bp(p, 1);
}

pub(super) const EXPR_FIRST: TokenSet = atom::ATOM_FIRST.union(TokenSet::new(&[T![-], T![!]]));

pub(super) fn is_expr_start(p: &Parser<'_>) -> bool {
    atom::ATOM_FIRST.contains(p.current()) || matches!(p.current(), T![-] | T![!])
}

fn expr_bp(p: &mut Parser<'_>, min_bp: u8) -> Option<CompletedMarker> {
    let mut lhs = lhs(p)?;

    lhs = postfix_expr(p, lhs);

    while let Some((l_bp, r_bp, op, kind)) = current_op(p) {
        if l_bp < min_bp {
            break;
        }

        let m = lhs.precede(p);
        p.bump(op);

        let _rhs = expr_bp(p, r_bp);

        lhs = m.complete(p, kind);
    }

    Some(lhs)
}

fn lhs(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if matches!(p.current(), T![-] | T![!]) {
        let m = p.start();

        p.bump_any();

        expr_bp(p, PREFIX_BP);

        return Some(m.complete(p, PrefixExpr));
    }

    atom::atom_expr(p)
}

fn postfix_expr(p: &mut Parser<'_>, mut lhs: CompletedMarker) -> CompletedMarker {
    loop {
        lhs = match p.current() {
            T!['('] => call_expr(p, lhs),
            T!['['] => index_expr(p, lhs),
            T![.] => field_expr(p, lhs),
            T![As] => cast_expr(p, lhs),

            _ => break,
        };
    }

    lhs
}

// test call_expr
// function foo()
//     _ = f()
//     _ = f()(1)(1, 2,)
// endFunction
fn call_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(T!['(']));
    let m = lhs.precede(p);
    arg_list(p);
    m.complete(p, CallExpr)
}

// test index_expr
// function foo()
//     x[1][2]
// endFunction
fn index_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(T!['[']));
    let m = lhs.precede(p);
    p.bump(T!['[']);

    expr(p);
    p.expect(T![']']);
    m.complete(p, IndexExpr)
}

fn cast_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(T![As]));
    let m = lhs.precede(p);
    p.bump(T![As]);

    types::ty(p);
    m.complete(p, CastExpr)
}

// test field_expr
// Function foo()
//     obj.field
//     self.value
//     parent.data
// EndFunction
fn field_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(T![.]));

    let m = lhs.precede(p);

    p.bump(T![.]);

    name_ref_or_self(p);

    m.complete(p, FieldExpr)
}

/// Returns the binary operator at the current parser position.
///
/// # Multi-token Operators
///
/// - `rustc_lexer` tokenizes operators such as `||` as separate `|` tokens.
/// - `papyrus_lexer` follows the same behavior because it is forked from
///   `rustc_lexer`.
/// - Multi-token operators are detected by looking ahead one token.
/// - The `at()` check performs the internal lookahead required to distinguish
///   operators such as `|` from `||`.
fn current_op(p: &Parser<'_>) -> Option<(u8, u8, SyntaxKind, SyntaxKind)> {
    Some(match p.current() {
        T![|] if p.at(T![||]) => (1, 2, T![||], BinExpr),
        T![&] if p.at(T![&&]) => (3, 4, T![&&], BinExpr),

        T![=] if p.at(T![==]) => (5, 6, T![==], BinExpr),
        T![!] if p.at(T![!=]) => (5, 6, T![!=], BinExpr),

        T![<] if p.at(T![<=]) => (7, 8, T![<=], BinExpr),
        T![<] if p.at(T![<<]) => (13, 14, T![<<], BinExpr),
        T![<] => (7, 8, T![<], BinExpr),

        T![>] if p.at(T![>=]) => (7, 8, T![>=], BinExpr),
        T![>] if p.at(T![>>]) => (13, 14, T![>>], BinExpr),
        T![>] => (7, 8, T![>], BinExpr),

        T![|] => (9, 10, T![|], BinExpr),
        T![&] => (11, 12, T![&], BinExpr),

        T![+] => (15, 16, T![+], BinExpr),
        T![-] => (15, 16, T![-], BinExpr),

        T![*] => (17, 18, T![*], BinExpr),
        T![/] => (17, 18, T![/], BinExpr),
        T![%] => (17, 18, T![%], BinExpr),

        T![=] => (0, 0, T![=], AssignStmt),
        T![+=] => (0, 0, T![+=], AssignStmt),
        T![-=] => (0, 0, T![-=], AssignStmt),
        T![*=] => (0, 0, T![*=], AssignStmt),
        T![/=] => (0, 0, T![/=], AssignStmt),
        T![%=] => (0, 0, T![%=], AssignStmt),

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::tests::*;

    #[test]
    fn literal() {
        check_expr(
            "1",
            expect![[r#"
                Literal
                  INT_NUMBER "1"
            "#]],
        );
    }

    #[test]
    fn name_ref() {
        check_expr(
            "foo",
            expect![[r#"
                NameRef
                  IDENT "foo"
            "#]],
        );
    }

    #[test]
    fn prefix() {
        check_expr(
            "-foo",
            expect![[r#"
                PrefixExpr
                  MINUS "-"
                  NameRef
                    IDENT "foo"
            "#]],
        );
    }

    #[test]
    fn binary() {
        check_expr(
            "1 + 2",
            expect![[r#"
BinExpr
  Literal
    INT_NUMBER "1"
  WHITESPACE " "
  PLUS "+"
  WHITESPACE " "
  Literal
    INT_NUMBER "2"
"#]],
        );
    }

    #[test]
    fn precedence() {
        check_expr(
            "1 + 2 * 3",
            expect![[r#"
                BinExpr
                  Literal
                    INT_NUMBER "1"
                  WHITESPACE " "
                  PLUS "+"
                  WHITESPACE " "
                  BinExpr
                    Literal
                      INT_NUMBER "2"
                    WHITESPACE " "
                    STAR "*"
                    WHITESPACE " "
                    Literal
                      INT_NUMBER "3"
        "#]],
        );
    }

    #[test]
    fn paren() {
        check_expr(
            "(1 + 2) * 3",
            expect![[r#"
BinExpr
  ParenExpr
    L_PAREN "("
    BinExpr
      Literal
        INT_NUMBER "1"
      WHITESPACE " "
      PLUS "+"
      WHITESPACE " "
      Literal
        INT_NUMBER "2"
    R_PAREN ")"
  WHITESPACE " "
  STAR "*"
  WHITESPACE " "
  Literal
    INT_NUMBER "3"
"#]],
        );
    }

    #[test]
    fn field_expr() {
        check_expr(
            "foo.bar",
            expect![[r#"
FieldExpr
  NameRef
    IDENT "foo"
  DOT "."
  NameRef
    IDENT "bar"
"#]],
        );
    }

    #[test]
    fn chained_field_expr() {
        check_expr(
            "foo.bar.baz",
            expect![[r#"
FieldExpr
  FieldExpr
    NameRef
      IDENT "foo"
    DOT "."
    NameRef
      IDENT "bar"
  DOT "."
  NameRef
    IDENT "baz"
"#]],
        );
    }

    #[test]
    fn call_expr() {
        check_expr(
            "foo(1, 2)",
            expect![[r#"
CallExpr
  NameRef
    IDENT "foo"
  ArgList
    L_PAREN "("
    Literal
      INT_NUMBER "1"
    COMMA ","
    WHITESPACE " "
    Literal
      INT_NUMBER "2"
    R_PAREN ")"
"#]],
        );
    }

    #[test]
    fn chained_call_expr() {
        check_expr(
            "foo()(1)",
            expect![[r#"
CallExpr
  CallExpr
    NameRef
      IDENT "foo"
    ArgList
      L_PAREN "("
      R_PAREN ")"
  ArgList
    L_PAREN "("
    Literal
      INT_NUMBER "1"
    R_PAREN ")"
"#]],
        );
    }

    #[test]
    fn index_expr() {
        check_expr(
            "arr[0]",
            expect![[r#"
IndexExpr
  NameRef
    IDENT "arr"
  L_BRACK "["
  Literal
    INT_NUMBER "0"
  R_BRACK "]"
"#]],
        );
    }

    #[test]
    fn chained_index_expr() {
        check_expr(
            "arr[1][0]",
            expect![[r#"
IndexExpr
  IndexExpr
    NameRef
      IDENT "arr"
    L_BRACK "["
    Literal
      INT_NUMBER "1"
    R_BRACK "]"
  L_BRACK "["
  Literal
    INT_NUMBER "0"
  R_BRACK "]"
"#]],
        );
    }

    #[test]
    fn cast_expr() {
        check_expr(
            "foo As Int",
            expect![[r#"
CastExpr
  NameRef
    IDENT "foo"
  WHITESPACE " "
  As_KW "As"
  WHITESPACE " "
  Type
    BaseType
      PrimitiveType
        Int_KW "Int"
"#]],
        );
    }

    #[test]
    fn chained_postfix_expr() {
        check_expr(
            "foo()[1].bar As String",
            expect![[r#"
CastExpr
  FieldExpr
    IndexExpr
      CallExpr
        NameRef
          IDENT "foo"
        ArgList
          L_PAREN "("
          R_PAREN ")"
      L_BRACK "["
      Literal
        INT_NUMBER "1"
      R_BRACK "]"
    DOT "."
    NameRef
      IDENT "bar"
  WHITESPACE " "
  As_KW "As"
  WHITESPACE " "
  Type
    BaseType
      PrimitiveType
        String_KW "String"
"#]],
        );
    }

    #[test]
    fn logical_precedence() {
        check_expr(
            "a || b && c",
            expect![[r#"
BinExpr
  NameRef
    IDENT "a"
  WHITESPACE " "
  PIPE2 "||"
  WHITESPACE " "
  BinExpr
    NameRef
      IDENT "b"
    WHITESPACE " "
    AMP2 "&&"
    WHITESPACE " "
    NameRef
      IDENT "c"
"#]],
        );
    }

    #[test]
    fn comparison_precedence() {
        check_expr(
            "a + b < c * d",
            expect![[r#"
BinExpr
  BinExpr
    NameRef
      IDENT "a"
    WHITESPACE " "
    PLUS "+"
    WHITESPACE " "
    NameRef
      IDENT "b"
  WHITESPACE " "
  LT "<"
  WHITESPACE " "
  BinExpr
    NameRef
      IDENT "c"
    WHITESPACE " "
    STAR "*"
    WHITESPACE " "
    NameRef
      IDENT "d"
"#]],
        );
    }

    #[test]
    fn recover_missing_rhs() {
        check_expr_errors(
            "foo +",
            expect![[r#"
BinExpr
  NameRef
    IDENT "foo"
  WHITESPACE " "
  PLUS "+"
  ERROR
error 5: expected expression
"#]],
        );
    }

    #[test]
    fn recover_missing_index() {
        check_expr_errors(
            "arr[",
            expect![[r#"
IndexExpr
  NameRef
    IDENT "arr"
  L_BRACK "["
  ERROR
error 4: expected expression
error 4: expected R_BRACK
"#]],
        );
    }

    #[test]
    fn recover_missing_field_name() {
        check_expr_errors(
            "foo.",
            expect![[r#"
FieldExpr
  NameRef
    IDENT "foo"
  DOT "."
  ERROR
error 4: expected identifier, `self` or `parent`
"#]],
        );
    }

    #[test]
    fn recover_missing_call_arg() {
        check_expr_errors(
            "foo(,)",
            expect![[r#"
CallExpr
  NameRef
    IDENT "foo"
  ArgList
    L_PAREN "("
    ERROR
      COMMA ","
    R_PAREN ")"
error 4: expected call arguments
"#]],
        );
    }
}
