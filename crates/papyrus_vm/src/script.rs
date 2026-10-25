//! Scripts as the machine runs them: a `.pex` file with its strings resolved
//! and names keyed in lowercase (Papyrus ignores case).

use std::rc::Rc;

use papyrus_pex::{
    PexFile,
    pex::{FunctionInfo, OpCode, VariableValue},
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
    pub(crate) code: Vec<Inst>,
}

#[derive(Debug)]
pub(crate) struct Property {
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
        let func = |name: String, info: &FunctionInfo| {
            let slots = |v: &[papyrus_pex::pex::VariableType]| {
                v.iter().map(|p| (key(&s(p.name)), s(p.ty))).collect()
            };
            Rc::new(Func {
                name,
                return_type: s(info.return_type),
                native: info.is_native(),
                params: slots(&info.params),
                locals: slots(&info.locals),
                code: info
                    .instructions
                    .iter()
                    .map(|i| Inst { op: i.opcode, args: i.args.iter().map(operand).collect() })
                    .collect(),
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
            properties: object
                .properties
                .iter()
                .map(|p| {
                    let name = s(p.name);
                    let prop = Property {
                        auto_var: p.auto_var_name.map(|id| key(&s(id))),
                        getter: p.read_handler.as_ref().map(|f| func(name.clone(), f)),
                        setter: p.write_handler.as_ref().map(|f| func(name.clone(), f)),
                    };
                    (key(&name), prop)
                })
                .collect(),
            states: object
                .states
                .iter()
                .map(|st| {
                    let functions = st
                        .functions
                        .iter()
                        .map(|f| {
                            let name = s(f.name);
                            (key(&name), func(name, &f.info))
                        })
                        .collect();
                    (key(&s(st.name)), functions)
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
