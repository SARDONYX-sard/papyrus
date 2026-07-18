use super::*;
use expect_test::{Expect, expect};

fn kinds(src: &str) -> Vec<TokenKind> {
    tokenize(src).map(|t| t.kind).collect()
}

fn assert_tokens(src: &str, expected: &[TokenKind]) {
    assert_eq!(kinds(src), expected);
}

fn check_lexing(src: &str, expect: Expect) {
    let actual: String = tokenize(src).map(|token| format!("{:?}\n", token)).collect();
    expect.assert_eq(&actual)
}

// /////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn identifier() {
    assert_tokens("Foo", &[TokenKind::Ident]);
}

#[test]
fn function_keyword() {
    assert_tokens("Function", &[TokenKind::Ident]);
}

// /////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn line_comment() {
    assert_tokens("; hello\n", &[TokenKind::LineComment, TokenKind::LineFeed]);
}

#[test]
fn nested_block_comment() {
    assert_tokens(";/ a ;/ b /; c /;", &[TokenKind::BlockComment { terminated: true }]);
}

#[test]
fn unterminated_block_comment() {
    assert_tokens(";/ abc", &[TokenKind::BlockComment { terminated: false }]);
}

// /////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn doc_comment() {
    assert_tokens("{ docs }", &[TokenKind::DocComment { terminated: true }]);
}

// /////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn decimal_integer() {
    assert_tokens(
        "123",
        &[TokenKind::Literal {
            kind: LiteralKind::Int { base: Base::Decimal, empty_int: false },
            suffix_start: 3,
        }],
    );
}

#[test]
fn float_literal() {
    assert_tokens(
        "1.5",
        &[TokenKind::Literal {
            kind: LiteralKind::Float { base: Base::Decimal, empty_exponent: false },
            suffix_start: 3,
        }],
    );
}

#[test]
fn string_literal() {
    assert_tokens(
        r#""abc""#,
        &[TokenKind::Literal { kind: LiteralKind::Str { terminated: true }, suffix_start: 5 }],
    );
}

#[test]
fn unterminated_string() {
    assert_tokens(
        "\"abc",
        &[TokenKind::Literal { kind: LiteralKind::Str { terminated: false }, suffix_start: 4 }],
    );
}

#[test]
fn whitespace() {
    assert_tokens(" \t\n", &[TokenKind::Whitespace, TokenKind::LineFeed]);
}

#[test]
fn punctuation() {
    assert_tokens(
        "(),[]",
        &[
            TokenKind::OpenParen,
            TokenKind::CloseParen,
            TokenKind::Comma,
            TokenKind::OpenBracket,
            TokenKind::CloseBracket,
        ],
    );
}

#[test]
fn smoke_test() {
    let src = r#"
ScriptName TestScript Extends Quest Hidden Conditional

Import Utility

Int Property Counter Auto
String Property Name
    Function Get()
        Return ""
    EndFunction
EndProperty

State Waiting
    Event OnBeginState()
        Counter = 0
    EndEvent
EndState

Function Foo(Int x, Float y = 1.5)
    Int i = 0

    While i < 10
        If i == 5
            Debug.Trace("middle")
        ElseIf i == 8
            Return
        Else
            i += 1
        EndIf
    EndWhile
EndFunction

Event OnInit()
    Foo(1, 2.0)
EndEvent
"#;

    check_lexing(
        src,
        expect![[r#"
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 10 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 10 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 7 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 5 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 6 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 11 }
Token { kind: LineFeed, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 6 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 7 }
Token { kind: LineFeed, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 3 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 8 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 7 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 4 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 6 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 8 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 4 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 8 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 3 }
Token { kind: OpenParen, len: 1 }
Token { kind: CloseParen, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 8 }
Token { kind: Ident, len: 6 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Str { terminated: true }, suffix_start: 2 }, len: 2 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 11 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 11 }
Token { kind: LineFeed, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 5 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 7 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 5 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 12 }
Token { kind: OpenParen, len: 1 }
Token { kind: CloseParen, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 8 }
Token { kind: Ident, len: 7 }
Token { kind: Whitespace, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 1 }, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 8 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 8 }
Token { kind: LineFeed, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 8 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 3 }
Token { kind: OpenParen, len: 1 }
Token { kind: Ident, len: 3 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 1 }
Token { kind: Comma, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 5 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Float { base: Decimal, empty_exponent: false }, suffix_start: 3 }, len: 3 }
Token { kind: CloseParen, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 3 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 1 }, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 5 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Lt, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 2 }, len: 2 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 8 }
Token { kind: Ident, len: 2 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 1 }, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 12 }
Token { kind: Ident, len: 5 }
Token { kind: Dot, len: 1 }
Token { kind: Ident, len: 5 }
Token { kind: OpenParen, len: 1 }
Token { kind: Literal { kind: Str { terminated: true }, suffix_start: 8 }, len: 8 }
Token { kind: CloseParen, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 8 }
Token { kind: Ident, len: 6 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 1 }, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 12 }
Token { kind: Ident, len: 6 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 8 }
Token { kind: Ident, len: 4 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 12 }
Token { kind: Ident, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Plus, len: 1 }
Token { kind: Eq, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 1 }, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 8 }
Token { kind: Ident, len: 5 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 8 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 11 }
Token { kind: LineFeed, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 5 }
Token { kind: Whitespace, len: 1 }
Token { kind: Ident, len: 6 }
Token { kind: OpenParen, len: 1 }
Token { kind: CloseParen, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Whitespace, len: 4 }
Token { kind: Ident, len: 3 }
Token { kind: OpenParen, len: 1 }
Token { kind: Literal { kind: Int { base: Decimal, empty_int: false }, suffix_start: 1 }, len: 1 }
Token { kind: Comma, len: 1 }
Token { kind: Whitespace, len: 1 }
Token { kind: Literal { kind: Float { base: Decimal, empty_exponent: false }, suffix_start: 3 }, len: 3 }
Token { kind: CloseParen, len: 1 }
Token { kind: LineFeed, len: 1 }
Token { kind: Ident, len: 8 }
Token { kind: LineFeed, len: 1 }
"#]],
    );
}
