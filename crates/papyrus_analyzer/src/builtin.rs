//! The script built into papyrus_analyzer: `PapyrusAnalyzer`, the assertions
//! of tests ([`papyrus_vm::PAPYRUS_ANALYZER_PSC`]).
//!
//! Only the scripts of the test directories see it, and a script of the
//! same name of the project or its imports wins over it.

use std::path::{Path, PathBuf};

use papyrus_ide::{Origin, Workspace};
use papyrus_vm::{PAPYRUS_ANALYZER, PAPYRUS_ANALYZER_PSC};

/// Where the built-in script is written, for go to definition: a temporary
/// directory, outside of the workspace, so it is never edited.
fn path() -> PathBuf {
    std::env::temp_dir()
        .join(format!("papyrus_analyzer-{}", env!("CARGO_PKG_VERSION")))
        .join(format!("{PAPYRUS_ANALYZER}.psc"))
}

/// Adds the built-in script to `ws`, seen by the files below `test_dirs`.
pub(crate) fn install(ws: &mut Workspace, test_dirs: &[PathBuf]) {
    ws.set_test_dirs(test_dirs);
    let path = path();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(PAPYRUS_ANALYZER_PSC) {
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, PAPYRUS_ANALYZER_PSC));
        if let Err(e) = written {
            log::warn!("cannot write {}: {e}", path.display());
        }
    }
    ws.set_disk_index(path, papyrus_ide::index_text(PAPYRUS_ANALYZER_PSC), Origin::Builtin);
}

/// The script of the workspace hiding the built-in one, if any.
pub(crate) fn shadowed_by(ws: &Workspace) -> Option<&Path> {
    ws.files()
        .find(|(_, d)| {
            d.origin != Origin::Builtin
                && d.index.name.is_some_and(|n| n.as_str().eq_ignore_ascii_case(PAPYRUS_ANALYZER))
        })
        .map(|(_, d)| d.path.as_path())
}
