// SPDX-FileCopyrightText: (C) 2025 russo-2025
// SPDX-License-Identifier: MIT
pub const EMPTY_STATE_NAME: &str = "";

#[inline]
pub fn property_auto_var_name(name: &str) -> String {
    format!("::{name}_var")
}

pub const LE_MAGIC_NUMBER: u32 = 0xFA57C0DE;

/// Preserve string table index
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(pub u16);

impl StringId {
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl core::fmt::Display for StringId {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameType {
    Unknown = 0,
    Skyrim = 1,
}

impl GameType {
    #[inline]
    pub const fn to_u16(self) -> u16 {
        match self {
            GameType::Unknown => 0,
            GameType::Skyrim => 1,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Nop = 0,

    IAdd,
    FAdd,

    ISub,
    FSub,

    IMul,
    FMul,

    IDiv,
    FDiv,

    IMod,

    Not,

    INeg,
    FNeg,

    Assign,
    Cast,

    CmpEq,
    CmpLt,
    CmpLe,
    CmpGt,
    CmpGe,

    Jmp,
    JmpT,
    JmpF,

    CallMethod,
    CallParent,
    CallStatic,

    Ret,

    StrCat,

    PropGet,
    PropSet,

    ArrayCreate,
    ArrayLength,

    ArrayGetElement,
    ArraySetElement,

    ArrayFindElement,
    ArrayRFindElement,
}

impl OpCode {
    #[inline]
    pub const fn argument_count(self) -> usize {
        match self {
            Self::Nop => 0,

            Self::Jmp | Self::Ret => 1,

            Self::Not
            | Self::INeg
            | Self::FNeg
            | Self::Assign
            | Self::Cast
            | Self::JmpT
            | Self::JmpF
            | Self::ArrayCreate
            | Self::ArrayLength => 2,

            Self::IAdd
            | Self::FAdd
            | Self::ISub
            | Self::FSub
            | Self::IMul
            | Self::FMul
            | Self::IDiv
            | Self::FDiv
            | Self::IMod
            | Self::CmpEq
            | Self::CmpLt
            | Self::CmpLe
            | Self::CmpGt
            | Self::CmpGe
            | Self::StrCat
            | Self::PropGet
            | Self::PropSet
            | Self::ArrayGetElement
            | Self::ArraySetElement => 3,

            Self::ArrayFindElement | Self::ArrayRFindElement => 4,

            Self::CallParent => 2,

            Self::CallStatic | Self::CallMethod => 3,
        }
    }
}

impl TryFrom<u8> for OpCode {
    type Error = OpcodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Nop,
            1 => Self::IAdd,
            2 => Self::FAdd,
            3 => Self::ISub,
            4 => Self::FSub,
            5 => Self::IMul,
            6 => Self::FMul,
            7 => Self::IDiv,
            8 => Self::FDiv,
            9 => Self::IMod,
            10 => Self::Not,
            11 => Self::INeg,
            12 => Self::FNeg,
            13 => Self::Assign,
            14 => Self::Cast,
            15 => Self::CmpEq,
            16 => Self::CmpLt,
            17 => Self::CmpLe,
            18 => Self::CmpGt,
            19 => Self::CmpGe,
            20 => Self::Jmp,
            21 => Self::JmpT,
            22 => Self::JmpF,
            23 => Self::CallMethod,
            24 => Self::CallParent,
            25 => Self::CallStatic,
            26 => Self::Ret,
            27 => Self::StrCat,
            28 => Self::PropGet,
            29 => Self::PropSet,
            30 => Self::ArrayCreate,
            31 => Self::ArrayLength,
            32 => Self::ArrayGetElement,
            33 => Self::ArraySetElement,
            34 => Self::ArrayFindElement,
            35 => Self::ArrayRFindElement,
            _ => return Err(OpcodeError { value }),
        })
    }
}

#[derive(Debug)]
pub struct OpcodeError {
    value: u8,
}

impl core::fmt::Display for OpcodeError {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "OpCodes range from 0 to 35, but got {}.", self.value)
    }
}

impl core::error::Error for OpcodeError {}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugFunctionType {
    Method = 0,
    Getter = 1,
    Setter = 2,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ObjectFlags: u32 {
        const HIDDEN      = 0b0001;
        const CONDITIONAL = 0b0010;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PropertyFlags: u8 {
        const READ    = 0b0001;
        const WRITE   = 0b0010;
        const AUTO_VAR = 0b0100;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FunctionFlags: u8 {
        const GLOBAL = 0b0001;
        const NATIVE = 0b0010;
    }
}

#[derive(Debug, Clone)]
pub enum VariableValue {
    Null,
    Identifier(StringId),
    String(StringId),
    Integer(i32),
    Float(f32),
    Boolean(bool),
}

#[derive(Debug, Clone)]
pub struct UserFlag {
    pub name: StringId,
    pub flag_index: u8,
}

#[derive(Debug, Clone)]
pub struct VariableType {
    pub name: StringId,
    pub ty: StringId,
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: OpCode,
    pub args: Vec<VariableValue>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: StringId,
    pub type_name: StringId,
    pub user_flags: u32,
    pub value: VariableValue,
}

#[derive(Debug, Clone)]
pub struct DebugFunction {
    pub object_name: StringId,
    pub state_name: StringId,
    pub function_name: StringId,
    pub function_type: DebugFunctionType,
    pub instruction_line_numbers: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub return_type: StringId,
    pub docstring: StringId,

    pub user_flags: u32,
    pub flags: FunctionFlags,

    pub params: Vec<VariableType>,
    pub locals: Vec<VariableType>,
    pub instructions: Vec<Instruction>,
}

impl FunctionInfo {
    #[inline]
    pub fn is_global(self) -> bool {
        self.flags.contains(FunctionFlags::GLOBAL)
    }

    #[inline]
    pub fn is_native(self) -> bool {
        self.flags.contains(FunctionFlags::NATIVE)
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: StringId,
    pub info: FunctionInfo,
}

#[derive(Debug, Clone)]
pub struct State {
    pub name: StringId,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub name: StringId,
    pub ty: StringId,

    pub docstring: StringId,

    pub user_flags: u32,
    pub flags: PropertyFlags,

    pub auto_var_name: Option<StringId>,

    pub read_handler: Option<FunctionInfo>,
    pub write_handler: Option<FunctionInfo>,
}

impl Property {
    #[inline]
    pub fn is_read(&self) -> bool {
        self.flags.contains(PropertyFlags::READ)
    }

    #[inline]
    pub fn is_write(&self) -> bool {
        self.flags.contains(PropertyFlags::WRITE)
    }

    #[inline]
    pub fn is_auto_var(&self) -> bool {
        self.flags.contains(PropertyFlags::AUTO_VAR)
    }

    #[inline]
    pub fn is_hidden(&self) -> bool {
        (self.user_flags & 0b0001) != 0
    }
}

#[derive(Debug, Clone)]
pub struct Object {
    pub name: StringId,
    pub size: u32,

    pub parent_class_name: StringId,
    pub docstring: StringId,

    pub user_flags: ObjectFlags,
    pub auto_state_name: StringId,

    pub variables: Vec<Variable>,
    pub properties: Vec<Property>,
    pub states: Vec<State>,
}

impl Object {
    #[inline]
    pub fn is_hidden(self) -> bool {
        self.user_flags.contains(ObjectFlags::HIDDEN)
    }

    #[inline]
    pub fn is_conditional(self) -> bool {
        self.user_flags.contains(ObjectFlags::CONDITIONAL)
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub modification_time: i64,
    pub functions: Vec<DebugFunction>,
}

#[derive(Debug, Clone)]
pub struct PexFile<'a> {
    //
    // Header
    //
    pub magic_number: u32,

    pub major_version: u8,
    pub minor_version: u8,

    pub game_id: GameType,

    pub compilation_time: i64,

    pub src_file_name: &'a str,
    pub user_name: &'a str,
    pub machine_name: &'a str,

    //
    // String Table
    //
    pub string_table: Vec<&'a str>,

    //
    // Debug Info
    //
    pub debug_info: Option<DebugInfo>,

    //
    // Objects
    //
    pub user_flags: Vec<UserFlag>,
    pub objects: Vec<Object>,
}

impl<'a> PexFile<'a> {
    #[inline]
    pub fn string(&self, id: StringId) -> Option<&str> {
        self.string_table.get(id.index()).map(|v| &**v)
    }

    pub fn object(&'a self, name: &str) -> Option<&'a Object> {
        self.objects.iter().find(|obj| self.string(obj.name) == Some(name))
    }

    pub fn state(&'a self, object: &'a Object, name: &str) -> Option<&'a State> {
        object.states.iter().find(|state| self.string(state.name) == Some(name))
    }

    pub fn empty_state(&'a self, object: &'a Object) -> Option<&'a State> {
        self.state(object, EMPTY_STATE_NAME)
    }

    pub fn default_state(&self, object: &'a Object) -> Option<&'a State> {
        let name = self.string(object.auto_state_name);

        object.states.iter().find(|state| self.string(state.name) == name)
    }

    pub fn function(&self, state: &'a State, name: &str) -> Option<&'a Function> {
        state.functions.iter().find(|func| self.string(func.name) == Some(name))
    }

    pub fn function_from_empty_state(
        &'a self,
        object_name: &str,
        function_name: &str,
    ) -> Option<&'a Function> {
        let object = self.object(object_name)?;
        let state = self.empty_state(object)?;

        self.function(state, function_name)
    }

    pub fn property(&'a self, object_name: &str, property_name: &str) -> Option<&'a Property> {
        let object = self.object(object_name)?;

        object.properties.iter().find(|prop| self.string(prop.name) == Some(property_name))
    }

    pub fn variable(&'a self, object_name: &str, variable_name: &str) -> Option<&'a Variable> {
        let object = self.object(object_name)?;

        object.variables.iter().find(|var| self.string(var.name) == Some(variable_name))
    }
}
