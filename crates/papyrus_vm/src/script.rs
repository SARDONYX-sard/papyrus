//! Scripts as the machine runs them: a `.pex` file with its strings resolved
//! and names keyed in lowercase (Papyrus ignores case).

use std::rc::Rc;

use papyrus_pex::{
    PexFile,
    pex::{DebugFunctionType, FunctionInfo, OpCode, VariableValue},
};
use rustc_hash::FxHashMap;

/// A lowercase name, the key of every lookup.
pub(crate) fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// An operand of an instruction.
#[derive(Clone, Debug)]
pub(crate) enum Operand {
    /// A variable, parameter, property or script name, lowercase.
    Name(String),
    None,
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(Rc<str>),
}

#[derive(Clone, Debug)]
pub(crate) struct Inst {
    pub(crate) op: OpCode,
    pub(crate) args: Vec<Operand>,
}

/// A function, event or property accessor.
#[derive(Debug)]
pub(crate) struct Func {
    /// As declared, for messages.
    pub(crate) name: String,
    pub(crate) return_type: String,
    pub(crate) native: bool,
    /// Parameters, then locals: lowercase name and type.
    pub(crate) params: Vec<(String, String)>,
    pub(crate) locals: Vec<(String, String)>,
    /// The names of the parameters and locals as declared, by lowercase name.
    pub(crate) names: FxHashMap<String, String>,
    pub(crate) code: Vec<Inst>,
    /// The source line of each instruction; empty without debug info.
    pub(crate) lines: Vec<u32>,
}

#[derive(Debug)]
pub(crate) struct Property {
    /// As declared.
    pub(crate) name: String,
    pub(crate) auto_var: Option<String>,
    pub(crate) getter: Option<Rc<Func>>,
    pub(crate) setter: Option<Rc<Func>>,
}

/// A script.
#[derive(Debug)]
pub struct Script {
    /// As declared.
    pub name: String,
    /// The parent script, lowercase.
    pub(crate) parent: Option<String>,
    pub(crate) auto_state: String,
    /// Variables: lowercase name, type, initial value.
    pub(crate) variables: Vec<(String, String, Operand)>,
    /// The names of the variables as declared, by lowercase name.
    pub(crate) variable_names: FxHashMap<String, String>,
    pub(crate) properties: FxHashMap<String, Property>,
    /// Functions by lowercase state (`""`: the empty state) and name.
    pub(crate) states: FxHashMap<String, FxHashMap<String, Rc<Func>>>,
}

impl Script {
    /// Resolves the first object of a `.pex` file.
    pub fn from_pex(pex: &PexFile<'_>) -> Option<Script> {
        let object = pex.objects.first()?;
        let s = |id| pex.str(id).into_owned();
        let operand = |v: &VariableValue| match *v {
            VariableValue::Null => Operand::None,
            VariableValue::Identifier(id) => Operand::Name(key(&pex.str(id))),
            VariableValue::String(id) => Operand::Str(Rc::from(pex.str(id).as_ref())),
            VariableValue::Integer(i) => Operand::Int(i),
            VariableValue::Float(f) => Operand::Float(f),
            VariableValue::Boolean(b) => Operand::Bool(b),
        };
        // Line numbers by lowercase state, function (or property) and kind.
        let mut lines: FxHashMap<(String, String, u8), Vec<u32>> = FxHashMap::default();
        for f in pex.debug_info.iter().flat_map(|d| &d.functions) {
            let at = (key(&s(f.state_name)), key(&s(f.function_name)), f.function_type as u8);
            lines.insert(at, f.instruction_line_numbers.iter().map(|&l| u32::from(l)).collect());
        }
        let func = |name: String, info: &FunctionInfo, at: (&str, DebugFunctionType)| {
            let slots = |v: &[papyrus_pex::pex::VariableType]| {
                v.iter().map(|p| (key(&s(p.name)), s(p.ty))).collect()
            };
            let names = (info.params.iter().chain(&info.locals))
                .map(|p| (key(&s(p.name)), s(p.name)))
                .collect();
            Rc::new(Func {
                return_type: s(info.return_type),
                native: info.is_native(),
                params: slots(&info.params),
                locals: slots(&info.locals),
                names,
                code: info
                    .instructions
                    .iter()
                    .map(|i| Inst { op: i.opcode, args: i.args.iter().map(operand).collect() })
                    .collect(),
                lines: lines.get(&(key(at.0), key(&name), at.1 as u8)).cloned().unwrap_or_default(),
                name,
            })
        };
        let parent = s(object.parent_class_name);
        Some(Script {
            name: s(object.name),
            parent: (!parent.is_empty()).then(|| key(&parent)),
            auto_state: s(object.auto_state_name),
            variables: object
                .variables
                .iter()
                .map(|v| (key(&s(v.name)), s(v.type_name), operand(&v.value)))
                .collect(),
            variable_names: object.variables.iter().map(|v| (key(&s(v.name)), s(v.name))).collect(),
            properties: object
                .properties
                .iter()
                .map(|p| {
                    let name = s(p.name);
                    let accessor = |f, kind| func(name.clone(), f, ("", kind));
                    let prop = Property {
                        auto_var: p.auto_var_name.map(|id| key(&s(id))),
                        getter: p
                            .read_handler
                            .as_ref()
                            .map(|f| accessor(f, DebugFunctionType::Getter)),
                        setter: p
                            .write_handler
                            .as_ref()
                            .map(|f| accessor(f, DebugFunctionType::Setter)),
                        name: name.clone(),
                    };
                    (key(&name), prop)
                })
                .collect(),
            states: object
                .states
                .iter()
                .map(|st| {
                    let state = s(st.name);
                    let functions = st
                        .functions
                        .iter()
                        .map(|f| {
                            let name = s(f.name);
                            (key(&name), func(name, &f.info, (&state, DebugFunctionType::Method)))
                        })
                        .collect();
                    (key(&state), functions)
                })
                .collect(),
        })
    }

    /// The function `name` of `state`, else of the empty state.
    pub(crate) fn function(&self, state: &str, name: &str) -> Option<Rc<Func>> {
        let state = key(state);
        let in_state = |st: &str| self.states.get(st).and_then(|fs| fs.get(name)).cloned();
        (!state.is_empty()).then(|| in_state(&state)).flatten().or_else(|| in_state(""))
    }
}
