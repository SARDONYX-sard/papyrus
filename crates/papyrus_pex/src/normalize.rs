//! A canonical text of a `.pex` file, to compare the output of two compilers.
//!
//! Two compilations of the same source differ in ways that do not matter to
//! the game: the order of tables (the official compiler uses hash order), the
//! spelling case of names (string table entries are shared ignoring case),
//! the numbering of temporaries, and the header. [`normalize`] removes these
//! differences: tables are sorted, everything is lowercase, temporaries are
//! renumbered by first use in each function and left out of the local table,
//! and the header and line numbers are dropped.

use std::fmt::Write as _;

use crate::{disasm::jump_target, pex::*};

/// Renders the canonical text of `pex`.
pub fn normalize(pex: &PexFile<'_>) -> String {
    normalize_with(pex, false)
}

/// Like [`normalize`]; with `merge_temps`, all temporaries have the same
/// name, which hides differences in how temporaries are reused.
pub fn normalize_with(pex: &PexFile<'_>, merge_temps: bool) -> String {
    let s = |id: StringId| pex.str(id).to_lowercase();
    let mut out = String::new();
    for obj in &pex.objects {
        let _ = writeln!(
            out,
            "object {} extends {} flags {} auto {} doc {:?}",
            s(obj.name),
            s(obj.parent_class_name),
            obj.user_flags.bits(),
            s(obj.auto_state_name),
            s(obj.docstring)
        );
        let mut vars: Vec<String> = obj
            .variables
            .iter()
            .map(|v| {
                format!(
                    "  var {} {} flags {} = {}",
                    s(v.name),
                    s(v.type_name),
                    v.user_flags,
                    value(pex, &v.value, &mut Vec::new())
                )
            })
            .collect();
        vars.sort();
        lines(&mut out, vars);
        let mut props: Vec<String> = obj
            .properties
            .iter()
            .map(|p| {
                let mut text = format!(
                    "  property {} {} flags {} uflags {} doc {:?}",
                    s(p.name),
                    s(p.ty),
                    p.flags.bits(),
                    p.user_flags,
                    s(p.docstring)
                );
                if let Some(var) = p.auto_var_name {
                    let _ = write!(text, " auto {}", s(var));
                }
                if let Some(f) = &p.read_handler {
                    text.push('\n');
                    function(&mut text, pex, "get", f, merge_temps);
                }
                if let Some(f) = &p.write_handler {
                    text.push('\n');
                    function(&mut text, pex, "set", f, merge_temps);
                }
                text
            })
            .collect();
        props.sort();
        lines(&mut out, props);
        let mut states: Vec<String> = obj
            .states
            .iter()
            .map(|st| {
                let mut text = format!("  state {:?}", s(st.name));
                let mut functions: Vec<String> = st
                    .functions
                    .iter()
                    .map(|f| {
                        let mut t = String::new();
                        function(&mut t, pex, &s(f.name), &f.info, merge_temps);
                        t
                    })
                    .collect();
                functions.sort();
                for f in functions {
                    text.push('\n');
                    text.push_str(&f);
                }
                text
            })
            .collect();
        states.sort();
        lines(&mut out, states);
    }
    out
}

fn lines(out: &mut String, items: Vec<String>) {
    for item in items {
        out.push_str(&item);
        out.push('\n');
    }
}

fn function(out: &mut String, pex: &PexFile<'_>, name: &str, f: &FunctionInfo, merge_temps: bool) {
    let s = |id: StringId| pex.str(id).to_lowercase();
    let _ = write!(
        out,
        "    function {name} -> {} flags {} uflags {} doc {:?}",
        s(f.return_type),
        f.flags.bits(),
        f.user_flags,
        s(f.docstring)
    );
    for p in &f.params {
        let _ = write!(out, "\n      param {} {}", s(p.name), s(p.ty));
    }
    let mut locals: Vec<String> = f
        .locals
        .iter()
        .filter(|l| !is_temp(&s(l.name)))
        .map(|l| format!("\n      local {} {}", s(l.name), s(l.ty)))
        .collect();
    locals.sort();
    out.push_str(&locals.concat());
    let mut temps = Vec::new();
    let targets: Vec<usize> =
        f.instructions.iter().enumerate().filter_map(|(i, inst)| jump_target(i, inst)).collect();
    for (i, inst) in f.instructions.iter().enumerate() {
        if targets.contains(&i) {
            let _ = write!(out, "\n      @{i}:");
        }
        let _ = write!(out, "\n        {}", inst.opcode.mnemonic());
        let jump = matches!(inst.opcode, OpCode::Jmp | OpCode::JmpT | OpCode::JmpF);
        for (k, arg) in inst.args.iter().enumerate() {
            if jump && k + 1 == inst.args.len() {
                let _ = write!(out, " @{}", jump_target(i, inst).unwrap_or(usize::MAX));
            } else {
                let v = value(pex, arg, &mut temps);
                let v = if merge_temps && v.starts_with("::t") && !v.starts_with("::temp") {
                    "::t".to_owned()
                } else {
                    v
                };
                let _ = write!(out, " {v}");
            }
        }
    }
}

fn is_temp(name: &str) -> bool {
    name.strip_prefix("::temp").is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
}

fn value(pex: &PexFile<'_>, v: &VariableValue, temps: &mut Vec<String>) -> String {
    match v {
        VariableValue::Null => "none".to_owned(),
        VariableValue::Identifier(id) => {
            let name = pex.str(*id).to_lowercase();
            if is_temp(&name) {
                let k = temps.iter().position(|t| *t == name).unwrap_or_else(|| {
                    temps.push(name);
                    temps.len() - 1
                });
                format!("::t{k}")
            } else {
                name
            }
        }
        VariableValue::String(id) => format!("{:?}", pex.str(*id).to_lowercase()),
        VariableValue::Integer(i) => i.to_string(),
        VariableValue::Float(f) => format!("{f:?}"),
        VariableValue::Boolean(b) => b.to_string(),
    }
}
