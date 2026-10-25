use std::{path::PathBuf, sync::Arc};

use papyrus_ide::Workspace;

use crate::{Script, ScriptLoader, Value, Vm};

/// Compiles the scripts of a workspace in memory, with the built-in compiler.
struct Compiled(Workspace);

impl ScriptLoader for Compiled {
    fn load(&mut self, name: &str) -> Option<Script> {
        let (file, _) = self
            .0
            .files()
            .find(|(_, d)| d.index.name.is_some_and(|n| n.as_str().eq_ignore_ascii_case(name)))?;
        let pex = papyrus_compiler::compile(&self.0, file, &Default::default())
            .unwrap_or_else(|e| panic!("{name} does not compile: {e:?}"));
        Script::from_pex(&pex)
    }
}

const DEBUG: &str =
    "ScriptName Debug Hidden\nFunction Trace(String asText, Int aiSeverity = 0) Global Native\n";
const FORM: &str = "ScriptName Form Hidden\nInt Function GetFormID() Native\n";
const ACTOR: &str =
    "ScriptName Actor extends Form Hidden\nFloat Function GetActorValue(String asName) Native\n";

fn vm(scripts: &[(&str, &str)]) -> Vm {
    let mut ws = Workspace::new();
    for (name, text) in [("Debug", DEBUG), ("Form", FORM), ("Actor", ACTOR)].iter().chain(scripts) {
        ws.set_open(PathBuf::from(format!("/ws/{name}.psc")), Arc::from(*text), 0);
    }
    Vm::new(Compiled(ws))
}

#[test]
fn runs_arithmetic_loops_and_strings() {
    let src = "ScriptName T\nInt Function Sum(Int n) Global\n    Int total = 0\n    Int i = 1\n    While i <= n\n        total += i\n        i += 1\n    EndWhile\n    Debug.Trace(\"sum of \" + n + \" = \" + total)\n    Return total\nEndFunction\nFloat Function Half(Int x) Global\n    Return x / 2.0\nEndFunction\n";
    let mut vm = vm(&[("T", src)]);
    let v = vm.call_static("T", "Sum", &[Value::Int(10)]).unwrap();
    assert!(matches!(v, Value::Int(55)), "{v:?}");
    assert_eq!(vm.output, ["sum of 10 = 55"]);
    let half = vm.call_static("T", "Half", &[Value::Int(3)]).unwrap();
    assert_eq!(half.to_string(), "1.500000");
}

#[test]
fn dispatches_through_parents_states_and_properties() {
    let base = "ScriptName Base extends Form\nString Function Greet()\n    Return \"hello\"\nEndFunction\n";
    let child = "ScriptName Child extends Base\nInt Property Count = 1 Auto\nInt doubled\nInt Property Doubled\n    Int Function Get()\n        Return Count * 2\n    EndFunction\nEndProperty\nString Function Greet()\n    Return Parent.Greet() + \"+child\"\nEndFunction\nFunction Bump()\n    Count += 1\n    GotoState(\"Busy\")\nEndFunction\nState Busy\n    String Function Greet()\n        Return \"working\"\n    EndFunction\nEndState\n";
    let mut vm = vm(&[("Base", base), ("Child", child)]);
    let obj = vm.new_object("Child").unwrap();
    assert_eq!(vm.call(obj, "Greet", &[]).unwrap().to_string(), "hello+child");
    vm.call(obj, "Bump", &[]).unwrap();
    assert_eq!(vm.state_of(obj), "Busy");
    assert_eq!(vm.call(obj, "Greet", &[]).unwrap().to_string(), "working");
    assert_eq!(vm.call(obj, "GetState", &[]).unwrap().to_string(), "Busy");
    // The string table shares entries ignoring case, like the game: the
    // literal `base` would read `Base` (the script name).
    assert!(vm.errors.is_empty(), "{:?}", vm.errors);
}

#[test]
fn natives_none_and_failed_casts() {
    let src = "ScriptName T extends Form\nFloat Function Health(Form akForm) Global\n    Actor a = akForm as Actor\n    a.GetActorValue(\"Health\")\n    Return 1.0\nEndFunction\nFloat Function OwnHealth(Actor akActor) Global\n    Return akActor.GetActorValue(\"Health\")\nEndFunction\n";
    let mut vm = vm(&[("T", src)]);
    // A Form that is not an Actor: the cast gives None, the call fails.
    let form = vm.new_object("Form").unwrap();
    vm.call_static("T", "Health", &[Value::Object(form)]).unwrap();
    assert_eq!(vm.errors, ["Cannot call getactorvalue() on a None object"]);

    // An Actor: the native returns its default, and is recorded.
    let actor = vm.new_object("Actor").unwrap();
    let hp = vm.call_static("T", "OwnHealth", &[Value::Object(actor)]).unwrap();
    assert_eq!(hp.as_float(), 0.0);
    let call = vm.native_calls.last().unwrap();
    assert_eq!(
        (call.script.as_str(), call.function.as_str(), call.implemented),
        ("Actor", "GetActorValue", false)
    );
    assert_eq!(call.args[0].to_string(), "Health");

    // A mock.
    vm.register_native("Actor", "GetActorValue", |_, _, _| Value::Float(50.0));
    let hp = vm.call_static("T", "OwnHealth", &[Value::Object(actor)]).unwrap();
    assert_eq!(hp.as_float(), 50.0);
}
