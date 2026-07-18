use std::fmt::Write as _;

use crate::{LexedStr, TopEntryPoint};
use expect_test::Expect;
pub(crate) use expect_test::expect;

pub(crate) fn parse(entry: TopEntryPoint, text: &str, custom_flags: &[String]) -> (String, bool) {
    let lexed = LexedStr::new(text);
    let input = lexed.to_input(custom_flags);
    let output = entry.parse(&input);

    if cfg!(test) {
        let steps: Vec<_> = output.iter().collect();
        std::fs::write("../../target/dump.log", format!("{steps:#?}")).unwrap();
    }

    let mut buf = String::new();
    let mut errors = Vec::new();
    let mut indent = String::new();
    let mut depth = 0;
    let mut len = 0;
    lexed.intersperse_trivia(&output, &mut |step| match step {
        crate::StrStep::Token { kind, text } => {
            assert!(depth > 0);
            len += text.len();
            writeln!(buf, "{indent}{kind:?} {text:?}").unwrap();
        }
        crate::StrStep::Enter { kind } => {
            assert!(depth > 0 || len == 0);
            depth += 1;
            writeln!(buf, "{indent}{kind:?}").unwrap();
            indent.push_str("  ");
        }
        crate::StrStep::Exit => {
            assert!(depth > 0);
            depth -= 1;
            indent.pop();
            indent.pop();
        }
        crate::StrStep::Error { msg, pos } => {
            assert!(depth > 0);
            errors.push(format!("error {pos}: {msg}\n"))
        }
    });
    assert_eq!(
        len,
        text.len(),
        "didn't parse all text.\nParsed:\n{}\n\nAll:\n{}\n",
        &text[..len],
        text
    );

    for (token, msg) in lexed.errors() {
        let pos = lexed.text_start(token);
        errors.push(format!("error {pos}: {msg}\n"));
    }

    let has_errors = !errors.is_empty();
    for e in errors {
        buf.push_str(&e);
    }
    (buf, has_errors)
}

pub(crate) fn check(src: &str, expect: Expect) {
    let (tree, errors) = parse(TopEntryPoint::SourceFile, src, &[]);

    assert!(!errors);

    expect.assert_eq(&tree);
}

pub(crate) fn check_with_flags(src: &str, expect: Expect, custom_flags: &[String]) {
    let (tree, errors) = parse(TopEntryPoint::SourceFile, src, custom_flags);

    assert!(!errors);

    expect.assert_eq(&tree);
}

pub(crate) fn check_errors(src: &str, expect: Expect) {
    let (tree, _) = parse(TopEntryPoint::SourceFile, src, &[]);

    expect.assert_eq(&tree);
}

pub(crate) fn check_expr(src: &str, expect: Expect) {
    let (tree, errors) = parse(TopEntryPoint::Expr, src, &[]);

    assert!(!errors);

    expect.assert_eq(&tree);
}

pub(crate) fn check_expr_errors(src: &str, expect: Expect) {
    let (tree, _) = parse(TopEntryPoint::Expr, src, &[]);

    expect.assert_eq(&tree);
}
