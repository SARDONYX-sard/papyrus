//! This module contains free-standing functions for creating AST fragments
//! out of smaller pieces, mirroring the approach used by rust-analyzer's
//! `syntax::ast::make` module.
//!
//! Note that every function here is intended to be a "dumb" constructor: it
//! just assembles a finished node from its immediate children by round-
//! tripping through the parser. If you need something smarter, it probably
//! doesn't belong in this module.
//!
//! # A note on incomplete accessors
//!
//! A handful of nodes in `ast::generated::nodes` currently expose no field
//! accessors at all (`AssignStmt`, `BinExpr`, `PrefixExpr`, `Literal`), and a
//! few others are missing accessors you might expect (`IfStmt` has no
//! `then_branch`/`else_branch`, `ElseIfBranch`/`ElseBranch` expose no body).
//! The constructors below still build syntactically correct nodes for these
//! cases by composing raw text, but callers that need to *read back* the
//! sub-parts of the returned node will have to walk `.syntax()` manually
//! until the generator grows the missing accessors.
#![allow(unused)]

// mod quote;

use either::Either;
use itertools::Itertools;
use papyrus_parser::T;
use rowan::NodeOrToken;
use stdx::{format_to, format_to_acc, never};

use crate::{
    AstNode, SourceFile, SyntaxKind, SyntaxToken,
    ast::{
        self,
        Param,
        // make::quote::quote
    },
};

// ============================================================================
// Names
// ============================================================================

pub fn name(text: &str) -> ast::Name {
    ast_from_text(&format!("ScriptName {text}"))
}

pub fn name_ref(text: &str) -> ast::NameRef {
    expr_from_text(text)
}

// ============================================================================
// Types
// ============================================================================

fn ty_from_text(text: &str) -> ast::Type {
    ast_from_text(&format!("ScriptName f\n{text} Property f\n"))
}

/// Builds a `Type` from raw text, e.g. `ty("Actor")` or `ty("Int")`.
pub fn ty(text: &str) -> ast::Type {
    ty_from_text(text)
}

pub fn ty_int() -> ast::Type {
    ty_from_text("Int")
}

pub fn ty_float() -> ast::Type {
    ty_from_text("Float")
}

pub fn ty_bool() -> ast::Type {
    ty_from_text("Bool")
}

pub fn ty_string() -> ast::Type {
    ty_from_text("String")
}

pub fn ty_custom(name: ast::NameRef) -> ast::Type {
    ty_from_text(&name.to_string())
}

/// Appends an array suffix to an existing type, e.g. `ty_array(ty_int())` -> `Int[]`.
pub fn ty_array(elem: ast::Type) -> ast::Type {
    ty_from_text(&format!("{elem}[]"))
}

pub fn array_suffix() -> ast::ArraySuffix {
    ast_from_text("Int[] Function f()\nEndFunction")
}

pub fn primitive_type_int() -> ast::PrimitiveType {
    ast_from_text("Int Function f()\nEndFunction")
}

pub fn primitive_type_float() -> ast::PrimitiveType {
    ast_from_text("Float Function f()\nEndFunction")
}

pub fn primitive_type_bool() -> ast::PrimitiveType {
    ast_from_text("Bool Function f()\nEndFunction")
}

pub fn primitive_type_string() -> ast::PrimitiveType {
    ast_from_text("String Function f()\nEndFunction")
}

pub fn custom_type(name: ast::NameRef) -> ast::CustomType {
    ast_from_text(&format!("{name} Function f()\nEndFunction"))
}

pub fn return_type(ty: ast::Type) -> ast::ReturnType {
    ast_from_text(&format!("{ty} Function f()\nEndFunction"))
}

// ============================================================================
// Params
// ============================================================================

pub fn param(ty: ast::Type, name: ast::Name, default: Option<ast::Expr>) -> ast::Param {
    let text = match default {
        Some(expr) => format!("{ty} {name} = {expr}"),
        None => format!("{ty} {name}"),
    };
    ast_from_text(&format!("Function f({text})\nEndFunction"))
}

pub fn param_list(params: impl IntoIterator<Item = ast::Param>) -> ast::ParamList {
    let params = params.into_iter().join(", ");
    ast_from_text(&format!("Function f({params})\nEndFunction"))
}

pub fn initializer(expr: ast::Expr) -> ast::Initializer {
    ast_from_text(&format!("Function f()\nInt x = {expr}\nEndFunction"))
}

pub fn arg_list(args: impl IntoIterator<Item = ast::Expr>) -> ast::ArgList {
    let args = args.into_iter().join(", ");
    ast_from_text(&format!("Function f()\nf({args})\nEndFunction"))
}

// ============================================================================
// Expressions
//
// `ast::Expr` has its own fast-path parser (`ast::Expr::parse`), reached via
// the private `expr_from_text` helper below, so every node that is a variant
// of `Expr` can be built without wrapping it in a full `Function`/`Script`.
// ============================================================================

pub fn expr_name_ref(text: &str) -> ast::Expr {
    expr_from_text(text)
}

pub fn bin_expr(lhs: ast::Expr, op: &str, rhs: ast::Expr) -> ast::BinExpr {
    expr_from_text(&format!("{lhs} {op} {rhs}"))
}

pub fn prefix_expr(op: &str, expr: ast::Expr) -> ast::PrefixExpr {
    expr_from_text(&format!("{op}{expr}"))
}

pub fn call_expr(callee: ast::Expr, args: ast::ArgList) -> ast::CallExpr {
    expr_from_text(&format!("{callee}{args}"))
}

pub fn method_call_expr(
    receiver: ast::Expr,
    method: ast::NameRef,
    args: ast::ArgList,
) -> ast::MethodCallExpr {
    expr_from_text(&format!("{receiver}.{method}{args}"))
}

pub fn index_expr(base: ast::Expr, index: ast::Expr) -> ast::IndexExpr {
    expr_from_text(&format!("{base}[{index}]"))
}

pub fn field_expr(base: ast::Expr, field: ast::NameRef) -> ast::FieldExpr {
    expr_from_text(&format!("{base}.{field}"))
}

pub fn cast_expr(expr: ast::Expr, ty: ast::Type) -> ast::CastExpr {
    expr_from_text(&format!("{expr} As {ty}"))
}

pub fn new_expr(ty: ast::Type) -> ast::NewExpr {
    expr_from_text(&format!("New {ty}"))
}

pub fn paren_expr(expr: ast::Expr) -> ast::ParenExpr {
    expr_from_text(&format!("({expr})"))
}

pub fn literal(text: &str) -> ast::Literal {
    expr_from_text(text)
}

pub fn literal_int(value: i64) -> ast::Literal {
    literal(&value.to_string())
}

pub fn literal_float(value: f64) -> ast::Literal {
    literal(&value.to_string())
}

pub fn literal_bool(value: bool) -> ast::Literal {
    literal(if value { "true" } else { "false" })
}

pub fn literal_string(value: &str) -> ast::Literal {
    literal(&format!("\"{value}\""))
}

pub fn literal_none() -> ast::Literal {
    literal("None")
}

// ============================================================================
// Statements
// ============================================================================

pub fn var_decl_stmt(
    ty: ast::Type,
    name: ast::Name,
    initializer: Option<ast::Expr>,
) -> ast::VarDeclStmt {
    let text = match initializer {
        Some(expr) => format!("{ty} {name} = {expr}"),
        None => format!("{ty} {name}"),
    };
    ast_from_text(&format!("Function f()\n{text}\nEndFunction"))
}

/// Builds a plain `lhs = rhs` assignment.
///
/// `AssignStmt` currently has no generated field accessors, so callers that
/// need to inspect `lhs`/`rhs` back out of the returned node must walk
/// `.syntax()` themselves for the time being.
pub fn assign_stmt(lhs: ast::Expr, rhs: ast::Expr) -> ast::AssignStmt {
    ast_from_text(&format!("Function f()\n{lhs} = {rhs}\nEndFunction"))
}

pub fn return_stmt(expr: Option<ast::Expr>) -> ast::ReturnStmt {
    let text = match expr {
        Some(expr) => format!("Return {expr}"),
        None => "Return".to_owned(),
    };
    ast_from_text(&format!("Function f()\n{text}\nEndFunction"))
}

pub fn expr_stmt(expr: ast::Expr) -> ast::ExprStmt {
    ast_from_text(&format!("Function f()\n{expr}\nEndFunction"))
}

pub fn while_stmt(condition: ast::Expr, body: ast::Block) -> ast::WhileStmt {
    ast_from_text(&format!("Function f()\nWhile {condition}\n{body}\nEndWhile\nEndFunction"))
}

/// Builds an `If` statement with only a `then` branch. Composing `ElseIf`/
/// `Else` branches on top of it is left to the caller (see `else_if_branch`
/// and `else_branch` below), matching the granularity `IfStmt` currently
/// exposes.
pub fn if_stmt(
    condition: ast::Expr,
    then_branch: impl IntoIterator<Item = ast::Stmt>,
) -> ast::IfStmt {
    let mut buf = format!("If {condition}\n");
    for stmt in then_branch {
        format_to!(buf, "{stmt}\n");
    }
    buf += "EndIf";
    ast_from_text(&format!("Function f()\n{buf}\nEndFunction"))
}

pub fn else_if_branch(condition: ast::Expr) -> ast::ElseIfBranch {
    ast_from_text(&format!("Function f()\nIf true\nElseIf {condition}\nEndIf\nEndFunction"))
}

pub fn else_branch() -> ast::ElseBranch {
    ast_from_text("Function f()\nIf true\nElse\nEndIf\nEndFunction")
}

pub fn block(stmts: impl IntoIterator<Item = ast::Stmt>) -> ast::Block {
    let mut buf = String::new();
    for stmt in stmts {
        format_to!(buf, "{stmt}\n");
    }
    ast_from_text(&format!("Function f()\n{buf}EndFunction"))
}

// ============================================================================
// Flags
// ============================================================================

fn flag_modifier_from_text(text: &str) -> ast::FlagModifier {
    ast_from_text(&format!("ScriptName Foo {text}"))
}

pub fn flag_native() -> ast::FlagModifier {
    flag_modifier_from_text("Native")
}

pub fn flag_global() -> ast::FlagModifier {
    flag_modifier_from_text("Global")
}

pub fn flag_auto() -> ast::FlagModifier {
    flag_modifier_from_text("Auto")
}

pub fn flag_auto_read_only() -> ast::FlagModifier {
    flag_modifier_from_text("AutoReadOnly")
}

/// Builds a bare-identifier flag modifier, e.g. a custom/unknown flag.
pub fn flag_custom(ident: &str) -> ast::FlagModifier {
    flag_modifier_from_text(ident)
}

// ============================================================================
// Items
// ============================================================================

pub fn import(name: ast::NameRef) -> ast::Import {
    ast_from_text(&format!("Import {name}"))
}

pub fn inline_property(
    ty: ast::Type,
    name: ast::Name,
    initializer: Option<ast::Expr>,
    flags: impl IntoIterator<Item = ast::FlagModifier>,
) -> ast::InlineProperty {
    let init = match initializer {
        Some(expr) => format!(" = {expr}"),
        None => String::new(),
    };
    let flags: String = flags.into_iter().map(|flag| format!(" {flag}")).collect();
    ast_from_text(&format!("{ty} Property {name}{init}{flags}"))
}

pub fn property_member(function: ast::Function) -> ast::PropertyMember {
    ast_from_text(&format!("Int Property P\n{function}\nEndProperty"))
}

pub fn full_property(
    ty: ast::Type,
    name: ast::Name,
    flags: impl IntoIterator<Item = ast::FlagModifier>,
    members: impl IntoIterator<Item = ast::PropertyMember>,
) -> ast::FullProperty {
    let flags: String = flags.into_iter().map(|flag| format!(" {flag}")).collect();
    let mut buf = format!("{ty} Property {name}{flags}\n");
    for member in members {
        format_to!(buf, "{member}\n");
    }
    buf += "EndProperty";
    ast_from_text(&buf)
}

pub fn state(
    is_auto: bool,
    name: ast::Name,
    members: impl IntoIterator<Item = ast::StateMember>,
) -> ast::State {
    let auto = if is_auto { "Auto " } else { "" };
    let mut buf = format!("{auto}State {name}\n");
    for member in members {
        format_to!(buf, "{member}\n");
    }
    buf += "EndState";
    ast_from_text(&buf)
}

pub fn fn_(
    fn_name: ast::Name,
    ret_type: Option<ast::ReturnType>,
    params: ast::ParamList,
    body: ast::Block,
) -> ast::Function {
    let ret_type = match ret_type {
        Some(ret_type) => format!("{ret_type} "),
        None => "".into(),
    };

    ast_from_text(&format!(
        "{ret_type} function {fn_name}{params}
{body}
endFunction",
    ))
}

pub fn event_(name: ast::Name, params: ast::ParamList, body: ast::Block) -> ast::Event {
    ast_from_text(&format!(
        "Event {name}{params}
{body}
EndEvent",
    ))
}

// ============================================================================
// Script structure
// ============================================================================

pub fn script_name_decl(name: ast::Name) -> ast::ScriptNameDecl {
    ast_from_text(&format!("ScriptName {name}"))
}

pub fn extends_clause(parent: ast::NameRef) -> ast::ExtendsClause {
    ast_from_text(&format!("ScriptName Foo Extends {parent}"))
}

pub fn header(
    name: ast::Name,
    extends: Option<ast::NameRef>,
    flags: impl IntoIterator<Item = ast::FlagModifier>,
) -> ast::Header {
    let extends = match extends {
        Some(parent) => format!(" Extends {parent}"),
        None => String::new(),
    };
    let flags: String = flags.into_iter().map(|flag| format!(" {flag}")).collect();
    ast_from_text(&format!("ScriptName {name}{extends}{flags}"))
}

pub fn script(header: ast::Header, items: impl IntoIterator<Item = ast::Item>) -> ast::Script {
    let mut buf = format!("{header}\n");
    for item in items {
        format_to!(buf, "{item}\n");
    }
    ast_from_text(&buf)
}

pub fn source_file(script: ast::Script) -> ast::SourceFile {
    ast_from_text(&format!("{script}"))
}

// ============================================================================
// Internal helpers
// ============================================================================

#[track_caller]
fn expr_from_text<E: Into<ast::Expr> + AstNode>(text: &str) -> E {
    expr_from_text_with_flags(text, &[])
}

#[track_caller]
fn expr_from_text_with_flags<E: Into<ast::Expr> + AstNode>(text: &str, flags: &[String]) -> E {
    let parse = ast::Expr::parse(text, flags);
    let node = match parse.tree().syntax().descendants().find_map(E::cast) {
        Some(it) => it,
        None => {
            let node = std::any::type_name::<E>();
            panic!("Failed to make ast node `{node}` from text {text}")
        }
    };
    let node = node.clone_subtree();
    assert_eq!(node.syntax().text_range().start(), 0.into());
    node
}

#[track_caller]
fn ast_from_text<N: AstNode>(text: &str) -> N {
    ast_from_text_with_flags(text, &[])
}

#[track_caller]
fn ast_from_text_with_flags<N: AstNode>(text: &str, flags: &[String]) -> N {
    let parse = SourceFile::parse(text, flags);
    let node = match parse.tree().syntax().descendants().find_map(N::cast) {
        Some(it) => it,
        None => {
            std::fs::write(
                "../nodes.log",
                format!(
                    "{:#?}, errors:\n{:#?}",
                    parse.tree().syntax().descendants().collect::<Vec<_>>(),
                    parse.errors()
                ),
            )
            .unwrap();
            let node = std::any::type_name::<N>();
            panic!("Failed to make ast node `{node}` from text {text}")
        }
    };
    let node = node.clone_subtree();
    assert_eq!(node.syntax().text_range().start(), 0.into());
    node
}

pub fn token(kind: SyntaxKind) -> SyntaxToken {
    tokens::SOURCE_FILE
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|it| it.into_token())
        .find(|it| it.kind() == kind)
        .unwrap_or_else(|| panic!("unhandled token: {kind:?}"))
}

pub mod tokens {
    use std::sync::LazyLock;

    use crate::{AstNode, Parse, SourceFile, SyntaxKind::*, SyntaxToken, ast};

    /// A small, syntactically valid Papyrus script used purely as a source of
    /// punctuation/operator tokens (`&&`, `||`, `==`, `!=`, `<`, `<=`, `>`,
    /// `>=`, `!`, arithmetic operators, brackets, etc). Unlike a straight
    /// port of rust-analyzer's seed text, this must actually be Papyrus:
    /// Papyrus has no pointers, `mut`, `async`, `unsafe`, `impl`, or `where`,
    /// so none of that Rust-flavored text would lex the way `token()` needs.
    pub(super) static SOURCE_FILE: LazyLock<Parse<SourceFile>> = LazyLock::new(|| {
        SourceFile::parse(
            "ScriptName Foo Extends Bar Hidden\n\n\
             Function F(Int a, Float b = 1.0) Global Native\n\
             \tInt[] arr = New Int[3]\n\
             \tBool cond = (a == 1) && (a != 2) || (a < 3) || (a <= 4) || (a > 5) || (a >= 6)\n\
             \tBool neg = !cond\n\
             \tarr[0] = a + 1 - 2 * 3 / 4 % 5\n\
             \tSelf.DoSomething(a, b)\n\
             \tReturn a\n\
             EndFunction\n",
            &[],
        )
    });

    pub fn whitespace(text: &str) -> SyntaxToken {
        assert!(text.trim().is_empty());
        let sf = SourceFile::parse(text, &[]).ok().unwrap();
        sf.syntax().first_child_or_token().unwrap().into_token().unwrap()
    }

    pub fn doc_comment(text: &str) -> SyntaxToken {
        assert!(!text.trim().is_empty());
        let sf = SourceFile::parse(text, &[]).ok().unwrap();
        sf.syntax().first_child_or_token().unwrap().into_token().unwrap()
    }

    /// Extracts the literal token that results from parsing `text` as a
    /// Papyrus expression (e.g. `"1"`, `"1.0"`, `"true"`, `"\"hi\""`, `"None"`).
    pub fn literal(text: &str) -> SyntaxToken {
        assert_eq!(text.trim(), text);
        let lit: ast::Literal = super::expr_from_text(text);
        lit.syntax().first_child_or_token().unwrap().into_token().unwrap()
    }

    /// Extracts a bare `IDENT` token by parsing `text` as a Papyrus
    /// `NameRef` expression (e.g. a variable or type name reference).
    pub fn ident(text: &str) -> SyntaxToken {
        assert_eq!(text.trim(), text);

        let name_ref: ast::NameRef = super::expr_from_text(text);
        name_ref
            .syntax()
            .descendants_with_tokens()
            .filter_map(|it| it.into_token())
            .find(|it| it.kind() == IDENT)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    #[track_caller]
    fn check(node: impl AstNode, expect: expect_test::Expect) {
        let node_debug = format!("{:#?}", node.syntax());
        expect.assert_eq(&node_debug);
    }

    #[test]
    fn test_name_and_name_ref() {
        check(
            name("Foo"),
            expect![[r#"
                NAME@0..3
                  IDENT@0..3 "Foo"
            "#]],
        );
        check(
            name_ref("Foo"),
            expect![[r#"
                NAME_REF@0..3
                  IDENT@0..3 "Foo"
            "#]],
        );
    }

    #[test]
    fn test_ty() {
        check(
            ty_int(),
            expect![[r#"
            TYPE@0..3
              BASE_TYPE@0..3
                PRIMITIVE_TYPE@0..3
                  Int_KW@0..3 "Int"
        "#]],
        );
        check(ty_array(ty_int()), expect![[r#""#]]);
        // check(ty_custom(name_ref("Actor")), expect![[r#""#]]);
    }

    #[test]
    fn test_literal() {
        check(
            literal_int(42),
            expect![[r#"
LITERAL@0..2
  INT_NUMBER@0..2 "42"
"#]],
        );
        check(
            literal_bool(true),
            expect![[r#"
LITERAL@0..4
  True_KW@0..4 "true"
"#]],
        );
        check(
            literal_string("hi"),
            expect![[r#"
LITERAL@0..4
  STRING@0..4 "\"hi\""
"#]],
        );
        check(literal_none(), expect![[r#""#]]);
    }

    #[test]
    fn test_param_list() {
        let params = param_list(vec![
            param(ty_int(), name("a"), None),
            param(ty_float(), name("b"), Some(literal_float(1.0).into())),
        ]);
        check(params, expect![[r#""#]]);
    }

    #[test]
    fn test_var_decl_stmt() {
        let stmt = var_decl_stmt(ty_int(), name("x"), Some(literal_int(1).into()));
        check(stmt, expect![[r#""#]]);
    }

    #[test]
    fn test_if_stmt() {
        let cond = bin_expr(name_ref("a").into(), "==", literal_int(1).into());
        let stmt = if_stmt(cond.into(), vec![return_stmt(Some(name_ref("a").into())).into()]);
        check(stmt, expect![[r#""#]]);
    }

    #[test]
    fn test_while_stmt() {
        let cond = literal_bool(true);
        let body = block(vec![expr_stmt(name_ref("a").into()).into()]);
        check(while_stmt(cond.into(), body), expect![[r#""#]]);
    }

    #[test]
    fn test_fn_() {
        let params = param_list(vec![param(ty_int(), name("a"), None)]);
        let body = block(vec![return_stmt(Some(name_ref("a").into())).into()]);
        let f = fn_(name("DoIt"), Some(return_type(ty_int())), params, body);
        check(f, expect![[r#""#]]);
    }

    #[test]
    fn test_event_() {
        let params = param_list(vec![param(ty_int(), name("akButton"), None)]);
        let body = block(vec![]);
        let ev = event_(name("OnKeyDown"), params, body);
        check(ev, expect![[r#""#]]);
    }

    #[test]
    fn test_script() {
        let header = header(name("MyScript"), Some(name_ref("Actor")), vec![flag_auto()]);
        let f = fn_(name("DoIt"), None, param_list(vec![]), block(vec![]));
        let script = script(header, vec![f.into()]);
        check(source_file(script), expect![[r#""#]]);
    }
}
