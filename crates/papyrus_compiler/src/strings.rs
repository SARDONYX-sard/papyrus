//! The string table of a `.pex` file.

use rustc_hash::FxHashMap;

use papyrus_pex::{
    PexStr,
    pex::{StringId, VariableValue},
};

use crate::ir::Operand;

/// Interns the strings of a file.
///
/// Like the official compiler, entries are shared ignoring ASCII case: the
/// first spelling wins. The game looks identifiers up ignoring case anyway,
/// but string *literals* are affected too: `"busy"` reads `"Busy"` when
/// `"Busy"` was interned first. This is kept for compatibility.
#[derive(Default)]
pub(crate) struct StringTable {
    strings: Vec<String>,
    index: FxHashMap<String, StringId>,
}

impl StringTable {
    pub(crate) fn id(&mut self, s: &str) -> StringId {
        let key = s.to_ascii_lowercase();
        if let Some(&id) = self.index.get(&key) {
            return id;
        }
        let id = StringId(self.strings.len() as u16);
        self.strings.push(s.to_owned());
        self.index.insert(key, id);
        id
    }

    pub(crate) fn value(&mut self, operand: &Operand) -> VariableValue {
        match operand {
            Operand::Var(v) => VariableValue::Identifier(self.id(v)),
            Operand::None => VariableValue::Null,
            Operand::Int(i) => VariableValue::Integer(*i),
            Operand::Float(f) => VariableValue::Float(*f),
            Operand::Bool(b) => VariableValue::Boolean(*b),
            Operand::Str(s) => VariableValue::String(self.id(s)),
        }
    }

    pub(crate) fn get(&self, id: StringId) -> Option<&str> {
        self.strings.get(id.index()).map(String::as_str)
    }

    pub(crate) fn into_table(self) -> Vec<PexStr<'static>> {
        self.strings.iter().map(|s| PexStr::encode(s)).collect()
    }
}
