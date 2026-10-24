//! `papyrus_analyzer disasm`: prints the disassembly of `.pex` files.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use papyrus_pex::{PexFile, disasm::DisasmOptions};

/// Runs the subcommand and returns the process exit code.
pub(crate) fn run(args: &[String]) -> i32 {
    let options = DisasmOptions {
        skip_header: args.iter().any(|a| a == "--no-header"),
        skip_lines: args.iter().any(|a| a == "--no-lines"),
    };
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        eprintln!("usage: papyrus_analyzer disasm [--no-header] [--no-lines] FILE.pex...");
        return 2;
    }
    let mut code = 0;
    for file in files {
        let bytes = match std::fs::read(file) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{file}: {e}");
                code = 1;
                continue;
            }
        };
        match PexFile::from_bytes(&bytes) {
            Ok(pex) => print!("{}", papyrus_pex::disasm::disassemble(&pex, options)),
            Err(e) => {
                eprintln!("{file}: {e}");
                code = 1;
            }
        }
    }
    code
}
