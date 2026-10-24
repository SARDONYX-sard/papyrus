use core::fmt;

/// An error reading or writing a `.pex` file.
#[derive(Debug)]
pub enum Error {
    /// The file ends too early.
    UnexpectedEof {
        /// Where.
        offset: usize,
        /// Bytes needed.
        expected: usize,
        /// Bytes left.
        remaining: usize,
    },
    /// Bytes follow the last object.
    TrailingBytes {
        /// Where.
        offset: usize,
        /// How many.
        len: usize,
    },
    /// Not 0, 1 or 2.
    InvalidDebugFunctionType {
        /// Where.
        offset: usize,
        /// The byte.
        value: u8,
    },
    /// Not a Skyrim `.pex` file.
    InvalidMagicNumber {
        /// Where.
        offset: usize,
        /// The value.
        value: u32,
    },
    /// Not 1 (Skyrim).
    InvalidGameType {
        /// Where.
        offset: usize,
        /// The value.
        value: u16,
    },
    /// Not a value type (0 to 5).
    InvalidVariableValue {
        /// Where.
        offset: usize,
        /// The byte.
        value: u8,
    },
    /// Not an opcode.
    InvalidOpCode {
        /// Where.
        offset: usize,
        /// Why.
        source: crate::pex::OpcodeError,
    },
    /// An auto property without its variable.
    MissingAutoVerInProperty {
        /// The property.
        name: String,
    },
    /// More than 65535 items.
    OverFlowVecLen {
        /// How many.
        len: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedEof { offset, expected, remaining } => write!(
                f,
                "unexpected EOF at offset {offset}, expected {expected} more bytes, {remaining} bytes remaining"
            ),
            Error::TrailingBytes { offset, len } => {
                write!(f, "{len} unexpected bytes at offset {offset}")
            }
            Error::InvalidDebugFunctionType { offset, value } => {
                write!(f, "invalid debug function type 0x{value:02X} at offset {offset}")
            }
            Error::InvalidMagicNumber { offset, value } => {
                write!(f, "invalid magic number 0x{value:08X} at offset {offset}")
            }
            Error::InvalidGameType { offset, value } => {
                write!(f, "invalid game type {value} at offset {offset}")
            }
            Error::InvalidVariableValue { offset, value } => {
                write!(f, "invalid variable value 0x{value:02X} at offset {offset}")
            }
            Error::InvalidOpCode { offset, source } => {
                write!(f, "invalid opcode at offset {offset}: {source}")
            }
            Error::MissingAutoVerInProperty { name } => {
                write!(f, "the auto property {name} has no variable")
            }
            Error::OverFlowVecLen { len } => {
                write!(f, "the maximum length of a list is 65535, but got {len}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::InvalidOpCode { source, .. } => Some(source),
            _ => None,
        }
    }
}
