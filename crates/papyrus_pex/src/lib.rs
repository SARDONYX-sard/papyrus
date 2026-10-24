//! Compiled Papyrus scripts (`.pex`): data model, reader, writer and
//! disassembler.
//!
//! ```no_run
//! let bytes = std::fs::read("Foo.pex").unwrap();
//! let pex = papyrus_pex::PexFile::from_bytes(&bytes).unwrap();
//! assert_eq!(pex.to_bytes().unwrap(), bytes);
//! println!("{}", papyrus_pex::disasm::disassemble(&pex, Default::default()));
//! ```

pub mod disasm;
mod error;
pub mod normalize;
pub mod pex;
mod read;
mod write;

pub use self::{
    error::Error,
    pex::{PexFile, PexStr},
};
