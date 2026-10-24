use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{features::completion::Completions, *};

/// Builds a workspace from `(file name, text)` pairs, all opened in the editor.
/// The cursor of the first file containing `$0` is returned.
fn workspace(files: &[(&str, &str)]) -> (Workspace, FileId, TextSize) {
    let mut ws = Workspace::new();
    let mut cursor = None;
    for (name, text) in files {
        let path = PathBuf::from(format!("/ws/{name}"));
        let (text, offset) = match text.find("$0") {
            Some(i) => (text.replacen("$0", "", 1), Some(i)),
            None => ((*text).to_owned(), None),
        };
        let id = ws.set_open(path, Arc::from(text), 0);
        if let Some(offset) = offset {
            cursor = Some((id, TextSize::new(offset as u32)));
        }
    }
    let (file, offset) = cursor.expect("no $0 cursor");
    (ws, file, offset)
}

fn script_of(file: NavFile) -> FileId {
    match file {
        NavFile::Script(f) => f,
        NavFile::Flags => panic!("expected a script"),
    }
}

const FORM: &str = "ScriptName Form Hidden\nInt Function GetFormID() Native\n";
const ACTOR: &str = "ScriptName Actor extends Form Hidden\n; Kills the actor.\nFunction Kill(Actor akKiller = None) Native\nFloat Function GetAV(String asName) Native\nEvent OnDeath(Actor akKiller)\nEndEvent\n";
const DEBUG: &str =
    "ScriptName Debug Hidden\nFunction Trace(String asText, Int aiSeverity = 0) Global Native\n";

#[test]
fn hover_member_with_docs() {
    let (ws, file, offset) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Test.psc", "ScriptName Test\nFunction F(Actor a)\n  a.Ki$0ll()\nEndFunction\n"),
    ]);
    let hover = hover(&ws, file, offset).unwrap();
    assert_eq!(
        hover.markdown,
        "```papyrus\nFunction Kill(Actor akKiller = None) Native\n```\n*Actor*\n\n---\n\nKills the actor."
    );
}

#[test]
fn hover_local_and_global() {
    let (ws, file, offset) = workspace(&[
        ("Debug.psc", DEBUG),
        (
            "Test.psc",
            "ScriptName Test\nFunction F()\n  Int count = 1\n  Debug.Trace(\"x\" + cou$0nt)\nEndFunction\n",
        ),
    ]);
    assert_eq!(
        hover(&ws, file, offset).unwrap().markdown,
        "```papyrus\nInt count\n```\n*local variable*"
    );

    let (ws, file, offset) = workspace(&[
        ("Debug.psc", DEBUG),
        ("Test.psc", "ScriptName Test\nFunction F()\n  Debug.Tr$0ace(\"x\")\nEndFunction\n"),
    ]);
    assert!(
        hover(&ws, file, offset)
            .unwrap()
            .markdown
            .contains("Function Trace(String asText, Int aiSeverity = 0) Global Native")
    );
}

#[test]
fn goto_inherited_and_imported() {
    let (ws, file, offset) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Debug.psc", DEBUG),
        (
            "Test.psc",
            "ScriptName Test extends Actor\nImport Debug\nEvent OnInit()\n  GetForm$0ID()\n  Trace(\"\")\nEndEvent\n",
        ),
    ]);
    let targets = goto_definition(&ws, file, offset);
    assert_eq!(targets.len(), 1);
    assert_eq!(ws.file(script_of(targets[0].file)).path, Path::new("/ws/Form.psc"));

    let text = ws.text(file).unwrap();
    let trace = TextSize::new(text.find("Trace").unwrap() as u32 + 1);
    let targets = goto_definition(&ws, file, trace);
    assert_eq!(ws.file(script_of(targets[0].file)).path, Path::new("/ws/Debug.psc"));
}

#[test]
fn goto_property_through_cast_and_index() {
    let (ws, file, offset) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        (
            "Test.psc",
            "ScriptName Test\nForm[] forms\nFunction F()\n  (forms[0] as Actor).Kil$0l()\nEndFunction\n",
        ),
    ]);
    let targets = goto_definition(&ws, file, offset);
    assert_eq!(ws.file(script_of(targets[0].file)).path, Path::new("/ws/Actor.psc"));
}

#[test]
fn locals_shadowing_and_scope() {
    let src = "ScriptName Test\nInt x\nFunction F(Int x)\n  If True\n    Int y = 1\n  EndIf\n  x$0 = y\nEndFunction\n";
    let (ws, file, offset) = workspace(&[("Test.psc", src)]);
    let targets = goto_definition(&ws, file, offset);
    // The parameter shadows the script variable.
    assert_eq!(targets[0].focus_range, TextRange::at(TextSize::new(37), TextSize::new(1)));
    // `y` is out of scope after `EndIf`.
    let diags: Vec<_> = diagnostics(&ws, file).into_iter().map(|d| d.code).collect();
    assert!(!diags.contains(&"syntax"));
}

#[test]
fn references_across_files() {
    let (ws, file, offset) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("A.psc", "ScriptName A\nFunction F(Actor a)\n  a.Kill()\n  a.Kill(a)\nEndFunction\n"),
        ("B.psc", "ScriptName B extends Actor\nEvent OnInit()\n  Ki$0ll()\nEndEvent\n"),
    ]);
    let refs = references(&ws, file, offset, true);
    assert_eq!(refs.len(), 4, "{refs:?}");
    let refs = references(&ws, file, offset, false);
    assert_eq!(refs.len(), 3, "{refs:?}");
}

fn labels(c: &Completions) -> Vec<&str> {
    c.items.iter().map(|i| i.label.as_str()).collect()
}

#[test]
fn complete_members() {
    let (ws, file, offset) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Test.psc", "ScriptName Test\nFunction F(Actor a)\n  a.$0\nEndFunction\n"),
    ]);
    let c = crate::features::completion::completions(&ws, file, offset);
    let l = labels(&c);
    assert!(l.contains(&"Kill") && l.contains(&"GetAV") && l.contains(&"GetFormID"), "{l:?}");
    assert!(l.contains(&"GoToState"));
}

#[test]
fn complete_static_and_arrays() {
    let (ws, file, offset) = workspace(&[
        ("Debug.psc", DEBUG),
        ("Test.psc", "ScriptName Test\nFunction F(Int[] arr)\n  Debug.T$0\n  arr.\nEndFunction\n"),
    ]);
    let c = crate::features::completion::completions(&ws, file, offset);
    assert_eq!(labels(&c), ["Trace"]);
    assert_eq!(c.items[0].replace_range, TextRange::new(46.into(), 47.into()));

    let text = ws.text(file).unwrap();
    let after_dot = TextSize::new(text.find("arr.").unwrap() as u32 + 4);
    let c = crate::features::completion::completions(&ws, file, after_dot);
    assert_eq!(labels(&c), ["Length", "Find", "RFind"]);
}

#[test]
fn complete_locals_in_body() {
    let (ws, file, offset) = workspace(&[(
        "Test.psc",
        "ScriptName Test\nInt Property P Auto\nFunction F(Int param)\n  Int local = 1\n  $0\nEndFunction\n",
    )]);
    let c = crate::features::completion::completions(&ws, file, offset);
    let l = labels(&c);
    for expected in ["param", "local", "P", "F", "If", "While", "Int"] {
        assert!(l.contains(&expected), "missing {expected}: {l:?}");
    }
    assert!(!l.contains(&"EndState"));
}

#[test]
fn complete_event_overrides() {
    let (ws, file, offset) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Test.psc", "ScriptName Test extends Actor\nEvent $0\n"),
    ]);
    let c = crate::features::completion::completions(&ws, file, offset);
    let item = c.items.iter().find(|i| i.label == "OnDeath(Actor akKiller)").unwrap();
    assert_eq!(item.snippet.as_deref(), Some("OnDeath(Actor akKiller)\n\t$0\nEndEvent"));
}

#[test]
fn signature_help_active_param() {
    let (ws, file, offset) = workspace(&[
        ("Debug.psc", DEBUG),
        ("Test.psc", "ScriptName Test\nFunction F()\n  Debug.Trace(\"x\", $0)\nEndFunction\n"),
    ]);
    let help = signature_help(&ws, file, offset).unwrap();
    assert_eq!(help.label, "Trace(String asText, Int aiSeverity = 0)");
    assert_eq!(help.active_param, Some(1));
    assert_eq!(help.params, [(6, 19), (21, 39)]);
}

#[test]
fn diagnostics_semantic() {
    let (ws, file, _) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        (
            "Wrong.psc",
            "ScriptName Test$0 extends Actor\nInt x\nInt x\nFunction F() Gloabl\n  Kill()\n  Fly()\n  Self.Swim()\n  Missing m\n  GoToState(\"Nowhere\")\nEndFunction\nState S\n  Function OnlyHere()\n  EndFunction\nEndState\n",
        ),
    ]);
    let diags: Vec<String> =
        diagnostics(&ws, file).into_iter().map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert_eq!(
        diags,
        [
            "script-name-mismatch: script name `Test` does not match the file name `Wrong.psc`",
            "duplicate-definition: `x` is already defined in this scope",
            "unknown-flag: unknown flag `Gloabl`, not defined in built-in Skyrim flags",
            "unresolved-name: cannot find `Fly` in this scope",
            "unknown-member: `Test` has no member `Swim`",
            "unknown-type: unknown type `Missing`",
            "state-function-not-declared: function `OnlyHere` in state `S` must also be declared outside of any state",
        ]
    );
}

#[test]
fn parameter_inlay_hints() {
    let (ws, file, _) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Debug.psc", DEBUG),
        (
            "Test.psc",
            "ScriptName Test$0\nFunction F(Actor killer, Int[] arr)\n  killer.Kill(killer)\n  killer.Kill(None)\n  Debug.Trace(\"x\", aiSeverity = 1)\n  Debug.Trace(\"x\", 2)\n  arr.Find(3)\nEndFunction\n",
        ),
    ]);
    let text = ws.text(file).unwrap();
    let hints: Vec<String> = inlay_hints(&ws, file, None)
        .into_iter()
        .map(|h| format!("{} {}", h.label, &text[usize::from(h.offset)..usize::from(h.offset) + 4]))
        .collect();
    // `killer` repeats `akKiller`; named arguments get no hint.
    assert_eq!(
        hints,
        [
            "akKiller: None",
            "asText: \"x\",",
            "asText: \"x\",",
            "aiSeverity: 2)\n ",
            "akElement: 3)\nE"
        ]
    );
}

#[test]
fn type_errors() {
    let (ws, file, _) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Debug.psc", DEBUG),
        (
            "Test.psc",
            r#"ScriptName Test$0
Int Property RO = 1 AutoReadOnly
Int Property Full
    Int Function Get()
        Return 1
    EndFunction
EndProperty
Int bad = 1.5
Int Function F(Actor a, Form f, Int[] arr)
    Int i = 1.5
    Float ok = 1
    String s = a
    Actor down = f
    Actor fine = f as Actor
    i = "x" * 2
    i = arr[1.0]
    i = i[0]
    a.Kill(1, 2)
    a.GetAV()
    Debug.Trace("x", aiSeverity = "high")
    Actor.Kill()
    RO = 2
    Full = 3
    Return
EndFunction
Function G()
    Return 1
EndFunction
"#,
        ),
    ]);
    let diags: Vec<String> = diagnostics(&ws, file)
        .into_iter()
        .filter(|d| d.needs_workspace())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect();
    assert_eq!(
        diags,
        [
            "type-mismatch: expected `Int`, found `Float`",
            "type-mismatch: expected `Actor`, found `Form`",
            "invalid-operator: cannot apply `*` to `String` and `Int`",
            "type-mismatch: expected `Int`, found `Float`",
            "not-an-array: `Int` is not an array",
            "type-mismatch: expected `Actor`, found `Int`",
            "wrong-arg-count: `Kill` takes 1 arguments, but more were given",
            "wrong-arg-count: missing arguments for `GetAV`: asName",
            "type-mismatch: expected `Int`, found `String`",
            "not-global: `Kill` is not a Global function and needs an object, not the script `Actor`",
            "read-only: property `RO` is read-only",
            "read-only: property `Full` is read-only",
            "missing-return-value: this function must return a `Int`",
            "unexpected-return-value: this function does not return a value",
        ],
        "{diags:#?}"
    );
}

/// Rules found by comparing with the official compiler.
#[test]
fn compiler_rules() {
    let (ws, file, _) = workspace(&[
        ("Form.psc", FORM),
        ("Actor.psc", ACTOR),
        ("Debug.psc", DEBUG),
        (
            "Test.psc",
            r#"ScriptName Test$0
Int speed
Int Property Speed Auto
Int Property Length Auto
Float Property F1 = 1 Auto
Float Property F2 = 1 AutoReadOnly
String s1 = None
Int Property P = 1 + 1 Auto
Int v = 1 Bogus
Function Fn(Int speed, Bool b) Conditional
    Int b = 1
    Int Debug = 2
    Int x = 1
    If b
        Int x = 2
        Int y = 1
    Else
        Int y = 2
    EndIf
    Int[] arr = new Int[0]
    arr = new Int[x]
EndFunction
Function G(Actor a, Debug d) Global
    Self.Fn(1, True)
    Bool same = a == d
EndFunction
String Function GetState()
    Return ""
EndFunction
"#,
        ),
    ]);
    let diags: Vec<String> =
        diagnostics(&ws, file).into_iter().map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert_eq!(
        diags,
        [
            "reserved-name: `Length` is reserved for the length of arrays and cannot be a name",
            "initializer-mismatch: cannot initialize a `Float` with `1`: initial values are not converted",
            "initializer-mismatch: cannot initialize a `String` with `None`: initial values are not converted",
            "non-literal-initializer: initial values must be literals",
            "unknown-flag: unknown flag `Bogus`, not defined in built-in Skyrim flags",
            "name-conflict: `speed` is already defined as a variable of the script",
            "misplaced-flag: flag `Conditional` is not allowed on functions (allowed on: Script, Variable)",
            "name-conflict: `b` is already defined as a parameter",
            "name-is-type: `Debug` is the name of a script; variables and properties cannot use it",
            "name-conflict: `x` is already defined in an enclosing block, and locals cannot shadow it",
            "array-size: arrays must have between 1 and 128 elements",
            "array-size: the size of a new array must be an integer literal (1 to 128)",
            "global-self: `Self` does not exist in a Global function",
            "invalid-operator: cannot apply `==` to `Actor` and `Debug`",
            "builtin-redefined: `GetState` is generated by the compiler and cannot be defined",
        ],
        "{diags:#?}"
    );
}

#[test]
fn document_symbols_nest_states_and_accessors() {
    let (ws, file, _) = workspace(&[(
        "Test.psc",
        "ScriptName Test$0\nInt Property P\n  Int Function Get()\n    Return 1\n  EndFunction\nEndProperty\nState S\n  Event OnInit()\n  EndEvent\nEndState\n",
    )]);
    let symbols = document_symbols(&ws, file);
    assert_eq!(symbols.len(), 1);
    let children: Vec<_> =
        symbols[0].children.iter().map(|s| (s.name.as_str(), s.children.len())).collect();
    assert_eq!(children, [("P", 1), ("S", 1)]);
}

const FLAGS: &str = "// Hides things.\nFlag Hidden 0 { Script Property }\nFlag Conditional 1 { Script Variable }\nFlag Everywhere 5\n";

fn with_flags(files: &[(&str, &str)]) -> (Workspace, FileId, TextSize) {
    let (mut ws, file, offset) = workspace(files);
    ws.set_flags_file(PathBuf::from("/ws/Custom.flg"), Arc::from(FLAGS));
    (ws, file, offset)
}

#[test]
fn flag_completion_depends_on_declaration() {
    let complete = |src: &str| {
        let (ws, file, offset) = with_flags(&[("Test.psc", src)]);
        let c = crate::features::completion::completions(&ws, file, offset);
        let mut l: Vec<String> = c.items.iter().map(|i| i.label.clone()).collect();
        l.sort();
        l
    };
    assert_eq!(complete("ScriptName Test $0\n"), ["Conditional", "Everywhere", "Hidden", "Native"]);
    assert_eq!(
        complete("ScriptName Test\nFunction F() $0\nEndFunction\n"),
        ["Everywhere", "Global", "Native"]
    );
    assert_eq!(
        complete("ScriptName Test\nInt Property P = 1 Auto $0\n"),
        ["Conditional", "Everywhere", "Hidden"]
    );
    assert_eq!(complete("ScriptName Test\nInt x = 1 H$0\n"), ["Conditional", "Everywhere"]);
    assert_eq!(
        complete("ScriptName Test\nInt Property P $0\n"),
        ["Auto", "AutoReadOnly", "Everywhere", "Hidden"]
    );
}

#[test]
fn flag_hover_goto_and_references() {
    let (ws, file, offset) = with_flags(&[
        ("Test.psc", "ScriptName Test Hid$0den\nInt Property P Auto Hidden\n"),
        ("Other.psc", "ScriptName Other Hidden\n"),
    ]);
    let h = hover(&ws, file, offset).unwrap();
    assert_eq!(
        h.markdown,
        "```papyrus\nFlag Hidden 0 { Script Property }\n```\n*Custom.flg*\n\n---\n\nHides things."
    );
    let targets = goto_definition(&ws, file, offset);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].file, NavFile::Flags);
    assert_eq!(&FLAGS[targets[0].focus_range], "Hidden");

    let refs = references(&ws, file, offset, true);
    assert_eq!(refs.len(), 4, "{refs:?}");
    let from_flg = flag_references(&ws, targets[0].focus_range.start(), false);
    assert_eq!(from_flg.len(), 3);

    assert!(flag_file_hover(&ws, targets[0].focus_range.start()).is_some());
    assert_eq!(flag_file_symbols(&ws).len(), 3);
}

#[test]
fn keyword_hover_has_examples() {
    let (ws, file, offset) = workspace(&[(
        "Test.psc",
        "ScriptName Test\nFunction F()\n  Wh$0ile True\n  EndWhile\nEndFunction\n",
    )]);
    let h = hover(&ws, file, offset).unwrap();
    assert!(h.markdown.starts_with("```papyrus\nWhile\n```\n*keyword*"), "{}", h.markdown);
    assert!(h.markdown.contains("# Examples\n\n```papyrus\nInt i = 0"), "{}", h.markdown);

    let (ws, file, offset) = workspace(&[(
        "Test.psc",
        "ScriptName Test\nFunction F()\n  Se$0lf.Disable()\nEndFunction\n",
    )]);
    let h = hover(&ws, file, offset).unwrap();
    assert!(
        h.markdown.contains("Refers to:") && h.markdown.contains("ScriptName Test"),
        "{}",
        h.markdown
    );

    let (ws, _, _) = with_flags(&[("Test.psc", "ScriptName Test$0\n")]);
    let at = |needle: &str| TextSize::new(FLAGS.find(needle).unwrap() as u32 + 1);
    assert!(flag_file_hover(&ws, at("Flag Hidden")).unwrap().markdown.contains("bit index"));
    assert!(
        flag_file_hover(&ws, at("Property }")).unwrap().markdown.contains("property declarations")
    );
    assert!(
        flag_file_hover(&ws, at("Hidden 0"))
            .unwrap()
            .markdown
            .starts_with("```papyrus\nFlag Hidden 0")
    );
    assert!(flag_file_hover(&ws, at("Hides")).is_none(), "comments have no hover");
}

#[test]
fn flag_file_completion() {
    let (ws, _, _) = with_flags(&[("Test.psc", "ScriptName Test$0\n")]);
    let at = |needle: &str| TextSize::new((FLAGS.find(needle).unwrap() + needle.len()) as u32);
    let labels = |offset| -> Vec<String> {
        flag_file_completions(&ws, offset).items.into_iter().map(|i| i.label).collect()
    };
    // Inside `{ Script Property }` of Hidden: only the missing targets.
    assert_eq!(labels(at("{ Script ")), ["Variable", "Function"]);
    assert_eq!(labels(TextSize::of(FLAGS)), ["Flag", "Flag"]);
    let snippet = flag_file_completions(&ws, TextSize::of(FLAGS)).items[0].snippet.clone().unwrap();
    assert!(snippet.contains("${2:2}"), "next free index: {snippet}");
    assert!(labels(at("// Hid")).is_empty());
}

#[test]
fn misplaced_flags() {
    let (ws, file, _) = with_flags(&[(
        "Test.psc",
        "ScriptName Test$0\nFunction F() Hidden Global\nEndFunction\nInt x Hidden\nInt Property P Auto Conditional\nInt Property Q Global Auto\n",
    )]);
    let diags: Vec<String> = diagnostics(&ws, file).into_iter().map(|d| d.message).collect();
    assert_eq!(
        diags,
        [
            "flag `Hidden` is not allowed on functions (allowed on: Script, Property)",
            "flag `Hidden` is not allowed on variables (allowed on: Script, Property)",
            "flag `Global` is not allowed on properties (allowed on: Function)",
        ]
    );
}

// === Corpus ===

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

/// The corpus compiles with the official compiler, so semantic checks must not
/// report anything on it. Each mod is checked against itself plus the vanilla
/// and SKSE sources of `psc_deps`.
#[test]
fn corpus_has_no_semantic_diagnostics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests/fixtures/papyrus-compiler-test-sources");
    let deps = root.join("psc_deps");
    let mut dep_files = Vec::new();
    collect_psc(&deps, &mut dep_files);
    if dep_files.len() < 100 {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("skipping corpus test: submodule not checked out");
        }
        return;
    }

    let mut ws = Workspace::new();
    let flg = deps.join("TESV_Papyrus_Flags.flg");
    ws.set_flags_file(flg.clone(), Arc::from(read_file(&flg).unwrap()));
    assert!(ws.flags().errors.is_empty());
    for path in &dep_files {
        let text = read_file(path).unwrap();
        ws.set_disk_index(path.clone(), index_text(&text), Origin::Import(0));
    }
    let mut files = Vec::new();
    collect_psc(&root, &mut files);
    files.retain(|p| !p.starts_with(&deps));
    let ids: Vec<_> = files
        .iter()
        .map(|p| {
            let text = read_file(p).unwrap();
            ws.set_disk_index(p.clone(), index_text(&text), Origin::Workspace)
        })
        .collect();

    let mut failures = Vec::new();
    let checked = ids.len();
    for id in ids {
        // The corpus puts unrelated mods in one workspace: a script of one mod
        // is not a known type for another.
        for d in diagnostics(&ws, id).into_iter().filter(|d| d.code != "name-is-type") {
            let text = ws.text(id).unwrap();
            let line = text[..usize::from(d.range.start())].lines().count().max(1);
            let path = ws
                .file(id)
                .path
                .strip_prefix(&root)
                .unwrap_or(&ws.file(id).path)
                .display()
                .to_string();
            failures.push(format!("{path}:{line}: [{}] {}", d.code, d.message));
        }
    }
    if !failures.is_empty() {
        let mut by_code = std::collections::BTreeMap::<String, usize>::new();
        for f in &failures {
            let code = f.split('[').nth(1).and_then(|s| s.split(']').next()).unwrap_or("?");
            *by_code.entry(code.to_owned()).or_default() += 1;
        }
        let mut shown_per_code = std::collections::BTreeMap::<String, usize>::new();
        let shown = failures
            .iter()
            .filter(|f| {
                let code =
                    f.split('[').nth(1).and_then(|s| s.split(']').next()).unwrap_or("?").to_owned();
                let n = shown_per_code.entry(code).or_default();
                *n += 1;
                *n <= 12
            })
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        panic!("{} diagnostics in {checked} files: {by_code:?}\n{shown}", failures.len());
    }
}
