//! Values of the virtual machine.

use std::{cell::RefCell, fmt, rc::Rc};

/// An object: an instance of a script, by index into [`crate::Vm`]'s objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjId(pub usize);

/// A value.
#[derive(Clone, Debug, Default)]
pub enum Value {
    /// `None`, also the value of objects and arrays not set.
    #[default]
    None,
    /// `Int`
    Int(i32),
    /// `Float`
    Float(f32),
    /// `Bool`
    Bool(bool),
    /// `String`
    Str(Rc<str>),
    /// An object.
    Object(ObjId),
    /// An array, shared like in the game: arrays are references.
    Array(Rc<RefCell<Vec<Value>>>),
}

impl Value {
    /// The default value of a type, by name: `0` for `Int`, `None` for
    /// objects and arrays.
    pub fn default_of(ty: &str) -> Value {
        if ty.ends_with("[]") {
            return Value::None;
        }
        match ty.to_ascii_lowercase().as_str() {
            "int" => Value::Int(0),
            "float" => Value::Float(0.0),
            "bool" => Value::Bool(false),
            "string" => Value::Str(Rc::from("")),
            _ => Value::None,
        }
    }

    /// The value as an `Int`, like a cast.
    pub fn as_int(&self) -> i32 {
        match self {
            Value::Int(i) => *i,
            Value::Float(f) => *f as i32,
            Value::Bool(b) => *b as i32,
            Value::Str(s) => parse_number(s) as i32,
            _ => 0,
        }
    }

    /// The value as a `Float`, like a cast.
    pub fn as_float(&self) -> f32 {
        match self {
            Value::Int(i) => *i as f32,
            Value::Float(f) => *f,
            Value::Bool(b) => *b as i32 as f32,
            Value::Str(s) => parse_number(s),
            _ => 0.0,
        }
    }

    /// The value as a `Bool`, like a cast or a condition.
    pub fn truthy(&self) -> bool {
        match self {
            Value::None => false,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Bool(b) => *b,
            Value::Str(s) => !s.is_empty(),
            Value::Object(_) => true,
            Value::Array(a) => !a.borrow().is_empty(),
        }
    }

    /// Whether the value is a number, for arithmetic and comparisons.
    fn is_float(&self) -> bool {
        matches!(self, Value::Float(_))
    }

    /// Equality like `==`: numbers by value, strings ignoring case.
    pub fn loose_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::None, Value::None) => true,
            (Value::Str(a), Value::Str(b)) => a.eq_ignore_ascii_case(b),
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (a, b) if a.is_float() || b.is_float() => a.as_float() == b.as_float(),
            (Value::Int(_), _) | (_, Value::Int(_)) => self.as_int() == other.as_int(),
            _ => false,
        }
    }

    /// Ordering like `<`: numbers by value, strings ignoring case.
    pub fn compare(&self, other: &Value) -> std::cmp::Ordering {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()),
            (a, b) if a.is_float() || b.is_float() => {
                a.as_float().partial_cmp(&b.as_float()).unwrap_or(std::cmp::Ordering::Equal)
            }
            _ => self.as_int().cmp(&other.as_int()),
        }
    }
}

fn parse_number(s: &str) -> f32 {
    s.trim().parse().unwrap_or(0.0)
}

/// A value as `String` casts it: floats with 6 decimals like the game.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => f.write_str("None"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x:.6}"),
            Value::Bool(b) => f.write_str(if *b { "True" } else { "False" }),
            Value::Str(s) => f.write_str(s),
            Value::Object(id) => write!(f, "[object {}]", id.0),
            Value::Array(a) => write!(f, "[array of {}]", a.borrow().len()),
        }
    }
}
