//! Disassembly of `.pex` files into text.
//!
//! The format follows the `.pas` assembly of the official PapyrusAssembler:
//! nested `.object`, `.state`, `.function` and `.code` blocks. Jump offsets
//! are shown as `labelN` and the targets as `labelN:` lines; the source line
//! of each instruction, when known, follows as `;@line N`.

use std::fmt::Write as _;

use crate::pex::*;

/// Options of [`disassemble`].
#[derive(Clone, Copy, Debug, Default)]
pub struct DisasmOptions {
    /// Leave out the header (file names, times, user and machine), which
    /// differs between builds of the same source.
    pub skip_header: bool,
    /// Leave out the `;@line` comments.
    pub skip_lines: bool,
}

/// Disassembles a file.
pub fn disassemble(pex: &PexFile<'_>, options: DisasmOptions) -> String {
    let mut d = Disasm { pex, options, out: String::new(), indent: 0 };
    d.file();
    d.out
}

struct Disasm<'p, 'a> {
    pex: &'p PexFile<'a>,
    options: DisasmOptions,
    out: String,
    indent: usize,
}

impl Disasm<'_, '_> {
    fn line(&mut self, text: impl AsRef<str>) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text.as_ref());
        self.out.push('\n');
    }

    fn open(&mut self, text: impl AsRef<str>) {
        self.line(text);
        self.indent += 1;
    }

    fn close(&mut self, text: impl AsRef<str>) {
        self.indent -= 1;
        self.line(text);
    }

    fn s(&self, id: StringId) -> String {
        self.pex.str(id).into_owned()
    }

    /// A string as a quoted literal.
    fn quoted(&self, id: StringId) -> String {
        quote(&self.pex.str(id))
    }

    fn value(&self, value: &VariableValue) -> String {
        match *value {
            VariableValue::Null => "none".to_owned(),
            VariableValue::Identifier(id) => self.s(id),
            VariableValue::String(id) => self.quoted(id),
            VariableValue::Integer(i) => i.to_string(),
            VariableValue::Float(f) => format!("{f:?}"),
            VariableValue::Boolean(b) => b.to_string(),
        }
    }

    fn file(&mut self) {
        let pex = self.pex;
        if !self.options.skip_header {
            self.open(".info");
            self.line(format!(".source {}", quote(&pex.src_file_name.decode())));
            if let Some(debug) = &pex.debug_info {
                self.line(format!(".modifyTime {}", debug.modification_time));
            }
            self.line(format!(".compileTime {}", pex.compilation_time));
            self.line(format!(".user {}", quote(&pex.user_name.decode())));
            self.line(format!(".computer {}", quote(&pex.machine_name.decode())));
            self.line(format!(".version {}.{}", pex.major_version, pex.minor_version));
            self.close(".endInfo");
        }
        self.open(".userFlagsRef");
        let mut flags: Vec<_> = pex.user_flags.iter().collect();
        flags.sort_by_key(|f| f.flag_index);
        for flag in flags {
            self.line(format!(".flag {} {}", self.s(flag.name), flag.flag_index));
        }
        self.close(".endUserFlagsRef");
        self.open(".objectTable");
        for object in &pex.objects {
            self.object(object);
        }
        self.close(".endObjectTable");
    }

    fn object(&mut self, object: &Object) {
        self.open(format!(".object {} {}", self.s(object.name), self.s(object.parent_class_name)));
        self.line(format!(".userFlags {}", object.user_flags.bits()));
        self.line(format!(".docString {}", self.quoted(object.docstring)));
        self.line(format!(".autoState {}", self.s(object.auto_state_name)));

        self.open(".variableTable");
        for v in &object.variables {
            self.open(format!(".variable {} {}", self.s(v.name), self.s(v.type_name)));
            self.line(format!(".userFlags {}", v.user_flags));
            self.line(format!(".initialValue {}", self.value(&v.value)));
            self.close(".endVariable");
        }
        self.close(".endVariableTable");

        self.open(".propertyTable");
        for p in &object.properties {
            let auto = if p.is_auto_var() { " auto" } else { "" };
            self.open(format!(".property {} {}{auto}", self.s(p.name), self.s(p.ty)));
            self.line(format!(".userFlags {}", p.user_flags));
            self.line(format!(".docString {}", self.quoted(p.docstring)));
            if let Some(var) = p.auto_var_name {
                self.line(format!(".autoVar {}", self.s(var)));
            }
            if let Some(get) = &p.read_handler {
                self.function(
                    "get",
                    get,
                    self.debug_lines(object, None, p.name, DebugFunctionType::Getter),
                );
            }
            if let Some(set) = &p.write_handler {
                self.function(
                    "set",
                    set,
                    self.debug_lines(object, None, p.name, DebugFunctionType::Setter),
                );
            }
            self.close(".endProperty");
        }
        self.close(".endPropertyTable");

        self.open(".stateTable");
        for state in &object.states {
            self.open(format!(".state {}", self.s(state.name)).trim_end());
            for f in &state.functions {
                let lines =
                    self.debug_lines(object, Some(state.name), f.name, DebugFunctionType::Method);
                self.function(&self.s(f.name), &f.info, lines);
            }
            self.close(".endState");
        }
        self.close(".endStateTable");
        self.close(".endObject");
    }

    fn debug_lines(
        &self,
        object: &Object,
        state: Option<StringId>,
        name: StringId,
        ty: DebugFunctionType,
    ) -> Option<Vec<u16>> {
        let debug = self.pex.debug_info.as_ref()?;
        debug
            .functions
            .iter()
            .find(|f| {
                f.object_name == object.name
                    && f.function_name == name
                    && f.function_type == ty
                    && state.is_none_or(|s| f.state_name == s)
            })
            .map(|f| f.instruction_line_numbers.clone())
    }

    fn function(&mut self, name: &str, info: &FunctionInfo, lines: Option<Vec<u16>>) {
        let mut flags = String::new();
        if info.is_global() {
            flags.push_str(" static");
        }
        if info.is_native() {
            flags.push_str(" native");
        }
        self.open(format!(".function {name}{flags}"));
        self.line(format!(".userFlags {}", info.user_flags));
        self.line(format!(".docString {}", self.quoted(info.docstring)));
        self.line(format!(".return {}", self.s(info.return_type)));
        self.open(".paramTable");
        for p in &info.params {
            self.line(format!(".param {} {}", self.s(p.name), self.s(p.ty)));
        }
        self.close(".endParamTable");
        self.open(".localTable");
        for l in &info.locals {
            self.line(format!(".local {} {}", self.s(l.name), self.s(l.ty)));
        }
        self.close(".endLocalTable");
        self.open(".code");
        self.code(&info.instructions, lines.as_deref());
        self.close(".endCode");
        self.close(".endFunction");
    }

    fn code(&mut self, code: &[Instruction], lines: Option<&[u16]>) {
        // Number the jump targets in order of appearance.
        let mut targets: Vec<usize> =
            code.iter().enumerate().filter_map(|(i, ins)| jump_target(i, ins)).collect();
        targets.sort_unstable();
        targets.dedup();
        let label = |target: usize| format!("label{}", targets.binary_search(&target).unwrap_or(0));

        for (i, ins) in code.iter().enumerate() {
            if targets.binary_search(&i).is_ok() {
                self.indent -= 1;
                self.line(format!("{}:", label(i)));
                self.indent += 1;
            }
            let mut text = ins.opcode.mnemonic().to_owned();
            let jump = jump_target(i, ins);
            for (n, arg) in ins.args.iter().enumerate() {
                text.push(' ');
                let is_offset =
                    matches!((ins.opcode, n), (OpCode::Jmp, 0) | (OpCode::JmpT | OpCode::JmpF, 1));
                match (is_offset, jump) {
                    (true, Some(t)) => text.push_str(&label(t)),
                    _ => text.push_str(&self.value(arg)),
                }
            }
            if !self.options.skip_lines
                && let Some(line) = lines.and_then(|l| l.get(i))
            {
                let _ = write!(text, " ;@line {line}");
            }
            self.line(text);
        }
        // A jump past the last instruction.
        if targets.last().is_some_and(|&t| t >= code.len()) {
            self.indent -= 1;
            self.line(format!("{}:", label(code.len())));
            self.indent += 1;
        }
    }
}

/// The instruction index a jump goes to: offsets are relative to the jump.
pub fn jump_target(index: usize, ins: &Instruction) -> Option<usize> {
    let offset = match (ins.opcode, ins.args.as_slice()) {
        (OpCode::Jmp, [VariableValue::Integer(o)]) => *o,
        (OpCode::JmpT | OpCode::JmpF, [_, VariableValue::Integer(o)]) => *o,
        _ => return None,
    };
    usize::try_from(index as i64 + offset as i64).ok()
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
