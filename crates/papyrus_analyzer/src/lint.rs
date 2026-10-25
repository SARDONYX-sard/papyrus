//! `papyrus_analyzer lint`: checks scripts like `cargo clippy`, and with
//! `--fix` applies the automatic fixes like `fmt` rewrites files.
//!
//! The levels come from `[lints]` of the `papyrus.toml` of the current
//! directory (or its ancestors), then from `-A`, `-W` and `-D`, which win.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use papyrus_ide::{Severity, Workspace};
use papyrus_lint::{Level, LintConfig};

const USAGE: &str = "\
usage: papyrus_analyzer lint [OPTIONS] [PATH]...

  PATH            scripts, or directories searched recursively (default: .)

options:
  --fix           apply the automatic fixes and rewrite the files
  -A, --allow NAME   silence a lint, a group or `all`
  -W, --warn NAME    report as a warning
  -D, --deny NAME    report as an error; `-D warnings` turns every warning
                     into an error
  --list          list the lints, their group and level

Levels come from [lints] of papyrus.toml, then from the flags. A lint wins
over its group, which wins over `all`.

exit code: 1 when an error (a denied lint) is reported, else 0.

examples:
  papyrus_analyzer lint Scripts\\Source
  papyrus_analyzer lint -D warnings -A style
  papyrus_analyzer lint --fix Scripts\\Source\\MyQuest.psc";

/// Runs the subcommand and returns the process exit code.
pub(crate) fn run(args: &[String]) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    // `[lints]` of papyrus.toml first; the flags are added after and win.
    let manifest =
        cwd.ancestors().map(|d| d.join(papyrus_project::MANIFEST_NAME)).find(|p| p.is_file());
    let mut config = match &manifest {
        Some(m) => match papyrus_project::load_lints(m) {
            Ok(c) => c,
            Err(errors) => {
                for e in errors {
                    eprintln!("{}: {}", m.display(), e.message);
                }
                return 2;
            }
        },
        None => LintConfig::default(),
    };
    let mut fix = false;
    let mut list = false;
    let mut paths = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let level = match arg.as_str() {
            "-A" | "--allow" => Some(Level::Allow),
            "-W" | "--warn" => Some(Level::Warn),
            "-D" | "--deny" => Some(Level::Deny),
            "--fix" => {
                fix = true;
                None
            }
            "--list" => {
                list = true;
                None
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return 0;
            }
            a if a.starts_with('-') => {
                eprintln!("unknown option `{a}`\n\n{USAGE}");
                return 2;
            }
            path => {
                paths.push(PathBuf::from(path));
                None
            }
        };
        if let Some(level) = level {
            let Some(name) = it.next() else {
                eprintln!("`{arg}` needs a lint name\n\n{USAGE}");
                return 2;
            };
            if let Err(e) = config.set(name, level) {
                eprintln!("{e}");
                return 2;
            }
        }
    }

    if list {
        println!("{:<26} {:<12} level", "lint", "group");
        for lint in papyrus_lint::LINTS {
            println!(
                "{:<26} {:<12} {:<6} {}",
                lint.code,
                lint.group.name(),
                config.level(lint).to_string(),
                lint.summary
            );
        }
        return 0;
    }
    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    let mut scripts = Vec::new();
    for path in &paths {
        collect(path, &mut scripts);
    }

    // Files are independent (lints look at one script): each gets its own
    // workspace, in parallel; reports keep the order of the files.
    use rayon::prelude::*;
    let reports: Vec<FileReport> = scripts.par_iter().map(|p| lint_file(p, &config, fix)).collect();
    let (mut warnings, mut errors, mut fixed, mut fixed_files) = (0, 0, 0, 0);
    for report in reports {
        for line in &report.notes {
            eprintln!("{line}");
        }
        for (severity, line) in &report.problems {
            println!("{line}");
            match severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
            }
        }
        if report.fixed > 0 {
            fixed += report.fixed;
            fixed_files += 1;
        }
    }
    if fix {
        eprintln!("fixed {fixed} problems in {fixed_files} files");
    }
    eprintln!("{errors} errors, {warnings} warnings in {} scripts", scripts.len());
    i32::from(errors > 0)
}

/// What linting one file gives.
#[derive(Default)]
struct FileReport {
    /// The problems left, printed.
    problems: Vec<(Severity, String)>,
    /// Messages about the file (not fixed, cannot write).
    notes: Vec<String>,
    /// The number of applied fixes.
    fixed: usize,
}

fn lint_file(path: &Path, config: &LintConfig, fix: bool) -> FileReport {
    let mut report = FileReport::default();
    let Some(text) = papyrus_ide::read_file(path) else { return report };
    let mut ws = Workspace::new();
    let file = ws.set_open(path.to_path_buf(), Arc::from(text.as_str()), 0);
    // Like `fmt`: writing other encodings (often Windows-1252) back as UTF-8
    // would change more than the fixes.
    let bytes = std::fs::read(path).unwrap_or_default();
    let bom = bytes.starts_with(b"\xEF\xBB\xBF");
    let utf8 = std::str::from_utf8(&bytes).is_ok();
    if fix && !utf8 {
        report.notes.push(format!("{}: not fixed: not UTF-8", path.display()));
    }
    if fix && utf8 {
        let (new, applied) = papyrus_lint::fix_all(&mut ws, file, config);
        if applied > 0 {
            let mut out = Vec::with_capacity(new.len() + 3);
            if bom {
                out.extend_from_slice(b"\xEF\xBB\xBF");
            }
            out.extend_from_slice(new.as_bytes());
            match std::fs::write(path, out) {
                Ok(()) => report.fixed = applied,
                Err(e) => report.notes.push(format!("{}: {e}", path.display())),
            }
        }
    }
    // What is left (everything without `--fix`).
    let text = ws.text(file).unwrap_or_default();
    for d in papyrus_lint::lint(&ws, file, config) {
        let before = &text[..usize::from(d.range.start()).min(text.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        let kind = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        report.problems.push((
            d.severity,
            format!("{}({line},{col}): {kind}[{}]: {}", path.display(), d.code, d.message),
        ));
    }
    report
}
fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("psc")) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else { return };
    let mut entries: Vec<(PathBuf, bool)> =
        entries.flatten().map(|e| (e.path(), e.file_type().is_ok_and(|t| t.is_dir()))).collect();
    entries.sort();
    for (path, is_dir) in entries {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if is_dir && (name.starts_with('.') || name == "node_modules" || name == "target") {
            continue;
        }
        collect(&path, out);
    }
}
