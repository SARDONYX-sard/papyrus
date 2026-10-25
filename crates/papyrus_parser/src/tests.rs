use std::path::{Path, PathBuf};

use expect_test::{Expect, expect};

use crate::{ast::*, parse};

fn check(src: &str, expect: Expect) {
    let parse = parse(src);
    assert_eq!(parse.syntax_node().text(), src, "tree is not lossless");
    expect.assert_eq(&parse.debug_dump());
}

fn errors(src: &str) -> Vec<String> {
    parse(src).errors().iter().map(|e| format!("{:?}: {}", e.range(), e.message())).collect()
}

#[test]
fn header() {
    check(
        "ScriptName Foo extends Bar Hidden Conditional\n",
        expect![[r#"
            SOURCE_FILE@0..46
              SCRIPT_HEADER@0..45
                SCRIPTNAME_KW@0..10 "ScriptName"
                WHITESPACE@10..11 " "
                NAME@11..14
                  IDENT@11..14 "Foo"
                WHITESPACE@14..15 " "
                EXTENDS_CLAUSE@15..26
                  EXTENDS_KW@15..22 "extends"
                  WHITESPACE@22..23 " "
                  NAME_REF@23..26
                    IDENT@23..26 "Bar"
                WHITESPACE@26..27 " "
                FLAG@27..33
                  IDENT@27..33 "Hidden"
                WHITESPACE@33..34 " "
                FLAG@34..45
                  IDENT@34..45 "Conditional"
              NEWLINE@45..46 "\n"
        "#]],
    );
}

#[test]
fn flags_and_body_are_split_by_newline() {
    // `Hidden` on the next line is a statement, not a flag.
    check(
        "ScriptName A\nFunction Foo() Global\n  Hidden()\nEndFunction\n",
        expect![[r#"
            SOURCE_FILE@0..58
              SCRIPT_HEADER@0..12
                SCRIPTNAME_KW@0..10 "ScriptName"
                WHITESPACE@10..11 " "
                NAME@11..12
                  IDENT@11..12 "A"
              NEWLINE@12..13 "\n"
              FUNCTION@13..57
                FUNCTION_KW@13..21 "Function"
                WHITESPACE@21..22 " "
                NAME@22..25
                  IDENT@22..25 "Foo"
                PARAM_LIST@25..27
                  L_PAREN@25..26 "("
                  R_PAREN@26..27 ")"
                WHITESPACE@27..28 " "
                FLAG@28..34
                  GLOBAL_KW@28..34 "Global"
                BLOCK@34..46
                  NEWLINE@34..35 "\n"
                  WHITESPACE@35..37 "  "
                  EXPR_STMT@37..45
                    CALL_EXPR@37..45
                      NAME_REF@37..43
                        IDENT@37..43 "Hidden"
                      ARG_LIST@43..45
                        L_PAREN@43..44 "("
                        R_PAREN@44..45 ")"
                  NEWLINE@45..46 "\n"
                ENDFUNCTION_KW@46..57 "EndFunction"
              NEWLINE@57..58 "\n"
        "#]],
    );
}

#[test]
fn native_function_has_no_body() {
    check(
        "ScriptName A\nInt Function Foo(Actor akActor, Float afValue = 1.0) Native Global\n{ Docs }\nFunction Bar()\nEndFunction",
        expect![[r#"
            SOURCE_FILE@0..115
              SCRIPT_HEADER@0..12
                SCRIPTNAME_KW@0..10 "ScriptName"
                WHITESPACE@10..11 " "
                NAME@11..12
                  IDENT@11..12 "A"
              NEWLINE@12..13 "\n"
              FUNCTION@13..79
                TYPE@13..16
                  INT_KW@13..16 "Int"
                WHITESPACE@16..17 " "
                FUNCTION_KW@17..25 "Function"
                WHITESPACE@25..26 " "
                NAME@26..29
                  IDENT@26..29 "Foo"
                PARAM_LIST@29..65
                  L_PAREN@29..30 "("
                  PARAM@30..43
                    TYPE@30..35
                      IDENT@30..35 "Actor"
                    WHITESPACE@35..36 " "
                    NAME@36..43
                      IDENT@36..43 "akActor"
                  COMMA@43..44 ","
                  WHITESPACE@44..45 " "
                  PARAM@45..64
                    TYPE@45..50
                      FLOAT_KW@45..50 "Float"
                    WHITESPACE@50..51 " "
                    NAME@51..58
                      IDENT@51..58 "afValue"
                    WHITESPACE@58..59 " "
                    INITIALIZER@59..64
                      EQ@59..60 "="
                      WHITESPACE@60..61 " "
                      LITERAL@61..64
                        FLOAT_NUMBER@61..64 "1.0"
                  R_PAREN@64..65 ")"
                WHITESPACE@65..66 " "
                FLAG@66..72
                  NATIVE_KW@66..72 "Native"
                WHITESPACE@72..73 " "
                FLAG@73..79
                  GLOBAL_KW@73..79 "Global"
              NEWLINE@79..80 "\n"
              DOC_COMMENT@80..88 "{ Docs }"
              NEWLINE@88..89 "\n"
              FUNCTION@89..115
                FUNCTION_KW@89..97 "Function"
                WHITESPACE@97..98 " "
                NAME@98..101
                  IDENT@98..101 "Bar"
                PARAM_LIST@101..103
                  L_PAREN@101..102 "("
                  R_PAREN@102..103 ")"
                BLOCK@103..104
                  NEWLINE@103..104 "\n"
                ENDFUNCTION_KW@104..115 "EndFunction"
        "#]],
    );
}

#[test]
fn expressions_precedence() {
    check(
        "ScriptName A\nFunction F()\nx = -a.b[1] as Int + 2 * 3 || !c && Foo(z = 1, \"s\")\nEndFunction\n",
        expect![[r#"
            SOURCE_FILE@0..90
              SCRIPT_HEADER@0..12
                SCRIPTNAME_KW@0..10 "ScriptName"
                WHITESPACE@10..11 " "
                NAME@11..12
                  IDENT@11..12 "A"
              NEWLINE@12..13 "\n"
              FUNCTION@13..89
                FUNCTION_KW@13..21 "Function"
                WHITESPACE@21..22 " "
                NAME@22..23
                  IDENT@22..23 "F"
                PARAM_LIST@23..25
                  L_PAREN@23..24 "("
                  R_PAREN@24..25 ")"
                BLOCK@25..78
                  NEWLINE@25..26 "\n"
                  ASSIGN_STMT@26..77
                    NAME_REF@26..27
                      IDENT@26..27 "x"
                    WHITESPACE@27..28 " "
                    EQ@28..29 "="
                    WHITESPACE@29..30 " "
                    BIN_EXPR@30..77
                      BIN_EXPR@30..52
                        PREFIX_EXPR@30..44
                          MINUS@30..31 "-"
                          CAST_EXPR@31..44
                            INDEX_EXPR@31..37
                              FIELD_EXPR@31..34
                                NAME_REF@31..32
                                  IDENT@31..32 "a"
                                DOT@32..33 "."
                                NAME_REF@33..34
                                  IDENT@33..34 "b"
                              L_BRACK@34..35 "["
                              LITERAL@35..36
                                INT_NUMBER@35..36 "1"
                              R_BRACK@36..37 "]"
                            WHITESPACE@37..38 " "
                            AS_KW@38..40 "as"
                            WHITESPACE@40..41 " "
                            TYPE@41..44
                              INT_KW@41..44 "Int"
                        WHITESPACE@44..45 " "
                        PLUS@45..46 "+"
                        WHITESPACE@46..47 " "
                        BIN_EXPR@47..52
                          LITERAL@47..48
                            INT_NUMBER@47..48 "2"
                          WHITESPACE@48..49 " "
                          STAR@49..50 "*"
                          WHITESPACE@50..51 " "
                          LITERAL@51..52
                            INT_NUMBER@51..52 "3"
                      WHITESPACE@52..53 " "
                      PIPE2@53..55 "||"
                      WHITESPACE@55..56 " "
                      BIN_EXPR@56..77
                        PREFIX_EXPR@56..58
                          BANG@56..57 "!"
                          NAME_REF@57..58
                            IDENT@57..58 "c"
                        WHITESPACE@58..59 " "
                        AMP2@59..61 "&&"
                        WHITESPACE@61..62 " "
                        CALL_EXPR@62..77
                          NAME_REF@62..65
                            IDENT@62..65 "Foo"
                          ARG_LIST@65..77
                            L_PAREN@65..66 "("
                            ARG@66..71
                              NAME_REF@66..67
                                IDENT@66..67 "z"
                              WHITESPACE@67..68 " "
                              EQ@68..69 "="
                              WHITESPACE@69..70 " "
                              LITERAL@70..71
                                INT_NUMBER@70..71 "1"
                            COMMA@71..72 ","
                            WHITESPACE@72..73 " "
                            ARG@73..76
                              LITERAL@73..76
                                STRING@73..76 "\"s\""
                            R_PAREN@76..77 ")"
                  NEWLINE@77..78 "\n"
                ENDFUNCTION_KW@78..89 "EndFunction"
              NEWLINE@89..90 "\n"
        "#]],
    );
}

#[test]
fn line_continuation_joins_lines() {
    let src = "ScriptName A\nFunction F()\n  Foo(1, \\\n    2)\nEndFunction\n";
    assert_eq!(errors(src), Vec::<String>::new());
}

#[test]
fn properties_states_events() {
    let src = r#"Scriptname A extends ObjectReference
Import Utility
Int Property Count = 5 Auto Hidden
Actor Property Target Auto
Float Property Speed
    Float Function Get()
        Return 1.0
    EndFunction
    Function Set(Float value)
    EndFunction
EndProperty
Int[] values
Auto State Waiting
    Event OnActivate(ObjectReference akActionRef)
        GoToState("Done")
    EndEvent
EndState
State Done
EndState
"#;
    assert_eq!(errors(src), Vec::<String>::new());
    let parse = parse(src);
    let file = parse.source_file();
    let kinds: Vec<_> = file.items().map(|i| format!("{:?}", i.syntax().kind())).collect();
    assert_eq!(
        kinds,
        [
            "SCRIPT_HEADER",
            "IMPORT",
            "PROPERTY",
            "PROPERTY",
            "PROPERTY",
            "VAR_DECL",
            "STATE",
            "STATE"
        ]
    );
    let Some(Item::Property(speed)) = file.items().nth(4) else { panic!() };
    assert!(!speed.is_auto());
    assert_eq!(speed.functions().count(), 2);
    let Some(Item::State(waiting)) = file.items().nth(6) else { panic!() };
    assert!(waiting.is_auto());
    assert_eq!(waiting.name().unwrap().text(), "Waiting");
}

#[test]
fn statements() {
    let src = r#"ScriptName A
Function F(Int[] arr)
    Int i = 0
    Actor[] actors = new Actor[10]
    arr[0] = 1
    While i < arr.Length
        If arr[i] == 1
            i += 1
        ElseIf (arr[i] > 2)
            Return
        Else
            Debug.Trace("x" + i)
        EndIf
    EndWhile
    (Self as Actor).Kill()
EndFunction
"#;
    assert_eq!(errors(src), Vec::<String>::new());
}

#[test]
fn recovers_missing_endif() {
    let src =
        "ScriptName A\nFunction F()\n  If x\n    y()\nEndFunction\nFunction G()\nEndFunction\n";
    assert_eq!(errors(src), ["41..52: expected `EndIf`, found `EndFunction`"]);
    let parse = parse(src);
    assert_eq!(parse.source_file().items().count(), 3);
}

#[test]
fn recovers_missing_endfunction() {
    let src = "ScriptName A\nFunction F()\n  y()\nFunction G()\nEndFunction\n";
    assert_eq!(errors(src), ["32..40: expected `EndFunction`, found `Function`"]);
    assert_eq!(parse(src).source_file().items().count(), 3);
}

#[test]
fn recovers_garbage_lines() {
    let src = "ScriptName A\nFunction F()\n  x = = 1\n  ) foo\n  y()\nEndFunction\n";
    assert_eq!(
        errors(src),
        ["31..31: expected an expression, found `=`", "38..39: expected an expression, found `)`"]
    );
    let parse = parse(src);
    let Some(Item::Function(f)) = parse.source_file().items().nth(1) else { panic!() };
    assert_eq!(f.body().unwrap().stmts().count(), 2);
}

#[test]
fn unclosed_paren_stops_at_newline() {
    let src = "ScriptName A\nFunction F()\n  Foo(1, 2\n  Bar()\nEndFunction\n";
    assert_eq!(errors(src), ["36..36: expected `)`"]);
}

#[test]
fn missing_script_name() {
    assert_eq!(errors("Int x\n"), ["0..3: expected `ScriptName` declaration, found `Int`"]);
    assert_eq!(errors(""), ["0..0: expected `ScriptName` declaration"]);
}

#[test]
fn assignment_in_condition() {
    let src = "ScriptName A\nFunction F()\n  If x = 1\n  EndIf\nEndFunction\n";
    assert_eq!(errors(src), ["33..34: `=` is an assignment, use `==` to compare"]);
}

#[test]
fn token_at_offset() {
    let src = "ScriptName Foo\n";
    let parse = parse(src);
    let root = parse.syntax_node();
    let t = root.token_at_offset(12.into()).right_biased().unwrap();
    assert_eq!(t.text(), "Foo");
    match root.token_at_offset(11.into()) {
        crate::TokenAtOffset::Between(l, r) => {
            assert_eq!(l.text(), " ");
            assert_eq!(r.text(), "Foo");
        }
        other => panic!("{other:?}"),
    }
    let t = root.token_at_offset(15.into()).left_biased().unwrap();
    assert_eq!(t.text(), "\n");
}

// === Corpus ===

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

fn collect_psc(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_psc(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("psc")) {
            out.push(path);
        }
    }
}

/// Every script of the test corpus compiles with the official compiler, so it
/// must parse without errors, and the tree must be lossless.
#[test]
fn corpus_parses_without_errors() {
    // Not `tests/playground`, which may hold scripts being written.
    let root = workspace_root().join("tests/corpus");
    let mut files = Vec::new();
    collect_psc(&root, &mut files);
    if files.len() < 100 {
        // The corpus is a git submodule that may not be checked out.
        #[allow(clippy::print_stderr)]
        {
            eprintln!("skipping corpus test: submodule not checked out");
        }
        return;
    }

    let mut failures = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let parse = parse(&text);
        assert_eq!(parse.syntax_node().text(), &*text, "{} is not lossless", path.display());
        if let Some(e) = parse.errors().first() {
            let line = text[..usize::from(e.range().start())].lines().count().max(1);
            failures.push(format!(
                "{}:{line}: {} ({} errors)",
                path.strip_prefix(&root).unwrap_or(path).display(),
                e.message(),
                parse.errors().len()
            ));
        }
    }
    if !failures.is_empty() {
        let shown = failures.iter().take(50).cloned().collect::<Vec<_>>().join("\n");
        panic!("{} of {} files have errors:\n{shown}", failures.len(), files.len());
    }
}
