//! `papyrus_analyzer compile`: compiles scripts with the built-in compiler.
//!
//! With arguments, they are those of `PapyrusCompiler.exe` (see
//! [`papyrus_compiler::cli`]). Without, the project of the `papyrus.toml` of
//! the current directory is compiled: its sources, imports, flags file,
//! output directory and optimization setting.

#![allow(clippy::print_stderr)]

use papyrus_compiler::cli::{self, Args};

/// Runs the subcommand and returns the process exit code.
pub(crate) fn run(args: &[String]) -> i32 {
    if !args.is_empty() {
        return cli::run(args);
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let Some(manifest) = papyrus_project::find_manifest(&cwd) else {
        eprintln!(
            "no papyrus.toml found; pass the arguments of PapyrusCompiler.exe\n\n{}",
            cli::USAGE
        );
        return 2;
    };
    let vars = papyrus_project::Variables::for_command_line(&cwd);
    let project = match papyrus_project::load(&manifest, &vars) {
        Ok(p) => p,
        Err(errors) => {
            for e in errors {
                eprintln!("{}: {}", manifest.display(), e.message);
            }
            return 1;
        }
    };
    let mut args = match Args::parse(&[]) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    args.targets = project.sources.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    args.all = true;
    args.imports = project.import_dirs();
    args.flags = project.flags.clone();
    args.output = Some(project.output.clone());
    args.options.optimize = project.optimize;
    args.quiet = project.quiet;
    cli::run_args(&args)
}
