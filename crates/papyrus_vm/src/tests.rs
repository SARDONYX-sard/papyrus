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
const FORM: &str = "ScriptName Form Hidden\nInt Function GetFormID() Native\nFunction RegisterForUpdate(Float afInterval) Native\nFunction RegisterForSingleUpdate(Float afInterval) Native\nFunction UnregisterForUpdate() Native\nFunction RegisterForSingleUpdateGameTime(Float afInterval) Native\n";
const UTILITY: &str = "ScriptName Utility Hidden\nFunction Wait(Float afSeconds) Global Native\nFunction WaitGameTime(Float afHours) Global Native\n";
const ACTOR: &str =
    "ScriptName Actor extends Form Hidden\nFloat Function GetActorValue(String asName) Native\n";

fn vm(scripts: &[(&str, &str)]) -> Vm {
    let mut ws = Workspace::new();
    let builtins = [("Debug", DEBUG), ("Form", FORM), ("Actor", ACTOR), ("Utility", UTILITY)];
    for (name, text) in builtins.iter().chain(scripts) {
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

#[test]
fn asserts_record_failures() {
    let src = "ScriptName T\nFunction Check(Int x) Global\n    Assert.IntEq(x, 3, \"three\")\n    Assert.FloatEq(x / 2.0, 1.5)\n    Assert.IsTrue(x > 0)\nEndFunction\n";
    let mut vm = vm(&[("T", src), ("Assert", crate::ASSERT_PSC)]);
    vm.call_static("T", "Check", &[Value::Int(3)]).unwrap();
    assert!(vm.errors.is_empty(), "{:?}", vm.errors);
    vm.call_static("T", "Check", &[Value::Int(-1)]).unwrap();
    assert_eq!(
        vm.errors,
        [
            "assertion failed: expected 3, got -1: three",
            "assertion failed: expected 1.5, got -0.5",
            "assertion failed: expected True",
        ]
    );
}

#[test]
fn mocks_replace_natives() {
    let src = "ScriptName T extends Form\nInt calls\nFloat Function Health(Actor akActor)\n    Return akActor.GetActorValue(\"Health\")\nEndFunction\nFloat Function MockAV(Actor akSelf, String asName)\n    calls += 1\n    If akSelf && asName == \"Health\"\n        Return 50.0\n    EndIf\n    Return 0.0\nEndFunction\nInt Function Calls()\n    Return calls\nEndFunction\n";
    let mut vm = vm(&[("T", src)]);
    let test = vm.new_object("T").unwrap();
    let actor = vm.new_object("Actor").unwrap();
    vm.mock("Actor.GetActorValue", test, "MockAV");
    let hp = vm.call(test, "Health", &[Value::Object(actor)]).unwrap();
    assert_eq!(hp.as_float(), 50.0);
    assert_eq!(vm.call(test, "Calls", &[]).unwrap().as_int(), 1);
    vm.clear_mocks();
    assert_eq!(vm.call(test, "Health", &[Value::Object(actor)]).unwrap().as_float(), 0.0);
}

#[test]
fn waits_send_update_events() {
    let src = "ScriptName T extends Form\nInt ticks\nInt nights\nEvent OnInit()\n    RegisterForUpdate(1.0)\n    RegisterForSingleUpdateGameTime(1.0)\nEndEvent\nEvent OnUpdate()\n    ticks += 1\n    If ticks == 3\n        UnregisterForUpdate()\n    EndIf\nEndEvent\nEvent OnUpdateGameTime()\n    nights += 1\nEndEvent\nFunction Run()\n    Utility.Wait(2.5)\n    Debug.Trace(ticks)\n    Utility.Wait(10.0)\n    Debug.Trace(ticks)\n    Utility.WaitGameTime(1.0)\n    Debug.Trace(nights)\nEndFunction\n";
    let mut vm = vm(&[("T", src)]);
    let obj = vm.new_object("T").unwrap();
    vm.send_event(obj, "OnInit", &[]).unwrap();
    // Not defined: nothing happens.
    vm.send_event(obj, "OnActivate", &[Value::None]).unwrap();
    vm.call(obj, "Run", &[]).unwrap();
    assert_eq!(vm.output, ["2", "3", "1"]);
    assert_eq!(vm.clock, 12.5 + 180.0);
    assert!(vm.errors.is_empty(), "{:?}", vm.errors);
}

#[test]
fn errors_of_events_stop_the_wait() {
    let src = "ScriptName T extends Form\nEvent OnUpdate()\n    While True\n    EndWhile\nEndEvent\nFunction Run()\n    RegisterForSingleUpdate(1.0)\n    Utility.Wait(2.0)\n    Debug.Trace(\"after\")\nEndFunction\n";
    let mut vm = vm(&[("T", src)]);
    vm.step_limit = 10_000;
    let obj = vm.new_object("T").unwrap();
    assert_eq!(vm.call(obj, "Run", &[]).unwrap_err(), crate::VmError::StepLimit);
    assert!(vm.output.is_empty());
}
