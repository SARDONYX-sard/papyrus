//! Go to definition, find references and document highlights.

use papyrus_intern::Name;
use papyrus_parser::{
    SyntaxKind, SyntaxNode, TextRange, TextSize,
    ast::{self, AstNode},
};

use super::{decl_def, def_at, def_location, with_sema};
use crate::{
    FileId, Workspace,
    sema::{Def, FileSource, Sema},
};

/// A location to navigate to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavTarget {
    /// The target file.
    pub file: FileId,
    /// The range of the declaring name, to select.
    pub focus_range: TextRange,
    /// The range of the whole declaration.
    pub full_range: TextRange,
}

/// The declarations of the name at `offset`.
pub fn goto_definition(ws: &Workspace, file: FileId, offset: TextSize) -> Vec<NavTarget> {
    with_sema(ws, file, |sema| {
        let Some((_, def)) = def_at(sema, offset) else { return Vec::new() };
        // A script may be declared by several files, e.g. two versions of a
        // library in different import directories.
        if let Def::Script { file } = def
            && let Some(name) = ws.file(file).index.name
        {
            return ws
                .scripts_named(name)
                .iter()
                .filter_map(|&f| {
                    let range = ws.file(f).index.name_range?;
                    Some(NavTarget { file: f, focus_range: range, full_range: range })
                })
                .collect();
        }
        def_location(sema, def)
            .map(|(file, focus_range, full_range)| NavTarget { file, focus_range, full_range })
            .into_iter()
            .collect()
    })
    .unwrap_or_default()
}

/// An occurrence of a name in the current file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Highlight {
    /// The identifier range.
    pub range: TextRange,
    /// The occurrence declares or assigns the name.
    pub write: bool,
}

/// All occurrences of the name at `offset` in the same file.
pub fn highlight(ws: &Workspace, file: FileId, offset: TextSize) -> Vec<Highlight> {
    with_sema(ws, file, |sema| {
        let Some((_, def)) = def_at(sema, offset) else { return Vec::new() };
        let Some(name) = def_name(sema, def) else { return Vec::new() };
        occurrences(sema, def, name)
    })
    .unwrap_or_default()
}

/// All references to the name at `offset`, in every known file.
pub fn references(
    ws: &Workspace,
    file: FileId,
    offset: TextSize,
    include_declaration: bool,
) -> Vec<(FileId, TextRange)> {
    let Some(Some((def, name))) = with_sema(ws, file, |sema| {
        let (_, def) = def_at(sema, offset)?;
        Some((def, def_name(sema, def)?))
    }) else {
        return Vec::new();
    };

    let keep = |h: &Highlight, f: FileId| {
        include_declaration || !h.write || !is_declaration(ws, def, f, h.range)
    };

    if let Def::Local { file, .. } = def {
        return with_sema(ws, file, |sema| {
            occurrences(sema, def, name)
                .into_iter()
                .filter(|h| keep(h, file))
                .map(|h| (file, h.range))
                .collect()
        })
        .unwrap_or_default();
    }

    let needle = name.as_str().to_ascii_lowercase();
    let files: Vec<FileId> = ws.files().map(|(id, _)| id).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = files.len().div_ceil(threads).max(1);

    let mut out: Vec<(FileId, TextRange)> = std::thread::scope(|s| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|chunk| {
                let needle = &needle;
                let keep = &keep;
                s.spawn(move || {
                    let mut found = Vec::new();
                    for &f in chunk {
                        let Some(src) = FileSource::load_if(ws, f, |text| {
                            contains_ignore_ascii_case(text, needle)
                        }) else {
                            continue;
                        };
                        let sema = src.sema(ws, f);
                        found.extend(
                            occurrences(&sema, def, name)
                                .into_iter()
                                .filter(|h| keep(h, f))
                                .map(|h| (f, h.range)),
                        );
                    }
                    found
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    out.sort_by_key(|&(f, r)| (f, r.start()));
    out.dedup();
    out
}

fn is_declaration(ws: &Workspace, def: Def, file: FileId, range: TextRange) -> bool {
    match def {
        Def::Local { range: r, file: f, .. } => f == file && r == range,
        Def::Member { file: f, index } => {
            f == file && ws.file(f).index.members[index as usize].name_range == range
        }
        Def::Script { file: f } => f == file && ws.file(f).index.name_range == Some(range),
        Def::Param { .. } | Def::Builtin(..) => false,
    }
}

fn def_name(sema: &Sema<'_>, def: Def) -> Option<Name> {
    Some(match def {
        Def::Local { name, .. } => name,
        Def::Member { file, index } => sema.member(file, index).name,
        Def::Param { file, member, param } => sema.member(file, member).params[param as usize].name,
        Def::Script { file } => sema.ws.file(file).index.name?,
        Def::Builtin(b, _) => Name::new(b.name()),
    })
}

/// Occurrences of `def` (spelled `name`) in the file of `sema`.
fn occurrences(sema: &Sema<'_>, def: Def, name: Name) -> Vec<Highlight> {
    let mut out = Vec::new();
    for tok in sema.root.descendant_tokens() {
        if !matches!(tok.kind(), SyntaxKind::IDENT | SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW) {
            continue;
        }
        if !tok.text().eq_ignore_ascii_case(name.as_str()) {
            continue;
        }
        let Some(parent) = tok.parent() else { continue };
        let (resolved, write) = match parent.kind() {
            SyntaxKind::NAME => (decl_def(sema, parent), true),
            SyntaxKind::NAME_REF => {
                let Some(nr) = ast::NameRef::cast(parent) else { continue };
                (sema.resolve_name_ref(nr), is_assign_target(parent))
            }
            SyntaxKind::TYPE => (ast::Type::cast(parent).and_then(|t| sema.resolve_type(t)), false),
            _ => continue,
        };
        if resolved == Some(def) {
            out.push(Highlight { range: tok.text_range(), write });
        }
    }
    out
}

fn is_assign_target(name_ref: SyntaxNode<'_>) -> bool {
    let Some(parent) = name_ref.parent() else { return false };
    let target = match parent.kind() {
        SyntaxKind::ASSIGN_STMT => Some(name_ref),
        SyntaxKind::FIELD_EXPR
            if ast::FieldExpr::cast(parent).and_then(|f| f.name_ref()).map(|n| n.syntax())
                == Some(name_ref) =>
        {
            parent.parent().filter(|p| p.kind() == SyntaxKind::ASSIGN_STMT).map(|_| parent)
        }
        _ => None,
    };
    target.is_some_and(|t| {
        t.parent()
            .and_then(ast::AssignStmt::cast)
            .and_then(|a| a.target())
            .is_some_and(|e| e.syntax() == t)
    })
}

fn contains_ignore_ascii_case(haystack: &str, needle_lower: &str) -> bool {
    let n = needle_lower.as_bytes();
    if n.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(n.len())
        .any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

impl FileSource {
    /// Like [`FileSource::load`], but skips parsing when `filter` rejects the text.
    pub(crate) fn load_if(
        ws: &Workspace,
        file: FileId,
        filter: impl FnOnce(&str) -> bool,
    ) -> Option<FileSource> {
        if ws.file(file).open.is_some() {
            return FileSource::load(ws, file).filter(|src| filter(src.text()));
        }
        let text = ws.text(file)?;
        if !filter(&text) {
            return None;
        }
        Some(FileSource::from_text(text))
    }
}
