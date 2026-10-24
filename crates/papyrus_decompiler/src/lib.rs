//! Decompiler of compiled Papyrus scripts (`.pex`, Skyrim) to source.
//!
//! The declarations come back exactly: variables, properties (`Auto`,
//! `AutoReadOnly` and full ones), states, functions and their flags and
//! documentation. Bodies are rebuilt from the patterns compilers generate
//! (see [`body`]), which gives back the structure of the source; what the
//! compiled code does not record is lost:
//!
//! - comments, formatting and parameter default values (call sites pass
//!   every argument);
//! - whether a function was written as an `Event` (functions of no result
//!   named `On...` are printed as events);
//! - explicit conversions the compiler would insert anyway.
//!
//! The output compiles back to the same code, which is how it is tested.

mod ast;
mod body;

use std::fmt::Write as _;

use papyrus_pex::{
    PexFile,
    pex::{FunctionInfo, Object, OpCode, StringId, VariableValue},
};
use rustc_hash::FxHashMap;

use crate::ast::Expr;

/// What the bodies of a script need to know about it.
pub(crate) struct ScriptCx<'p> {
    pex: &'p PexFile<'p>,
    /// The script name.
    script: String,
    /// Lowercase `::Name_var` to the auto property `Name`.
    auto_vars: FxHashMap<String, String>,
    /// Lowercase names of variables and properties to their type.
    member_types: FxHashMap<String, String>,
}

impl ScriptCx<'_> {
    fn str(&self, id: StringId) -> String {
        self.pex.str(id).into_owned()
    }
}

/// Decompiles every script of a file (there is one in files of the game).
pub fn decompile(pex: &PexFile<'_>) -> String {
    let mut out = String::new();
    for (i, object) in pex.objects.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        Writer::new(pex, object).script(&mut out);
    }
    out
}

struct Writer<'p> {
    cx: ScriptCx<'p>,
    object: &'p Object,
}

const GET_STATE_DOC: &str = "Function that returns the current state";
const GOTO_STATE_DOC: &str = "Function that switches this object to the specified state";

impl<'p> Writer<'p> {
    fn new(pex: &'p PexFile<'p>, object: &'p Object) -> Writer<'p> {
        let s = |id: StringId| pex.str(id).into_owned();
        let mut auto_vars = FxHashMap::default();
        let mut member_types = FxHashMap::default();
        for p in &object.properties {
            if let Some(var) = p.auto_var_name {
                auto_vars.insert(s(var).to_ascii_lowercase(), s(p.name));
            }
            member_types.insert(s(p.name).to_ascii_lowercase(), s(p.ty));
        }
        for v in &object.variables {
            member_types.insert(s(v.name).to_ascii_lowercase(), s(v.type_name));
        }
        let cx = ScriptCx { pex, script: s(object.name), auto_vars, member_types };
        Writer { cx, object }
    }

    fn s(&self, id: StringId) -> String {
        self.cx.str(id)
    }

    /// The names of the user flags set in `bits`, each with a leading space.
    fn flags(&self, bits: u32) -> String {
        let mut out = String::new();
        let mut defs: Vec<_> = self.cx.pex.user_flags.iter().collect();
        defs.sort_by_key(|f| f.flag_index);
        for def in defs {
            if def.flag_index < 32 && bits & (1 << def.flag_index) != 0 {
                let name = self.s(def.name);
                // The official compiler writes them lowercase.
                let mut chars = name.chars();
                let name: String =
                    chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars).collect();
                let _ = write!(out, " {name}");
            }
        }
        out
    }

    fn docstring(&self, out: &mut String, id: StringId, depth: usize) {
        let doc = self.s(id);
        if !doc.is_empty() {
            indent(out, depth);
            let _ = writeln!(out, "{{{doc}}}");
        }
    }

    fn script(&self, out: &mut String) {
        let obj = self.object;
        let _ = write!(out, "ScriptName {}", self.cx.script);
        let parent = self.s(obj.parent_class_name);
        if !parent.is_empty() {
            let _ = write!(out, " extends {parent}");
        }
        out.push_str(&self.flags(obj.user_flags.bits()));
        out.push('\n');
        self.docstring(out, obj.docstring, 0);

        let section = |out: &mut String| out.push('\n');
        let vars: Vec<_> = obj
            .variables
            .iter()
            .filter(|v| !self.cx.auto_vars.contains_key(&self.s(v.name).to_ascii_lowercase()))
            .collect();
        if !vars.is_empty() {
            section(out);
        }
        for v in vars {
            let _ = write!(out, "{} {}", self.s(v.type_name), self.s(v.name));
            if let Some(value) = self.literal(&v.value) {
                let _ = write!(out, " = {value}");
            }
            out.push_str(&self.flags(v.user_flags));
            out.push('\n');
        }
        for p in &obj.properties {
            section(out);
            self.property(out, p);
        }

        let (empty, states): (Vec<_>, Vec<_>) =
            obj.states.iter().partition(|st| self.s(st.name).is_empty());
        for st in empty {
            for f in &st.functions {
                let name = self.s(f.name);
                if self.is_generated(&name, &f.info) {
                    continue;
                }
                section(out);
                self.function(out, &name, &f.info, 0);
            }
        }
        let auto = self.s(obj.auto_state_name);
        for st in states {
            let name = self.s(st.name);
            section(out);
            if name.eq_ignore_ascii_case(&auto) {
                out.push_str("Auto ");
            }
            let _ = writeln!(out, "State {name}");
            for (i, f) in st.functions.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                self.function(out, &self.s(f.name), &f.info, 1);
            }
            out.push_str("EndState\n");
        }
    }

    /// `GetState` and `GotoState` as the compiler generates them.
    fn is_generated(&self, name: &str, info: &FunctionInfo) -> bool {
        let doc = self.s(info.docstring);
        (name.eq_ignore_ascii_case("GetState")
            && doc == GET_STATE_DOC
            && info.instructions.len() == 1)
            || (name.eq_ignore_ascii_case("GotoState")
                && doc == GOTO_STATE_DOC
                && info.instructions.len() == 3)
    }

    /// A constant as source, `None` for `None`.
    fn literal(&self, v: &VariableValue) -> Option<String> {
        let e = match v {
            VariableValue::Null => return None,
            VariableValue::Identifier(id) | VariableValue::String(id) => Expr::Str(self.s(*id)),
            VariableValue::Integer(i) => Expr::Int(*i),
            VariableValue::Float(f) => Expr::Float(*f),
            VariableValue::Boolean(b) => Expr::Bool(*b),
        };
        let mut out = String::new();
        e.write(&mut out);
        Some(out)
    }

    fn property(&self, out: &mut String, p: &papyrus_pex::pex::Property) {
        let ty = self.s(p.ty);
        let name = self.s(p.name);
        let flags = self.flags(p.user_flags);
        if let Some(var) = p.auto_var_name {
            let var = self.object.variables.iter().find(|v| v.name == var);
            let _ = write!(out, "{ty} Property {name}");
            if let Some(value) = var.and_then(|v| self.literal(&v.value)) {
                let _ = write!(out, " = {value}");
            }
            let _ = writeln!(
                out,
                " Auto{flags}{}",
                var.map_or(String::new(), |v| self.flags(v.user_flags))
            );
            self.docstring(out, p.docstring, 0);
            return;
        }
        // A getter returning a constant, without setter.
        if let (Some(get), None) = (&p.read_handler, &p.write_handler)
            && let [inst] = get.instructions.as_slice()
            && inst.opcode == OpCode::Ret
            && !matches!(inst.args[0], VariableValue::Identifier(_))
        {
            let _ = write!(out, "{ty} Property {name}");
            if let Some(value) = self.literal(&inst.args[0]) {
                let _ = write!(out, " = {value}");
            }
            let _ = writeln!(out, " AutoReadOnly{flags}");
            self.docstring(out, p.docstring, 0);
            return;
        }
        let _ = writeln!(out, "{ty} Property {name}{flags}");
        self.docstring(out, p.docstring, 0);
        if let Some(get) = &p.read_handler {
            self.function(out, "Get", get, 1);
        }
        if let Some(set) = &p.write_handler {
            self.function(out, "Set", set, 1);
        }
        out.push_str("EndProperty\n");
    }

    fn function(&self, out: &mut String, name: &str, info: &FunctionInfo, depth: usize) {
        let ret = self.s(info.return_type);
        let event = ret.eq_ignore_ascii_case("None")
            && name.len() > 2
            && name[..2].eq_ignore_ascii_case("on");
        indent(out, depth);
        if event {
            out.push_str("Event ");
        } else {
            if !ret.eq_ignore_ascii_case("None") {
                let _ = write!(out, "{ret} ");
            }
            out.push_str("Function ");
        }
        let _ = write!(out, "{name}(");
        for (i, p) in info.params.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let _ = write!(out, "{} {}", self.s(p.ty), self.s(p.name));
        }
        out.push(')');
        if info.is_global() {
            out.push_str(" Global");
        }
        if info.is_native() {
            out.push_str(" Native");
        }
        out.push_str(&self.flags(info.user_flags));
        out.push('\n');
        self.docstring(out, info.docstring, depth);
        if info.is_native() {
            return;
        }
        let stmts = body::decompile(&self.cx, info);
        ast::write_block(out, &stmts, depth + 1);
        indent(out, depth);
        out.push_str(if event { "EndEvent\n" } else { "EndFunction\n" });
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}

#[cfg(test)]
mod tests;
