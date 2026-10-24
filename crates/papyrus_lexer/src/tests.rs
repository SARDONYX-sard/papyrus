use super::*;
use expect_test::{Expect, expect};

fn check(src: &str, expect: Expect) {
    let actual: String = Lexer::new(src)
        .map(|t| match t.error {
            Some(e) => format!("{:?} {:?} error: {e}\n", t.kind, t.text),
            None => format!("{:?} {:?}\n", t.kind, t.text),
        })
        .collect();
    expect.assert_eq(&actual);
}

fn kinds(src: &str) -> Vec<TokenKind> {
    Lexer::new(src).map(|t| t.kind).collect()
}

#[test]
fn lossless() {
    let src = "ScriptName Foo extends Bar ; c\r\n;/ a\n b /;{doc}\n  x = \"s\\\"\" \\\n + 1.5\n#";
    let joined: String = Lexer::new(src).map(|t| t.text).collect();
    assert_eq!(joined, src);
}

#[test]
fn keywords_are_case_insensitive() {
    assert_eq!(
        kinds("scriptname SCRIPTNAME ScriptName endFunction autoreadonly"),
        [
            TokenKind::ScriptNameKw,
            TokenKind::Whitespace,
            TokenKind::ScriptNameKw,
            TokenKind::Whitespace,
            TokenKind::ScriptNameKw,
            TokenKind::Whitespace,
            TokenKind::EndFunctionKw,
            TokenKind::Whitespace,
            TokenKind::AutoReadOnlyKw,
        ]
    );
    assert_eq!(kinds("Hidden Length Conditional"), {
        use TokenKind::*;
        [Ident, Whitespace, Ident, Whitespace, Ident]
    });
}

#[test]
fn newlines_are_tokens() {
    check(
        "a\nb\r\n\r\nc",
        expect![[r#"
            Ident "a"
            Newline "\n"
            Ident "b"
            Newline "\r\n"
            Newline "\r\n"
            Ident "c"
        "#]],
    );
}

#[test]
fn comments() {
    check(
        "; line\n;/ block\nstill block /;{ doc\n comment }",
        expect![[r#"
            LineComment "; line"
            Newline "\n"
            BlockComment ";/ block\nstill block /;"
            DocComment "{ doc\n comment }"
        "#]],
    );
}

#[test]
fn block_comment_is_not_nested() {
    check(
        ";/ a ;/ b /; c",
        expect![[r#"
            BlockComment ";/ a ;/ b /;"
            Whitespace " "
            Ident "c"
        "#]],
    );
}

#[test]
fn line_comment_excludes_crlf() {
    check(
        "; c\r\n",
        expect![[r#"
            LineComment "; c"
            Newline "\r\n"
        "#]],
    );
}

#[test]
fn unterminated_comments() {
    check(
        ";/ open",
        expect![[r#"
            BlockComment ";/ open" error: unterminated block comment, expected `/;`
        "#]],
    );
    check(
        "{ open",
        expect![[r#"
            DocComment "{ open" error: unterminated documentation comment, expected `}`
        "#]],
    );
}

#[test]
fn line_continuation() {
    check(
        "Foo(a, \\  \r\n  b)",
        expect![[r#"
            Ident "Foo"
            LParen "("
            Ident "a"
            Comma ","
            Whitespace " "
            LineContinuation "\\  \r\n"
            Whitespace "  "
            Ident "b"
            RParen ")"
        "#]],
    );
    check(
        "a \\ b",
        expect![[r#"
            Ident "a"
            Whitespace " "
            Unknown "\\" error: a line continuation `\` must be the last character on the line
            Whitespace " "
            Ident "b"
        "#]],
    );
}

#[test]
fn numbers() {
    check(
        "0 42 0x1F 0xff 1.5 2.0e3 3.25E-2 1. 0x",
        expect![[r#"
            Int "0"
            Whitespace " "
            Int "42"
            Whitespace " "
            Int "0x1F"
            Whitespace " "
            Int "0xff"
            Whitespace " "
            Float "1.5"
            Whitespace " "
            Float "2.0e3"
            Whitespace " "
            Float "3.25E-2"
            Whitespace " "
            Int "1"
            Dot "."
            Whitespace " "
            Int "0x" error: expected hexadecimal digits after `0x`
        "#]],
    );
}

#[test]
fn strings() {
    check(
        r#""plain" "esc \" \\ \n" "open"#,
        expect![[r#"
            String "\"plain\""
            Whitespace " "
            String "\"esc \\\" \\\\ \\n\""
            Whitespace " "
            String "\"open" error: unterminated string literal
        "#]],
    );
    check(
        "\"open\nx",
        expect![[r#"
            String "\"open" error: unterminated string literal
            Newline "\n"
            Ident "x"
        "#]],
    );
}

#[test]
fn unescape() {
    assert_eq!(unescape_string(r#""abc""#), "abc");
    assert_eq!(unescape_string(r#""a\"b\\c\nd\te""#), "a\"b\\c\nd\te");
}

#[test]
fn operators() {
    check(
        "= == != < <= > >= + += - -= * *= / /= % %= ! && || & |",
        expect![[r#"
            Eq "="
            Whitespace " "
            EqEq "=="
            Whitespace " "
            NotEq "!="
            Whitespace " "
            Lt "<"
            Whitespace " "
            LtEq "<="
            Whitespace " "
            Gt ">"
            Whitespace " "
            GtEq ">="
            Whitespace " "
            Plus "+"
            Whitespace " "
            PlusEq "+="
            Whitespace " "
            Minus "-"
            Whitespace " "
            MinusEq "-="
            Whitespace " "
            Star "*"
            Whitespace " "
            StarEq "*="
            Whitespace " "
            Slash "/"
            Whitespace " "
            SlashEq "/="
            Whitespace " "
            Percent "%"
            Whitespace " "
            PercentEq "%="
            Whitespace " "
            Bang "!"
            Whitespace " "
            AmpAmp "&&"
            Whitespace " "
            PipePipe "||"
            Whitespace " "
            Unknown "&" error: unknown character
            Whitespace " "
            Unknown "|" error: unknown character
        "#]],
    );
}

#[test]
fn unicode_and_bom() {
    check(
        "\u{FEFF}a\u{3000}b №",
        expect![[r#"
            Whitespace "\u{feff}"
            Ident "a"
            Whitespace "\u{3000}"
            Ident "b"
            Whitespace " "
            Unknown "№" error: unknown character
        "#]],
    );
}
