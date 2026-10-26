//! `papyrus_analyzer debug`: a debug adapter (the Debug Adapter Protocol, over
//! stdio) running a test or a function in the virtual machine of
//! [`papyrus_vm`]: breakpoints, steps, the call stack and the variables.
//!
//! The launch arguments: `test` (`"Script.Function"`, a `; #[test]`
//! function) or `run` with `args`, `cwd` (the folder of papyrus.toml) and
//! `stopOnEntry`. The VM runs on the main thread; a thread reads the
//! requests, which the VM answers when it is paused. Watch expressions,
//! hovers and the debug console evaluate Papyrus expressions ([`eval`]).

#![allow(clippy::print_stderr)]

use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    rc::Rc,
};

use crossbeam_channel::Receiver;
use papyrus_vm::{Debugger, ObjId, Value, Vm, VmError};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::{Value as Json, json};

use crate::run;

mod eval;

/// The only thread of the machine.
const THREAD: i64 = 1;

/// Reads the messages of the client, until the end of stdin.
fn read_messages(tx: crossbeam_channel::Sender<Json>) {
    let mut input = BufReader::new(std::io::stdin().lock());
    loop {
        let mut length = None;
        loop {
            let mut line = String::new();
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                length = v.trim().parse::<usize>().ok();
            }
        }
        let Some(length) = length else { continue };
        let mut body = vec![0; length];
        if input.read_exact(&mut body).is_err() {
            return;
        }
        match serde_json::from_slice(&body) {
            Ok(msg) => {
                if tx.send(msg).is_err() {
                    return;
                }
            }
            Err(e) => eprintln!("invalid message: {e}"),
        }
    }
}

/// Writes the messages to the client.
struct Client {
    seq: i64,
}

impl Client {
    fn send(&mut self, mut msg: Json) {
        self.seq += 1;
        msg["seq"] = json!(self.seq);
        let body = msg.to_string();
        let mut out = std::io::stdout().lock();
        let _ = write!(out, "Content-Length: {}\r\n\r\n{body}", body.len());
        let _ = out.flush();
    }

    fn respond(&mut self, req: &Json, body: Json) {
        self.send(json!({
            "type": "response",
            "request_seq": req["seq"],
            "success": true,
            "command": req["command"],
            "body": body,
        }));
    }

    fn fail(&mut self, req: &Json, message: &str) {
        self.send(json!({
            "type": "response",
            "request_seq": req["seq"],
            "success": false,
            "command": req["command"],
            "message": message,
        }));
    }

    fn event(&mut self, event: &str, body: Json) {
        self.send(json!({ "type": "event", "event": event, "body": body }));
    }

    fn output(&mut self, category: &str, text: &str) {
        self.event("output", json!({ "category": category, "output": format!("{text}\n") }));
    }
}

/// The breakpoints, by lowercase script name, and the sources of the scripts.
#[derive(Default)]
struct Sources {
    breakpoints: FxHashMap<String, FxHashSet<u32>>,
    /// The file of each script, by lowercase name.
    paths: FxHashMap<String, PathBuf>,
}

impl Sources {
    /// Handles `setBreakpoints`.
    fn set_breakpoints(&mut self, client: &mut Client, req: &Json) {
        let args = &req["arguments"];
        let path = PathBuf::from(args["source"]["path"].as_str().unwrap_or_default());
        let lines: Vec<u32> = args["breakpoints"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["line"].as_u64().map(|l| l as u32))
            .collect();
        let script = papyrus_ide::read_file(&path)
            .and_then(|text| papyrus_ide::index_text(&text).name)
            .map(|n| n.as_str().to_ascii_lowercase());
        if let Some(script) = &script {
            self.paths.insert(script.clone(), path);
            self.breakpoints.insert(script.clone(), lines.iter().copied().collect());
        }
        let breakpoints: Vec<Json> =
            lines.iter().map(|l| json!({ "verified": script.is_some(), "line": l })).collect();
        client.respond(req, json!({ "breakpoints": breakpoints }));
    }

    fn source(&self, script: &str) -> Json {
        match self.paths.get(&script.to_ascii_lowercase()) {
            Some(path) => json!({
                "name": path.file_name().map(|n| n.to_string_lossy().into_owned()),
                "path": path,
            }),
            None => json!({ "name": format!("{script}.psc") }),
        }
    }
}

/// When to pause next.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// At breakpoints only.
    Run,
    /// At the next line.
    StepIn,
    /// At the next line of a frame at most this deep.
    StepOver(usize),
    /// At the next line of a frame less deep than this.
    StepOut(usize),
}

/// What a `variablesReference` shows: valid while paused.
enum Reference {
    Locals(usize),
    Object(ObjId),
    Array(Rc<RefCell<Vec<Value>>>),
}

/// The state shared by the adapter loop and the machine.
struct Session {
    client: Client,
    sources: Sources,
    requests: Receiver<Json>,
    mode: Mode,
    pause: bool,
    /// Pause at the first line (`stopOnEntry`).
    entry: bool,
    /// The client disconnected: stop the run.
    stopped: bool,
    references: Vec<Reference>,
    /// The lines of [`Vm::output`] sent already.
    printed: usize,
}

impl Session {
    fn flush_output(&mut self, vm: &Vm) {
        for line in &vm.output[self.printed.min(vm.output.len())..] {
            self.client.output("stdout", line);
        }
        self.printed = vm.output.len();
    }

    /// A value for the client, with the reference of its children.
    fn variable(&mut self, vm: &Vm, name: &str, value: &Value) -> Json {
        let (shown, reference) = match value {
            Value::Str(s) => (format!("{s:?}"), None),
            Value::Object(obj) => {
                (format!("{} #{}", vm.fields(*obj).0, obj.0), Some(Reference::Object(*obj)))
            }
            Value::Array(items) => (
                format!("[{} elements]", items.borrow().len()),
                Some(Reference::Array(items.clone())),
            ),
            other => (other.to_string(), None),
        };
        let reference = reference.map_or(0, |r| {
            self.references.push(r);
            self.references.len()
        });
        json!({ "name": name, "value": shown, "variablesReference": reference })
    }

    /// Answers a request while paused; returns `false` when the run goes on.
    fn paused_request(&mut self, vm: &Vm, req: &Json) -> bool {
        let depth = vm.stack().len();
        let mode = match req["command"].as_str().unwrap_or_default() {
            "continue" => Some(Mode::Run),
            "next" => Some(Mode::StepOver(depth)),
            "stepIn" => Some(Mode::StepIn),
            "stepOut" => Some(Mode::StepOut(depth)),
            _ => None,
        };
        if let Some(mode) = mode {
            self.mode = mode;
            self.references.clear();
            self.client.respond(req, json!({ "allThreadsContinued": true }));
            return false;
        }
        match req["command"].as_str().unwrap_or_default() {
            "stackTrace" => {
                let frames: Vec<Json> = (vm.stack().iter().enumerate().rev())
                    .map(|(i, f)| {
                        json!({
                            "id": i + 1,
                            "name": format!("{}.{}", f.script, f.function),
                            "source": self.sources.source(&f.script),
                            "line": f.line.unwrap_or(0),
                            "column": 1,
                        })
                    })
                    .collect();
                let total = frames.len();
                self.client.respond(req, json!({ "stackFrames": frames, "totalFrames": total }));
            }
            // Watch, hover and the debug console: an expression in a frame.
            "evaluate" => {
                let args = &req["arguments"];
                let frame = (args["frameId"].as_u64())
                    .and_then(|id| vm.stack().get((id as usize).wrapping_sub(1)))
                    .or_else(|| vm.stack().last());
                let text = args["expression"].as_str().unwrap_or_default();
                match eval::evaluate(vm, frame, text) {
                    Ok(value) => {
                        let v = self.variable(vm, text, &value);
                        self.client.respond(
                            req,
                            json!({
                                "result": v["value"],
                                "variablesReference": v["variablesReference"],
                            }),
                        );
                    }
                    Err(message) => self.client.fail(req, &message),
                }
            }
            "scopes" => {
                let index = req["arguments"]["frameId"].as_u64().unwrap_or(1) as usize - 1;
                let mut scopes = Vec::new();
                if index < vm.stack().len() {
                    self.references.push(Reference::Locals(index));
                    scopes.push(json!({
                        "name": "Locals",
                        "presentationHint": "locals",
                        "variablesReference": self.references.len(),
                        "expensive": false,
                    }));
                    if let Some(this) = vm.stack()[index].this {
                        self.references.push(Reference::Object(this));
                        scopes.push(json!({
                            "name": "Self",
                            "variablesReference": self.references.len(),
                            "expensive": false,
                        }));
                    }
                }
                self.client.respond(req, json!({ "scopes": scopes }));
            }
            "variables" => {
                let reference = req["arguments"]["variablesReference"].as_u64().unwrap_or(0);
                let values: Vec<(String, Value)> = match self
                    .references
                    .get((reference as usize).wrapping_sub(1))
                {
                    Some(Reference::Locals(i)) => {
                        vm.stack().get(*i).map(|f| f.locals.clone()).unwrap_or_default()
                    }
                    Some(Reference::Object(obj)) => {
                        let mut fields =
                            vec![("::State".to_owned(), Value::Str(Rc::from(vm.state_of(*obj))))];
                        fields.extend(vm.fields(*obj).1);
                        fields
                    }
                    Some(Reference::Array(items)) => (items.borrow().iter().enumerate())
                        .map(|(i, v)| (format!("[{i}]"), v.clone()))
                        .collect(),
                    None => Vec::new(),
                };
                let variables: Vec<Json> =
                    values.iter().map(|(name, value)| self.variable(vm, name, value)).collect();
                self.client.respond(req, json!({ "variables": variables }));
            }
            _ => self.request(req),
        }
        true
    }

    /// Answers a request that does not need a paused machine.
    fn request(&mut self, req: &Json) {
        match req["command"].as_str().unwrap_or_default() {
            "setBreakpoints" => self.sources.set_breakpoints(&mut self.client, req),
            "threads" => self
                .client
                .respond(req, json!({ "threads": [{ "id": THREAD, "name": "Papyrus VM" }] })),
            "pause" => {
                self.pause = true;
                self.client.respond(req, json!({}));
            }
            "disconnect" | "terminate" => {
                self.stopped = true;
                self.client.respond(req, json!({}));
            }
            "evaluate" => self.client.fail(req, "pause the program to evaluate"),
            _ => self.client.respond(req, json!({})),
        }
    }
}

/// The [`Debugger`] of the machine: pauses at breakpoints and steps.
struct Adapter(Rc<RefCell<Session>>);

impl Debugger for Adapter {
    fn line(&mut self, vm: &Vm) -> bool {
        let mut session = self.0.borrow_mut();
        session.flush_output(vm);
        while let Ok(req) = session.requests.try_recv() {
            session.request(&req);
        }
        if session.stopped {
            return false;
        }
        let Some(top) = vm.stack().last() else { return true };
        let depth = vm.stack().len();
        let breakpoint = top.line.is_some_and(|line| {
            session
                .sources
                .breakpoints
                .get(&top.script.to_ascii_lowercase())
                .is_some_and(|lines| lines.contains(&line))
        });
        let reason = match session.mode {
            _ if session.entry => "entry",
            _ if session.pause => "pause",
            _ if breakpoint => "breakpoint",
            Mode::StepIn => "step",
            Mode::StepOver(d) if depth <= d => "step",
            Mode::StepOut(d) if depth < d => "step",
            _ => return true,
        };
        (session.pause, session.entry) = (false, false);
        session.client.event(
            "stopped",
            json!({ "reason": reason, "threadId": THREAD, "allThreadsStopped": true }),
        );
        loop {
            let Ok(req) = session.requests.recv() else { return false };
            if !session.paused_request(vm, &req) {
                return true;
            }
            if session.stopped {
                return false;
            }
        }
    }
}

/// Runs the subcommand and returns the process exit code.
pub(crate) fn run(_args: &[String]) -> i32 {
    let (tx, requests) = crossbeam_channel::unbounded();
    std::thread::spawn(move || read_messages(tx));
    let mut client = Client { seq: 0 };
    let mut sources = Sources::default();

    // Configuration, until `launch` and `configurationDone`.
    let mut launch: Option<Json> = None;
    let mut configured = false;
    while launch.is_none() || !configured {
        let Ok(req) = requests.recv() else { return 0 };
        match req["command"].as_str().unwrap_or_default() {
            "initialize" => {
                client.respond(
                    &req,
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "supportsEvaluateForHovers": true,
                    }),
                );
                client.event("initialized", json!({}));
            }
            "launch" => {
                client.respond(&req, json!({}));
                launch = Some(req["arguments"].clone());
            }
            "configurationDone" => {
                client.respond(&req, json!({}));
                configured = true;
            }
            "setBreakpoints" => sources.set_breakpoints(&mut client, &req),
            "threads" => {
                client.respond(&req, json!({ "threads": [{ "id": THREAD, "name": "Papyrus VM" }] }))
            }
            "disconnect" | "terminate" => {
                client.respond(&req, json!({}));
                return 0;
            }
            _ => client.respond(&req, json!({})),
        }
    }
    let launch = launch.unwrap_or_default();
    if let Some(cwd) = launch["cwd"].as_str().filter(|c| !c.is_empty()) {
        let _ = std::env::set_current_dir(cwd);
    }
    let (target, args) = match (launch["test"].as_str(), launch["run"].as_str()) {
        (Some(test), _) => (test.to_owned(), Vec::new()),
        (None, Some(run)) => {
            let args = launch["args"].as_array().into_iter().flatten();
            (run.to_owned(), args.map(|a| run::parse_arg(&json_text(a))).collect())
        }
        _ => (String::new(), Vec::new()),
    };

    let mut scripts: Vec<PathBuf> = Vec::new();
    run::collect(&std::env::current_dir().unwrap_or_default(), &mut scripts);
    for path in &scripts {
        let name = papyrus_ide::read_file(path).and_then(|t| papyrus_ide::index_text(&t).name);
        if let Some(name) = name {
            sources.paths.entry(name.as_str().to_ascii_lowercase()).or_insert_with(|| path.clone());
        }
    }
    let session = Rc::new(RefCell::new(Session {
        client,
        sources,
        requests: requests.clone(),
        mode: Mode::Run,
        pause: false,
        entry: launch["stopOnEntry"].as_bool() == Some(true),
        stopped: false,
        references: Vec::new(),
        printed: 0,
    }));

    let result = match target.split_once('.') {
        Some((script, function)) => {
            let marked = run::marked_functions(&scripts);
            let mut vm = Vm::new(run::loader(&scripts));
            vm.set_debugger(Adapter(session.clone()));
            let result =
                run::start(&mut vm, &marked, script).and_then(|obj| vm.call(obj, function, &args));
            Some((vm, result))
        }
        None => None,
    };

    let mut session = session.borrow_mut();
    let code = match &result {
        Some((vm, result)) => {
            session.flush_output(vm);
            for e in &vm.errors {
                session.client.output("stderr", &format!("error: {e}"));
            }
            match run::problem(vm, result) {
                _ if matches!(result, Err(VmError::Stopped)) => 1,
                None => {
                    if let Ok(v) = result
                        && !matches!(v, Value::None)
                    {
                        session.client.output("console", &format!("=> {v}"));
                    }
                    session.client.output("console", &format!("ok    {target}"));
                    0
                }
                Some(why) => {
                    session.client.output("console", &format!("FAIL  {target}: {why}"));
                    1
                }
            }
        }
        None => {
            let message = "launch: expected `test` or `run` as \"Script.Function\"";
            session.client.output("stderr", message);
            2
        }
    };
    session.client.event("exited", json!({ "exitCode": code }));
    session.client.event("terminated", json!({}));
    // Until the client disconnects.
    while let Ok(req) = requests.recv() {
        let command = req["command"].as_str().unwrap_or_default().to_owned();
        session.request(&req);
        if matches!(command.as_str(), "disconnect" | "terminate") {
            break;
        }
    }
    code
}

/// A launch argument as a command line argument.
fn json_text(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}
