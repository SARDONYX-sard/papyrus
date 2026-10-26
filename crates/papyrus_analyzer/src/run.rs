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

Runs the function on a new object of Script (`OnInit` is not called).
Debug.Trace prints; natives without implementation return their default
value (0, None, \"\"...) and are listed at the end.";

/// Compiles the scripts of the workspace when the machine loads them.
struct Compiled {
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

/// An argument from the command line.
fn parse_arg(arg: &str) -> Value {
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
    let Some((script, function)) = args.first().and_then(|a| a.split_once('.')) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let values: Vec<Value> = args[1..].iter().map(|a| parse_arg(a)).collect();

    let mut scripts: Vec<PathBuf> = Vec::new();
    collect(&std::env::current_dir().unwrap_or_default(), &mut scripts);
    let (ws, imports) = crate::fmt::workspace_with_imports(&scripts);
    let mut vm = Vm::new(Compiled { ws, imports });
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
                println!("FAIL  {script}.{function}");
                1
            } else {
                println!("ok    {script}.{function}");
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
    if args.iter().any(|a| a == "--init") {
        return init();
    }
    let mut scripts: Vec<PathBuf> = Vec::new();
    collect(&std::env::current_dir().unwrap_or_default(), &mut scripts);
    let marked = marked_functions(&scripts);
    let tests: Vec<&Marked> = marked.iter().filter(|m| m.mocks.is_none()).collect();

    let (ws, imports) = crate::fmt::workspace_with_imports(&scripts);
    let mut vm = Vm::new(Compiled { ws, imports });
    let mut failed = 0;
    for Marked { script, function, .. } in tests.iter().copied() {
        vm.output.clear();
        vm.errors.clear();
        let result = start(&mut vm, &marked, script).and_then(|obj| vm.call(obj, function, &[]));
        let problem = match &result {
            Err(e) => Some(e.to_string()),
            Ok(_) if !vm.errors.is_empty() => Some(vm.errors.join("; ")),
            Ok(Value::Bool(false)) => Some("returned False".to_owned()),
            Ok(_) => None,
        };
        match problem {
            None => println!("ok    {script}.{function}"),
            Some(why) => {
                failed += 1;
                println!("FAIL  {script}.{function}: {why}");
                for line in &vm.output {
                    println!("        {line}");
                }
            }
        }
    }
    println!("{} passed, {failed} failed", tests.len() - failed);
    i32::from(failed > 0)
}

/// A function marked `; #[test]` or `; #[mock(Script.Function)]`.
struct Marked {
    script: String,
    function: String,
    /// The native a mock replaces; `None` for a test.
    mocks: Option<String>,
}

/// The `#[test]` and `#[mock]` functions of `scripts`.
fn marked_functions(scripts: &[PathBuf]) -> Vec<Marked> {
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
fn start(
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

/// `test --init`: writes `Assert.psc` into the first `[test] sources`
/// directory of papyrus.toml, else `tests/`.
fn init() -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let vars = papyrus_project::Variables::for_command_line(&cwd);
    let dir = papyrus_project::find_manifest(&cwd)
        .and_then(|m| papyrus_project::load(&m, &vars).ok())
        .and_then(|p| p.test_sources.first().cloned())
        .unwrap_or_else(|| cwd.join("tests"));
    let path = dir.join("Assert.psc");
    if path.exists() {
        eprintln!("{} already exists", path.display());
        return 1;
    }
    match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, papyrus_vm::ASSERT_PSC))
    {
        Ok(()) => {
            println!("wrote {}", path.display());
            0
        }
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            1
        }
    }
}

fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
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
