//! `papyrus_analyzer decompile`: turns `.pex` files back into source.
//!
//! Files print to standard output, or with `-o DIR` are written as
//! `DIR/Name.psc`. A directory is decompiled recursively into `-o DIR`
//! (default: `decompile` in `[build] output` of the `papyrus.toml` of the
//! current directory, else `build/papyrus/decompile`), keeping its
//! structure: `in/a/B.pex`
//! becomes `DIR/a/B.psc`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::{Path, PathBuf};

use papyrus_pex::PexFile;

/// Where directories are decompiled without `-o`: `decompile` in the
/// output directory of `papyrus.toml`, else in the default build directory.
fn default_output() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    // The current directory and its ancestors only: a manifest in some
    // subdirectory belongs to another project.
    let manifest =
        cwd.ancestors().map(|d| d.join(papyrus_project::MANIFEST_NAME)).find(|p| p.is_file());
    let project = manifest.and_then(|manifest| {
        papyrus_project::load(&manifest, &papyrus_project::Variables::new(Default::default())).ok()
    });
    match project {
        Some(p) => p.output.join("decompile"),
        None => papyrus_project::default_build_dir(&cwd).join("decompile"),
    }
}

const USAGE: &str = "\
usage: papyrus_analyzer decompile [-o DIR] PATH...

  PATH     a .pex file, or a directory decompiled recursively
  -o DIR   write the .psc files there instead of printing them
           (directories: <[build] output of papyrus.toml>/decompile,
           else build/papyrus/decompile)

examples:
  papyrus_analyzer decompile Scripts\\MyQuestScript.pex
  papyrus_analyzer decompile -o Source Scripts";

/// Runs the subcommand and returns the process exit code.
pub(crate) fn run(args: &[String]) -> i32 {
    let mut output = None;
    let mut paths = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-o" | "--output" => match it.next() {
                Some(dir) => output = Some(PathBuf::from(dir)),
                None => {
                    eprintln!("{USAGE}");
                    return 2;
                }
            },
            "-h" | "--help" | "-?" => {
                println!("{USAGE}");
                return 0;
            }
            a if a.starts_with('-') => {
                eprintln!("unknown option `{a}`\n\n{USAGE}");
                return 2;
            }
            path => paths.push(PathBuf::from(path)),
        }
    }
    if paths.is_empty() {
        eprintln!("{USAGE}");
        return 2;
    }
    let mut failed = false;
    for path in &paths {
        if path.is_dir() {
            let out = output.clone().unwrap_or_else(default_output);
            let (done, errors) = decompile_dir(path, &out);
            for e in &errors {
                eprintln!("{e}");
            }
            failed |= !errors.is_empty();
            println!(
                "decompiled {done} of {} files of {} into {}",
                done + errors.len(),
                path.display(),
                out.display()
            );
            continue;
        }
        let result = decompile_file(path, false).and_then(|(name, text)| match &output {
            None => {
                print!("{text}");
                Ok(())
            }
            Some(dir) => write(&dir.join(format!("{name}.psc")), &text),
        });
        if let Err(e) = result {
            eprintln!("{e}");
            failed = true;
        }
    }
    i32::from(failed)
}

/// Decompiles a `.pex` file to source, or with `assembly` disassembles it.
/// Returns the script name and the text; errors start with the path.
pub(crate) fn decompile_file(file: &Path, assembly: bool) -> Result<(String, String), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let pex = PexFile::from_bytes(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    let name = pex.objects.first().map_or_else(
        || file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        |o| pex.str(o.name).into_owned(),
    );
    let text = if assembly {
        papyrus_pex::disasm::disassemble(&pex, Default::default())
    } else {
        papyrus_decompiler::decompile(&pex)
    };
    Ok((name, text))
}

/// Decompiles every `.pex` file below `dir` into `out`, keeping the
/// directory structure. Returns the number of files written and the errors.
pub(crate) fn decompile_dir(dir: &Path, out: &Path) -> (usize, Vec<String>) {
    let mut files = Vec::new();
    list_pex(dir, &mut files);
    let mut done = 0;
    let mut errors = Vec::new();
    for file in &files {
        let relative = file.strip_prefix(dir).unwrap_or(file);
        let target = out.join(relative).with_extension("psc");
        match decompile_file(file, false).and_then(|(_, text)| write(&target, &text)) {
            Ok(()) => done += 1,
            Err(e) => errors.push(e),
        }
    }
    (done, errors)
}

pub(crate) fn write(path: &Path, text: &str) -> Result<(), String> {
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(path, text))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The `.pex` files below `dir`, sorted.
fn list_pex(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<(PathBuf, bool)> =
        entries.flatten().map(|e| (e.path(), e.file_type().is_ok_and(|t| t.is_dir()))).collect();
    entries.sort();
    for (path, is_dir) in entries {
        if is_dir {
            list_pex(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pex")) {
            out.push(path);
        }
    }
}
