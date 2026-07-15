use core::fmt;

#[derive(Debug)]
pub enum Error {
    UnexpectedEof {
        offset: usize,
        expected: usize,
        remaining: usize,
    },

    InvalidUtf8 {
        offset: usize,
        source: core::str::Utf8Error,
    },
    InvalidDebugFunctionType {
        offset: usize,
        value: u8,
    },
    InvalidMagicNumber {
        offset: i32,
        value: u32,
    },
    InvalidGameType {
        offset: usize,
        value: u16,
    },
    InvalidVariableValue {
        offset: usize,
        value: u8,
    },
    InvalidOpCode {
        offset: usize,
        source: crate::pex::OpcodeError,
    },
    InvalidFunctionFlags {
        offset: usize,
        value: u8,
    },

    MissingAutoVerInProperty {
        /// property name
        name: String,
    },

    OverFlowVecLen {
        len: usize,
    },

    #[cfg(feature = "trace-layout")]
    Io {
        source: std::io::Error,
    },
}

#[cfg(feature = "trace-layout")]
impl From<std::io::Error> for Error {
    #[inline]
    fn from(value: std::io::Error) -> Self {
        Self::Io { source: value }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedEof { offset, expected, remaining } => {
                write!(
                    f,
                    "unexpected EOF at offset {}, expected {} more bytes, {} bytes remaining",
                    offset, expected, remaining
                )
            }

            Error::InvalidUtf8 { offset, source } => {
                write!(f, "invalid UTF-8 at offset {}: {}", offset, source)
            }

            Error::InvalidDebugFunctionType { offset, value } => {
                write!(f, "invalid debug function type 0x{:02X} at offset {}", value, offset)
            }

            Error::InvalidMagicNumber { offset, value } => {
                write!(f, "invalid magic number 0x{:08X} at offset {}", value, offset)
            }

            Error::InvalidGameType { offset, value } => {
                write!(f, "invalid game type {} at offset {}", value, offset)
            }

            Error::InvalidVariableValue { offset, value } => {
                write!(f, "invalid variable value 0x{:02X} at offset {}", value, offset)
            }

            Error::InvalidOpCode { offset, source } => {
                write!(f, "invalid opcode at offset {}: {}", offset, source)
            }

            Error::InvalidFunctionFlags { offset, value } => {
                write!(f, "invalid function flags 0x{:02X} at offset {}", value, offset)
            }

            Error::MissingAutoVerInProperty { name } => {
                write!(
                    f,
                    "The `auto_ver` variable does not exist in a Property({}) marked with the `auto_ver` flag.",
                    name
                )
            }

            Error::OverFlowVecLen { len } => {
                write!(
                    f,
                    "The maximum length of the array is expected to be u16::MAX(65535), but got {}",
                    len
                )
            }

            #[cfg(feature = "trace-layout")]
            Error::Io { source } => {
                write!(f, "I/O error: {}", source)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::InvalidUtf8 { source, .. } => Some(source),
            Error::InvalidOpCode { source, .. } => Some(source),

            #[cfg(feature = "trace-layout")]
            Error::Io { source } => Some(source),

            _ => None,
        }
    }
}
