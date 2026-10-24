//! `papyrus_analyzer fmt`: formats scripts from the command line, with the
//! same options as the editor: `.editorconfig`, then `[format]` of
//! `papyrus.toml`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::{Path, PathBuf};

/// Runs the subcommand and returns the process exit code: 0 on success, 1
/// when `--check` found unformatted files or a file could not be formatted.
pub(crate) fn run(args: &[String]) -> i32 {
    let check = args.iter().any(|a| a == "--check");
    let mut paths: Vec<PathBuf> =
        args.iter().filter(|a| !a.starts_with("--")).map(PathBuf::from).collect();
    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    let mut scripts = Vec::new();
    for path in &paths {
        collect(path, &mut scripts);
    }

    let mut failed = false;
    let mut changed = 0;
    for script in &scripts {
        match format_file(script, check) {
            Ok(true) => {
                changed += 1;
                if check {
                    println!("{}", script.display());
                    failed = true;
                }
            }
            Ok(false) => {}
            Err(err) => {
                eprintln!("{}: {err}", script.display());
                failed = true;
            }
        }
    }
    if !check {
        println!("formatted {changed} of {} scripts", scripts.len());
    }
    i32::from(failed)
}

/// Formats one file; returns whether it changed (or would change).
fn format_file(path: &Path, check: bool) -> Result<bool, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (true, rest),
        None => (false, &bytes[..]),
    };
    // Writing back other encodings (often Windows-1252) as UTF-8 would
    // change the file beyond formatting.
    let text = std::str::from_utf8(body).map_err(|_| "skipped: not UTF-8".to_owned())?;
    let options = papyrus_project::format_options_for(path, |_| {});
    let formatted = papyrus_fmt::format(text, &options).map_err(|e| e.to_string())?;
    if formatted == text {
        return Ok(false);
    }
    if !check {
        let mut out = Vec::with_capacity(formatted.len() + 3);
        if bom {
            out.extend_from_slice(b"\xEF\xBB\xBF");
        }
        out.extend_from_slice(formatted.as_bytes());
        std::fs::write(path, out).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("psc")) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for entry in entries {
        let name = entry.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if entry.is_dir() && (name.starts_with('.') || name == "node_modules" || name == "target") {
            continue;
        }
        collect(&entry, out);
    }
}
