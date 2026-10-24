//! The intermediate representation of function bodies.
//!
//! Code generation produces one [`FunctionIr`] per function: instructions
//! with symbolic labels and operands by name. Optimization passes
//! ([`crate::opt`]) rewrite it, and [`FunctionIr::lower`] resolves labels to
//! the relative jump offsets of the `.pex` format.

use papyrus_pex::pex::OpCode;

/// A value used by an instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    /// A variable, parameter, property or script name, `self`, `::NoneVar`, ...
    Var(String),
    /// `None`
    None,
    /// An integer constant.
    Int(i32),
    /// A float constant.
    Float(f32),
    /// A boolean constant.
    Bool(bool),
    /// A string constant.
    Str(String),
}

impl Operand {
    /// A variable operand.
    pub fn var(name: impl Into<String>) -> Operand {
        Operand::Var(name.into())
    }

    /// Whether the operand is a constant.
    pub fn is_const(&self) -> bool {
        !matches!(self, Operand::Var(_))
    }

    /// The variable name, for variable operands.
    pub fn as_var(&self) -> Option<&str> {
        match self {
            Operand::Var(v) => Some(v),
            _ => None,
        }
    }
}

/// A jump target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub u32);

/// An instruction of the IR.
#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
    /// An instruction without jump: the opcode and all its arguments (for
    /// calls, the argument count is added when lowering).
    Op {
        /// The opcode.
        opcode: OpCode,
        /// The arguments.
        args: Vec<Operand>,
    },
    /// `jmp`, or `jmpt` / `jmpf` on `cond`.
    Jump {
        /// `Jmp`, `JmpT` or `JmpF`.
        opcode: OpCode,
        /// The condition of conditional jumps.
        cond: Option<Operand>,
        /// The target.
        target: Label,
    },
    /// The position of a label; no code.
    Label(Label),
}

/// A typed slot of a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    /// The name.
    pub name: String,
    /// The type name.
    pub ty: String,
}

/// The code of one function, event or property accessor.
#[derive(Clone, Debug, Default)]
pub struct FunctionIr {
    /// Parameters.
    pub params: Vec<Slot>,
    /// Locals and temporaries, in creation order.
    pub locals: Vec<Slot>,
    /// Instructions with their source line (0 when unknown).
    pub code: Vec<(Inst, u32)>,
}

/// A lowered instruction: the opcode and the arguments with jump offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredInst {
    /// The opcode.
    pub opcode: OpCode,
    /// The arguments; jump offsets are `Operand::Int`.
    pub args: Vec<Operand>,
    /// The source line.
    pub line: u32,
}

impl std::fmt::Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::Var(v) => f.write_str(v),
            Operand::None => f.write_str("none"),
            Operand::Int(i) => write!(f, "{i}"),
            Operand::Float(x) => write!(f, "{x:?}"),
            Operand::Bool(b) => write!(f, "{b}"),
            Operand::Str(s) => write!(f, "{s:?}"),
        }
    }
}

/// The code as assembly, labels as `@N`, for logs.
impl std::fmt::Display for FunctionIr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (inst, line) in &self.code {
            match inst {
                Inst::Label(l) => writeln!(f, "  @{}:", l.0)?,
                Inst::Op { opcode, args } => {
                    write!(f, "    {}", opcode.mnemonic())?;
                    for a in args {
                        write!(f, " {a}")?;
                    }
                    writeln!(f, " ;@line {line}")?;
                }
                Inst::Jump { opcode, cond, target } => {
                    write!(f, "    {}", opcode.mnemonic())?;
                    if let Some(c) = cond {
                        write!(f, " {c}")?;
                    }
                    writeln!(f, " @{} ;@line {line}", target.0)?;
                }
            }
        }
        Ok(())
    }
}

impl FunctionIr {
    /// The number of instructions, labels excluded.
    pub fn instruction_count(&self) -> usize {
        self.code.iter().filter(|(i, _)| !matches!(i, Inst::Label(_))).count()
    }

    /// Resolves labels: jump offsets are relative to the jump instruction.
    pub fn lower(&self) -> Vec<LoweredInst> {
        let mut positions = rustc_hash::FxHashMap::default();
        let mut index = 0usize;
        for (inst, _) in &self.code {
            match inst {
                Inst::Label(l) => {
                    positions.insert(*l, index);
                }
                _ => index += 1,
            }
        }
        let mut out = Vec::with_capacity(index);
        for (inst, line) in &self.code {
            match inst {
                Inst::Label(_) => {}
                Inst::Op { opcode, args } => {
                    out.push(LoweredInst { opcode: *opcode, args: args.clone(), line: *line })
                }
                Inst::Jump { opcode, cond, target } => {
                    let here = out.len() as i64;
                    let to = positions.get(target).copied().unwrap_or(index) as i64;
                    let mut args: Vec<Operand> = cond.iter().cloned().collect();
                    args.push(Operand::Int((to - here) as i32));
                    out.push(LoweredInst { opcode: *opcode, args, line: *line });
                }
            }
        }
        out
    }
}
