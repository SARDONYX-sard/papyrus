//! Syntax and semantic diagnostics.
//!
//! Semantic checks that depend on other scripts (unknown types, members and
//! names) only run when the workspace knows the base game scripts (detected by
//! the presence of `Form`), and member checks only when the whole `extends`
//! chain of the receiver is known. Without the game sources every name would
//! look unresolved.

use papyrus_intern::Name;
use papyrus_parser::{
    SyntaxKind, TextRange,
    ast::{self, AstNode},
};
use rustc_hash::FxHashMap;

use super::{in_body, with_sema};
use crate::{
    FileId, Workspace,
    index::{Flags, MemberKind},
    sema::{Builtin, Sema, Ty},
};

/// The severity of a [`Diagnostic`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The script does not compile.
    Error,
    /// The script probably has a bug.
    Warning,
}

/// A problem in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The location.
    pub range: TextRange,
    /// The severity.
    pub severity: Severity,
    /// The message.
    pub message: String,
    /// A stable identifier of the check.
    pub code: &'static str,
}

impl Diagnostic {
    /// Returns `true` for checks that depend on other scripts being known:
    /// unknown types, members, parameters and names.
    pub fn needs_workspace(&self) -> bool {
        matches!(self.code, "unknown-type" | "unknown-member" | "unknown-param" | "unresolved-name")
    }

    fn error(range: TextRange, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic { range, severity: Severity::Error, message: message.into(), code }
    }

    fn warning(range: TextRange, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic { range, severity: Severity::Warning, message: message.into(), code }
    }
}

/// All diagnostics of a file.
pub fn diagnostics(ws: &Workspace, file: FileId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let data = ws.file(file);
    let syntax_errors: Vec<_> = match &data.open {
        Some(doc) => doc.errors.to_vec(),
        None => match ws.text(file) {
            Some(text) => papyrus_parser::parse(&text).errors().to_vec(),
            None => Vec::new(),
        },
    };
    let has_syntax_errors = !syntax_errors.is_empty();
    out.extend(
        syntax_errors.into_iter().map(|e| Diagnostic::error(e.range(), "syntax", e.message())),
    );

    with_sema(ws, file, |sema| {
        check_script_name(sema, &mut out);
        check_duplicates(sema, &mut out);
        check_flags(sema, &mut out);
        // Name resolution on broken code is noisy; fix the syntax first.
        if !has_syntax_errors && ws.script(Name::new("Form")).is_some() {
            check_names(sema, &mut out);
        }
    });
    out.sort_by_key(|d| (d.range.start(), d.severity));
    out
}

fn check_script_name(sema: &Sema<'_>, out: &mut Vec<Diagnostic>) {
    let index = sema.index;
    let (Some(name), Some(range)) = (index.name, index.name_range) else { return };
    let path = &sema.ws.file(sema.file).path;
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy()) else { return };
    if !stem.eq_ignore_ascii_case(name.as_str()) {
        out.push(Diagnostic::error(
            range,
            "script-name-mismatch",
            format!("script name `{name}` does not match the file name `{stem}.psc`"),
        ));
    }
    if let Some(ext) = index.extends
        && ext == name
    {
        out.push(Diagnostic::error(
            index.extends_range.unwrap_or(range),
            "extends-self",
            "a script cannot extend itself",
        ));
    }
}

fn check_duplicates(sema: &Sema<'_>, out: &mut Vec<Diagnostic>) {
    // Functions and events are scoped by state; properties, variables and
    // states share the script scope.
    let mut seen: FxHashMap<(Option<Name>, Option<Name>, u8, Name), usize> = FxHashMap::default();
    for (i, m) in sema.index.members.iter().enumerate() {
        let callable = m.is_callable();
        let category = match m.kind {
            MemberKind::State => 2,
            _ if callable => 1,
            _ => 0,
        };
        let key = (if callable { m.state } else { None }, m.property, category, m.name);
        if let Some(&first) = seen.get(&key) {
            let first = &sema.index.members[first];
            if m.kind == MemberKind::State && first.kind == MemberKind::State {
                out.push(Diagnostic::error(
                    m.name_range,
                    "duplicate-definition",
                    format!("state `{}` is already defined", m.name),
                ));
            } else {
                out.push(Diagnostic::error(
                    m.name_range,
                    "duplicate-definition",
                    format!("`{}` is already defined in this scope", m.name),
                ));
            }
        } else {
            seen.insert(key, i);
        }
    }

    // A state function must match a function of the empty state.
    for m in sema.index.members.iter() {
        let Some(state) = m.state else { continue };
        if !m.is_callable() || m.property.is_some() {
            continue;
        }
        let script = sema.script_name();
        let declared_elsewhere = sema
            .index
            .members
            .iter()
            .any(|o| o.state.is_none() && o.is_callable() && o.name == m.name)
            || script.is_some_and(|s| {
                sema.chain(s)
                    .skip(1)
                    .any(|(_, idx)| idx.find(m.name, |o| o.is_callable()).is_some())
            })
            || m.name.as_str().eq_ignore_ascii_case(Builtin::OnBeginState.name())
            || m.name.as_str().eq_ignore_ascii_case(Builtin::OnEndState.name());
        let parent_unknown = sema.index.extends.is_some_and(|p| sema.ws.script(p).is_none());
        if !declared_elsewhere && !parent_unknown && m.kind == MemberKind::Function {
            out.push(Diagnostic::error(
                m.name_range,
                "state-function-not-declared",
                format!(
                    "function `{}` in state `{state}` must also be declared outside of any state",
                    m.name
                ),
            ));
        }
    }
}

fn check_flags(sema: &Sema<'_>, out: &mut Vec<Diagnostic>) {
    for node in sema.root.descendants() {
        let Some(flag) = ast::Flag::cast(node) else { continue };
        if Flags::from_name(flag.text()).is_none() {
            out.push(Diagnostic::warning(
                node.text_range(),
                "unknown-flag",
                format!("unknown flag `{}`", flag.text()),
            ));
        }
    }
}

fn check_names(sema: &Sema<'_>, out: &mut Vec<Diagnostic>) {
    for node in sema.root.descendants() {
        match node.kind() {
            SyntaxKind::TYPE => {
                let Some(ty) = ast::Type::cast(node) else { continue };
                if ty.is_primitive() || ty.base().is_none() {
                    continue;
                }
                if sema.resolve_type(ty).is_none() {
                    out.push(Diagnostic::warning(
                        ty.base().map_or(node.text_range(), |b| b.text_range()),
                        "unknown-type",
                        format!("unknown type `{}`", ty.base_text()),
                    ));
                }
            }
            SyntaxKind::EXTENDS_CLAUSE => {
                let Some(nr) = node.children().find_map(ast::NameRef::cast) else { continue };
                if sema.resolve_name_ref(nr).is_none() {
                    out.push(Diagnostic::warning(
                        nr.syntax().text_range(),
                        "unknown-type",
                        format!("unknown script `{}`", nr.text()),
                    ));
                }
            }
            SyntaxKind::NAME_REF if in_body(node) => check_name_ref(sema, node, out),
            _ => {}
        }
    }
}

fn check_name_ref(
    sema: &Sema<'_>,
    node: papyrus_parser::SyntaxNode<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(nr) = ast::NameRef::cast(node) else { return };
    if nr.is_self() || nr.is_parent() || sema.resolve_name_ref(nr).is_some() {
        return;
    }
    let Some(parent) = node.parent() else { return };
    let range = nr.syntax().text_range();

    if let Some(field) = ast::FieldExpr::cast(parent).filter(|f| f.name_ref() == Some(nr)) {
        // `receiver.member`: only report when the receiver type is fully known.
        let Some(recv) = field.receiver().and_then(|r| sema.infer(r)) else { return };
        let script = match recv {
            Ty::Value(t) if t.array => {
                out.push(Diagnostic::warning(
                    range,
                    "unknown-member",
                    format!("arrays have no member `{}`", nr.text()),
                ));
                return;
            }
            Ty::Value(t) => match t.object() {
                Some(s) => s,
                None => {
                    out.push(Diagnostic::warning(
                        range,
                        "unknown-member",
                        format!("type `{t}` has no member `{}`", nr.text()),
                    ));
                    return;
                }
            },
            Ty::Static(s) => s,
            Ty::None => return,
        };
        if sema.chain_is_complete(script) {
            out.push(Diagnostic::warning(
                range,
                "unknown-member",
                format!("`{script}` has no member `{}`", nr.text()),
            ));
        }
        return;
    }

    if ast::Arg::cast(parent).is_some_and(|a| a.name() == Some(nr)) {
        let call = parent.parent().and_then(|p| p.parent()).and_then(ast::CallExpr::cast);
        if call.and_then(|c| c.callee_name()).and_then(|n| sema.resolve_name_ref(n)).is_some() {
            out.push(Diagnostic::warning(
                range,
                "unknown-param",
                format!("no parameter named `{}`", nr.text()),
            ));
        }
        return;
    }

    // A plain name: only report when every script it could come from is known.
    let complete = sema.script_name().is_none_or(|s| sema.chain_is_complete(s))
        && sema.index.imports.iter().all(|&i| sema.ws.script(i).is_some());
    if complete {
        out.push(Diagnostic::warning(
            range,
            "unresolved-name",
            format!("cannot find `{}` in this scope", nr.text()),
        ));
    }
}
