use std::path::{Path, PathBuf};

use expect_test::{Expect, expect};

use super::*;

fn check(input: &str, expect: Expect) {
    let out = format(input, &FormatOptions::default()).unwrap();
    expect.assert_eq(&out);
    assert_eq!(format(&out, &FormatOptions::default()).unwrap(), out, "not idempotent");
}

#[test]
fn simple_fixture() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().join("tests/simple");
    let input = std::fs::read_to_string(root.join("test.psc")).unwrap();
    let expected = std::fs::read_to_string(root.join("expected_formatted.psc")).unwrap();
    // The fixtures only differ by the script name.
    let out = format(&input, &FormatOptions::default()).unwrap();
    assert_eq!(out.replace("ScriptName Test ", "ScriptName expected_formatted "), expected);
}

#[test]
fn indentation_and_keywords() {
    check(
        "scriptname Foo extends Quest hidden\n\nint property count=0 auto\n\nauto state Busy\nevent OnInit()\nif count>0&&!done\ncount-=1\nelseif count==0\n; nothing\nelse\nwhile (count<10)\ncount+=1\nendwhile\nendif\nendevent\nendstate\n",
        expect![[r#"
            ScriptName Foo extends Quest hidden

            Int Property count = 0 Auto

            Auto State Busy
                Event OnInit()
                    If count > 0 && !done
                        count -= 1
                    ElseIf count == 0
                        ; nothing
                    Else
                        While (count < 10)
                            count += 1
                        EndWhile
                    EndIf
                EndEvent
            EndState
        "#]],
    );
}

#[test]
fn spacing_of_calls_arrays_and_unary() {
    check(
        "ScriptName A\nFunction F( Int[ ] a , Float b=-1.0 )\nInt[] x=new Int[ 5 ]\nx[ 0 ]=Foo( a[1] , -b )as Int\nDebug . Trace ( \"s\" )\nEndFunction\n",
        expect![[r#"
            ScriptName A
            Function F(Int[] a, Float b = -1.0)
                Int[] x = New Int[5]
                x[0] = Foo(a[1], -b) as Int
                Debug.Trace("s")
            EndFunction
        "#]],
    );
}

#[test]
fn comments_blank_lines_and_continuations() {
    check(
        "ScriptName A\n\n\n\n; about F\nFunction F()\n\n  Int x = 1     ; aligned comment\n;/ block\n   comment /;\n  Foo(1, \\\n2)\n\n\nEndFunction\nInt Function G() Native Global\n{ Docs of G }\n",
        expect![[r#"
            ScriptName A

            ; about F
            Function F()
                Int x = 1     ; aligned comment
                ;/ block
               comment /;
                Foo(1, \
                    2)
            EndFunction
            Int Function G() Native Global
            { Docs of G }
        "#]],
    );
}

#[test]
fn properties_and_tabs() {
    let options = FormatOptions { indent: Indent::Tabs, ..FormatOptions::default() };
    let out = format(
        "ScriptName A\nfloat property Speed\nfloat function get()\nreturn 1.0\nendfunction\nendproperty\n",
        &options,
    )
    .unwrap();
    assert_eq!(
        out,
        "ScriptName A\nFloat Property Speed\n\tFloat Function get()\n\t\tReturn 1.0\n\tEndFunction\nEndProperty\n"
    );
}

#[test]
fn keeps_crlf_and_keyword_case_option() {
    let options = FormatOptions { keyword_case: Case::Preserve, ..FormatOptions::default() };
    let out = format("scriptname A\r\nfunction F()\r\nreturn\r\nendfunction", &options).unwrap();
    assert_eq!(out, "scriptname A\r\nfunction F()\r\n    return\r\nendfunction\r\n");
}

#[test]
fn keyword_cases_and_overrides() {
    let src = "scriptname A extends Quest\nfunction F()\nx = y as Actor\nendfunction\n";
    let with = |case: Case| {
        format(src, &FormatOptions { keyword_case: case, ..FormatOptions::default() }).unwrap()
    };
    assert!(
        with(Case::Canonical).starts_with(
            "ScriptName A extends Quest\nFunction F()\n    x = y as Actor\nEndFunction"
        )
    );
    assert!(
        with(Case::PascalCase).contains("ScriptName A Extends Quest")
            && with(Case::PascalCase).contains("y As Actor")
    );
    assert!(
        with(Case::CamelCase).contains("scriptName A extends")
            && with(Case::CamelCase).contains("endFunction")
    );
    assert!(with(Case::Lowercase).contains("endfunction"));
    assert!(with(Case::Uppercase).contains("SCRIPTNAME A EXTENDS"));
    assert!(with(Case::Preserve).contains("scriptname A extends"));

    let mut options = FormatOptions::default();
    options.keyword_overrides.insert("as".to_owned(), "AS".to_owned());
    options.keyword_overrides.insert("if".to_owned(), "nope".to_owned());
    assert!(format(src, &options).unwrap().contains("y AS Actor"));

    assert_eq!("camelCase".parse::<Case>(), Ok(Case::CamelCase));
    assert_eq!("UPPER_CASE".parse::<Case>(), Ok(Case::Uppercase));
    assert!("kebab".parse::<Case>().is_err());
}

#[test]
fn names_follow_their_definition() {
    let src = "scriptname foo extends quest hidden\nactor property a auto\nfunction f(objectreference r)\ndebug.trace(\"actor\")\nendfunction\n";
    // What the language server resolves, by start offset.
    let defs = [
        ("quest", NameKind::Script, "Quest"),
        ("hidden", NameKind::Flag, "Hidden"),
        ("actor property", NameKind::Script, "Actor"),
        ("objectreference", NameKind::Script, "ObjectReference"),
        ("debug", NameKind::Script, "Debug"),
        ("trace", NameKind::Function, "Trace"),
        ("r)", NameKind::Variable, "R"),
    ];
    let mut names = Names::default();
    for (at, kind, spelling) in defs {
        names.insert(src.find(at).unwrap() as u32, kind, spelling.to_owned());
    }
    let options = FormatOptions { names: names.into(), ..FormatOptions::default() };
    assert_eq!(
        format(src, &options).unwrap(),
        "ScriptName foo extends Quest Hidden\nActor Property a Auto\nFunction f(ObjectReference R)\n    Debug.Trace(\"actor\")\nEndFunction\n"
    );
    // Each kind has its case; `Canonical` is the definition.
    let with = |name_cases| {
        let options = FormatOptions { name_cases, ..options.clone() };
        format(src, &options).unwrap()
    };
    let out = with(NameCases {
        scripts: Case::Preserve,
        functions: Case::CamelCase,
        flags: Case::Uppercase,
        variables: Case::Preserve,
        ..NameCases::default()
    });
    assert!(out.starts_with("ScriptName foo extends quest HIDDEN"), "{out}");
    assert!(out.contains("debug.trace(") && out.contains("(objectreference r)"), "{out}");
}

#[test]
fn newline_options() {
    let options =
        FormatOptions { newline: Newline::CrLf, final_newline: false, ..FormatOptions::default() };
    assert_eq!(format("ScriptName A\nInt x\n", &options).unwrap(), "ScriptName A\r\nInt x");
}

#[test]
fn refuses_broken_scripts() {
    assert_eq!(
        format("ScriptName A\nFunction F(\n", &FormatOptions::default()),
        Err(FormatError::SyntaxErrors(2))
    );
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

/// Every script of the corpus formats safely (same tokens, same tree) and
/// idempotently.
#[test]
fn corpus_formats_safely_and_idempotently() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests/fixtures/papyrus-compiler-test-sources");
    let mut files = Vec::new();
    collect_psc(&root, &mut files);
    if files.len() < 100 {
        return;
    }
    let mut failures = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let options = FormatOptions::default();
        match format(&text, &options) {
            Ok(once) => {
                let twice = format(&once, &options).unwrap_or_default();
                if twice != once {
                    let line =
                        once.lines().zip(twice.lines()).position(|(a, b)| a != b).unwrap_or(0) + 1;
                    failures.push(format!("{}: not idempotent at line {line}", path.display()));
                }
            }
            Err(FormatError::SyntaxErrors(_)) => {}
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n")
    );
}
