//! `papyrus_analyzer run Script.Function [ARG]...`: runs a function in the
//! Papyrus virtual machine of [`papyrus_vm`], outside of the game.
//!
//! The scripts below the current directory and the imports of its
//! `papyrus.toml` (else the game scripts) are compiled in memory with the
//! built-in compiler, when the run needs them. Natives without implementation
//! return their default value; every native call is listed at the end.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::{path::PathBuf, rc::Rc};

use papyrus_ide::Workspace;
use papyrus_vm::{Script, ScriptLoader, Value, Vm};

const USAGE: &str = "\
usage: papyrus_analyzer run Script.Function [ARG]...

  ARG   Int (`3`), Float (`1.5`), Bool (`True`), None, or a String

Runs the function on a new object of Script, after its `OnInit`.
Debug.Trace prints; natives without implementation return their default
value (0, None, \"\"...) and are listed at the end. `Utility.Wait` sends
the `OnUpdate` events registered with `RegisterForUpdate`.";

const TEST_USAGE: &str = "\
usage: papyrus_analyzer test [PATH | FILTER]...

Runs the `; #[test]` functions of the scripts below the current directory,
each on a new object of its script, after its `OnInit`. A test fails when a
`PapyrusAnalyzer.Assert*` fails, when the game would log an error (a call on
None...) or when it returns False. `; #[mock(Script.Function)]` functions of
the test script replace the native in its tests.

The scripts of the [test] sources of papyrus.toml see the built-in script
`PapyrusAnalyzer` (`AssertIntEq`, `AssertTrue`, `Fail`...; `Import
PapyrusAnalyzer` calls them unqualified). A project script of that name wins.

  PATH     only the tests of the scripts of this file or directory
  FILTER   only the tests whose `Script.Function` contains it (any case)

options:
  -h, --help   print this help";

/// Compiles the scripts of the workspace when the machine loads them.
pub(crate) struct Compiled {
    ws: Workspace,
    imports: Vec<PathBuf>,
}

impl ScriptLoader for Compiled {
    fn load(&mut self, name: &str) -> Option<Script> {
        let find = |ws: &Workspace| {
            ws.files()
                .find(|(_, d)| d.index.name.is_some_and(|n| n.as_str().eq_ignore_ascii_case(name)))
                .map(|(f, d)| (f, d.path.clone()))
        };
        // The scripts its code uses, which the first load may have skipped
        // (only the declarations of the scripts the targets use are read).
        let (_, path) = find(&self.ws)?;
        papyrus_compiler::loader::load(&mut self.ws, &[path], &self.imports);
        let (file, _) = find(&self.ws)?;
        match papyrus_compiler::compile(&self.ws, file, &Default::default()) {
            Ok(pex) => Script::from_pex(&pex),
            Err(e) => {
                eprintln!("cannot compile {name}: {e:?}");
                None
            }
        }
    }
}

/// The loader of the scripts below the current directory, their imports and
/// the built-in `PapyrusAnalyzer`, which every script of the run sees.
pub(crate) fn loader(scripts: &[PathBuf]) -> Compiled {
    let (mut ws, imports) = crate::fmt::workspace_with_imports(scripts);
    let cwd = std::env::current_dir().unwrap_or_default();
    crate::builtin::install(&mut ws, &[cwd]);
    if let Some(path) = crate::builtin::shadowed_by(&ws) {
        eprintln!(
            "warning: {} hides the built-in `{}`",
            path.display(),
            papyrus_vm::PAPYRUS_ANALYZER
        );
    }
    Compiled { ws, imports }
}

/// An argument from the command line.
pub(crate) fn parse_arg(arg: &str) -> Value {
    if let Ok(i) = arg.parse() {
        return Value::Int(i);
    }
    if let Ok(f) = arg.parse() {
        return Value::Float(f);
    }
    match arg.to_ascii_lowercase().as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "none" => Value::None,
        _ => Value::Str(Rc::from(arg)),
    }
}

/// Runs the subcommand and returns the process exit code.
pub(crate) fn run(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return 0;
    }
    let Some((script, function)) = args.first().and_then(|a| a.split_once('.')) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let values: Vec<Value> = args[1..].iter().map(|a| parse_arg(a)).collect();

    let mut scripts: Vec<PathBuf> = Vec::new();
    collect(&std::env::current_dir().unwrap_or_default(), &mut scripts);
    let mut vm = Vm::new(loader(&scripts));
    let marked = marked_functions(&scripts);
    let result = start(&mut vm, &marked, script).and_then(|obj| vm.call(obj, function, &values));

    for line in &vm.output {
        println!("{line}");
    }
    for e in &vm.errors {
        eprintln!("error: {e}");
    }
    if !vm.native_calls.is_empty() {
        eprintln!("native calls:");
        for c in &vm.native_calls {
            let args: Vec<String> = c.args.iter().map(|a| format!("{a:?}")).collect();
            let mark = if c.implemented { "" } else { "  (not implemented: default value)" };
            eprintln!("  {}.{}({}){mark}", c.script, c.function, args.join(", "));
        }
    }
    match result {
        Ok(v) => {
            if !matches!(v, Value::None) {
                println!("=> {v}");
            }
            // Like `test`: errors or `False` fail, the exit code says so.
            if !vm.errors.is_empty() || matches!(v, Value::Bool(false)) {
                println!("{}  {script}.{function}", paint(RED, "FAIL"));
                1
            } else {
                println!("{}    {script}.{function}", paint(GREEN, "ok"));
                0
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

/// `papyrus_analyzer test`: runs the functions marked `; #[test]` of the
/// scripts below the current directory. A test fails when the machine
/// reports an error (a call on `None`...) or when it returns `False`:
///
/// ```papyrus
/// ; #[test]
/// Bool Function AddNumbersWorks()
///     Return AddNumbers(1, 2) == 3
/// EndFunction
/// ```
pub(crate) fn test(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{TEST_USAGE}");
        return 0;
    }
    let mut scripts: Vec<PathBuf> = Vec::new();
    collect(&std::env::current_dir().unwrap_or_default(), &mut scripts);
    let marked = marked_functions(&scripts);
    let tests: Vec<&Marked> =
        marked.iter().filter(|m| m.mocks.is_none() && selected(m, args)).collect();

    let mut vm = Vm::new(loader(&scripts));
    let mut failed = 0;
    for Marked { script, function, .. } in tests.iter().copied() {
        vm.output.clear();
        vm.errors.clear();
        let result = start(&mut vm, &marked, script).and_then(|obj| vm.call(obj, function, &[]));
        match problem(&vm, &result) {
            None => println!("{}    {script}.{function}", paint(GREEN, "ok")),
            Some(why) => {
                failed += 1;
                println!("{}  {script}.{function}: {why}", paint(RED, "FAIL"));
                for line in &vm.output {
                    println!("        {line}");
                }
            }
        }
    }
    let passed = paint(GREEN, &format!("{} passed", tests.len() - failed));
    let failures = format!("{failed} failed");
    let failures = if failed > 0 { paint(RED, &failures) } else { failures };
    println!("{passed}, {failures}");
    i32::from(failed > 0)
}

const GREEN: &str = "32";
const RED: &str = "1;31";

/// `text` in the color `sgr` (an ANSI code) when stdout is a terminal and
/// `NO_COLOR` is not set.
fn paint(sgr: &str, text: &str) -> String {
    use std::io::IsTerminal;
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    if color { format!("\x1b[{sgr}m{text}\x1b[0m") } else { text.to_owned() }
}

/// Whether the arguments of `test` select `m`: every test without any; else
/// the tests below a path argument, or whose name contains another one.
fn selected(m: &Marked, args: &[String]) -> bool {
    let name = format!("{}.{}", m.script, m.function).to_ascii_lowercase();
    let mut filters = args.iter().filter(|a| !a.starts_with('-')).peekable();
    if filters.peek().is_none() {
        return true;
    }
    let below = |dir: &std::path::Path| {
        let canonical =
            |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_owned());
        canonical(&m.path).starts_with(canonical(dir))
    };
    filters.any(|f| {
        let path = std::path::Path::new(f);
        if path.exists() { below(path) } else { name.contains(&f.to_ascii_lowercase()) }
    })
}

/// Why a test failed: an error of the machine, errors the game would log
/// (failed assertions among them), or `False` returned.
pub(crate) fn problem(vm: &Vm, result: &Result<Value, papyrus_vm::VmError>) -> Option<String> {
    match result {
        Err(e) => Some(e.to_string()),
        Ok(_) if !vm.errors.is_empty() => Some(vm.errors.join("; ")),
        Ok(Value::Bool(false)) => Some("returned False".to_owned()),
        Ok(_) => None,
    }
}

/// A function marked `; #[test]` or `; #[mock(Script.Function)]`.
pub(crate) struct Marked {
    /// The file of the script.
    path: PathBuf,
    script: String,
    function: String,
    /// The native a mock replaces; `None` for a test.
    mocks: Option<String>,
}

/// The `#[test]` and `#[mock]` functions of `scripts`.
pub(crate) fn marked_functions(scripts: &[PathBuf]) -> Vec<Marked> {
    use papyrus_parser::ast::{self, AstNode};
    let mut out = Vec::new();
    for path in scripts {
        let Some(text) = papyrus_ide::read_file(path) else { continue };
        if !text.contains("#[") {
            continue;
        }
        let parse = papyrus_parser::parse(&text);
        let file = parse.source_file();
        let Some(script) = file.header().and_then(|h| h.name()).map(|n| n.text().to_owned()) else {
            continue;
        };
        for attr in papyrus_parser::attrs::attributes(parse.syntax_node()) {
            let mocks = match attr.path.to_ascii_lowercase().as_str() {
                "test" => None,
                "mock" => Some(attr.args.first().map(|a| a.to_string()).unwrap_or_default()),
                _ => continue,
            };
            let node = parse.syntax_node().covering_node(attr.target);
            let func = std::iter::once(node).chain(node.ancestors()).find_map(ast::Function::cast);
            if let Some(name) = func.and_then(|f| f.name()) {
                out.push(Marked {
                    path: path.clone(),
                    script: script.clone(),
                    function: name.text().to_owned(),
                    mocks,
                });
            }
        }
    }
    out
}

/// A new object of `script`, its `#[mock]` functions replacing the natives,
/// initialized by its `OnInit` event.
pub(crate) fn start(
    vm: &mut Vm,
    marked: &[Marked],
    script: &str,
) -> Result<papyrus_vm::ObjId, papyrus_vm::VmError> {
    vm.clear_mocks();
    let obj = vm.new_object(script)?;
    for m in marked.iter().filter(|m| m.script.eq_ignore_ascii_case(script)) {
        if let Some(native) = &m.mocks {
            vm.mock(native, obj, &m.function);
        }
    }
    vm.send_event(obj, "OnInit", &[])?;
    Ok(obj)
}

pub(crate) fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            if !(name.starts_with('.') || name == "target" || name == "node_modules") {
                collect(&path, out);
            }
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("psc")) {
            out.push(path);
        }
    }
}
