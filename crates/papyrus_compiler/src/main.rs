//! `papyrus_compiler`: compiles Papyrus scripts, with the arguments of
//! `PapyrusCompiler.exe`. See [`papyrus_compiler::cli`].

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(papyrus_compiler::cli::run(&args));
}
