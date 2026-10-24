//! IDE features. Shared helpers live here.

pub(crate) mod completion;
pub(crate) mod diagnostics;
pub(crate) mod folding;
pub(crate) mod hover;
pub(crate) mod navigation;
pub(crate) mod semantic_tokens;
pub(crate) mod signature_help;
pub(crate) mod symbols;

use papyrus_intern::Name;
use papyrus_parser::{
    SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
    ast::{self, AstNode},
};

use crate::{
    FileId,
    index::{MemberKind, TypeRef, render_docs},
    sema::{Def, FileSource, LocalKind, Sema},
};

/// The identifier-like token at `offset`, preferring identifiers over
/// punctuation when the offset is between two tokens.
pub(crate) fn ident_at<'a>(sema: &Sema<'a>, offset: TextSize) -> Option<SyntaxToken<'a>> {
    let tok = sema.root.token_at_offset(offset).max_by_key(|t| {
        matches!(t.kind(), SyntaxKind::IDENT | SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW)
            || t.kind().is_primitive_type()
    })?;
    matches!(tok.kind(), SyntaxKind::IDENT | SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW)
        .then_some(tok)
}

/// What the identifier at `offset` refers to, with the identifier range.
pub(crate) fn def_at(sema: &Sema<'_>, offset: TextSize) -> Option<(TextRange, Def)> {
    let tok = ident_at(sema, offset)?;
    let range = tok.text_range();
    let parent = tok.parent()?;
    let def = match parent.kind() {
        SyntaxKind::NAME => decl_def(sema, parent)?,
        SyntaxKind::NAME_REF => sema.resolve_name_ref(ast::NameRef::cast(parent)?)?,
        SyntaxKind::TYPE => sema.resolve_type(ast::Type::cast(parent)?)?,
        _ => return None,
    };
    Some((range, def))
}

/// The definition introduced by a `NAME` node.
pub(crate) fn decl_def(sema: &Sema<'_>, name: SyntaxNode<'_>) -> Option<Def> {
    let decl = name.parent()?;
    let range = name.text_range();
    let text = ast::Name::cast(name)?.text();
    match decl.kind() {
        SyntaxKind::SCRIPT_HEADER => Some(Def::Script { file: sema.file }),
        SyntaxKind::PARAM => Some(Def::Local {
            file: sema.file,
            name: Name::new(text),
            range,
            ty: ast::Param::cast(decl)?.ty().and_then(TypeRef::from_ast),
            kind: LocalKind::Parameter,
        }),
        SyntaxKind::VAR_DECL if decl.parent().is_some_and(|p| p.kind() == SyntaxKind::BLOCK) => {
            Some(Def::Local {
                file: sema.file,
                name: Name::new(text),
                range,
                ty: ast::VarDecl::cast(decl)?.ty().and_then(TypeRef::from_ast),
                kind: LocalKind::Variable,
            })
        }
        _ => {
            let index = sema.index.members.iter().position(|m| m.name_range == range)?;
            Some(Def::Member { file: sema.file, index: index as u32 })
        }
    }
}

/// Presentation of a definition.
pub(crate) struct DefInfo {
    /// Pseudo source code of the declaration.
    pub(crate) signature: String,
    /// Where it is declared, e.g. `Actor` or `local variable`.
    pub(crate) container: Option<String>,
    /// Rendered documentation comments.
    pub(crate) docs: String,
}

pub(crate) fn def_info(sema: &Sema<'_>, def: Def) -> DefInfo {
    match def {
        Def::Local { name, ty, kind, .. } => DefInfo {
            signature: match ty {
                Some(ty) => format!("{ty} {name}"),
                None => name.to_string(),
            },
            container: Some(
                match kind {
                    LocalKind::Variable => "local variable",
                    LocalKind::Parameter => "parameter",
                }
                .to_owned(),
            ),
            docs: String::new(),
        },
        Def::Member { file, index } => {
            let member = sema.member(file, index);
            let script = sema.ws.file(file).index.name_str();
            let container = match (member.property, member.state) {
                (Some(prop), _) => format!("{script}.{prop}"),
                (None, Some(state)) => format!("{script} (State {state})"),
                (None, None) => script.to_owned(),
            };
            DefInfo {
                signature: member.signature(),
                container: Some(container),
                docs: docs_in(sema, file, &member.docs),
            }
        }
        Def::Param { file, member, param } => {
            let m = sema.member(file, member);
            let mut signature = String::new();
            crate::index::render_param(&mut signature, &m.params[param as usize]);
            DefInfo {
                signature,
                container: Some(format!("parameter of {}", m.name)),
                docs: String::new(),
            }
        }
        Def::Script { file } => {
            let index = &sema.ws.file(file).index;
            let mut signature = format!("ScriptName {}", index.name_str());
            if let Some(ext) = index.extends {
                signature.push_str(&format!(" extends {ext}"));
            }
            for (flag, name) in [
                (crate::index::Flags::NATIVE, "Native"),
                (crate::index::Flags::CONDITIONAL, "Conditional"),
                (crate::index::Flags::HIDDEN, "Hidden"),
            ] {
                if index.flags.contains(flag) {
                    signature.push(' ');
                    signature.push_str(name);
                }
            }
            DefInfo { signature, container: None, docs: docs_in(sema, file, &index.docs) }
        }
        Def::Builtin(b, elem) => DefInfo {
            signature: b.signature(elem),
            container: Some("built-in".to_owned()),
            docs: b.docs().to_owned(),
        },
    }
}

/// Renders documentation comments of a file, loading it if needed.
pub(crate) fn docs_in(sema: &Sema<'_>, file: FileId, ranges: &[TextRange]) -> String {
    if ranges.is_empty() {
        return String::new();
    }
    if file == sema.file {
        return render_docs(sema.text, ranges);
    }
    FileSource::load(sema.ws, file).map_or_else(String::new, |src| render_docs(src.text(), ranges))
}

/// The declaration location of a definition: `(file, name range, full range)`.
pub(crate) fn def_location(sema: &Sema<'_>, def: Def) -> Option<(FileId, TextRange, TextRange)> {
    match def {
        Def::Local { file, range, .. } => Some((file, range, range)),
        Def::Member { file, index } => {
            let m = sema.member(file, index);
            Some((file, m.name_range, m.full_range))
        }
        Def::Param { file, member, .. } => {
            let m = sema.member(file, member);
            Some((file, m.name_range, m.full_range))
        }
        Def::Script { file } => {
            let index = &sema.ws.file(file).index;
            let range = index.name_range?;
            Some((file, range, range))
        }
        Def::Builtin(..) => None,
    }
}

/// A short label for completion details, e.g. `Int Function`.
pub(crate) fn member_kind_label(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Function => "Function",
        MemberKind::Event => "Event",
        MemberKind::Property => "Property",
        MemberKind::Variable => "Variable",
        MemberKind::State => "State",
    }
}

/// Runs `f` with semantic access to `file`.
pub(crate) fn with_sema<R>(
    ws: &crate::Workspace,
    file: FileId,
    f: impl FnOnce(&Sema<'_>) -> R,
) -> Option<R> {
    let src = FileSource::load(ws, file)?;
    let sema = src.sema(ws, file);
    Some(f(&sema))
}

/// Returns `true` if `node` is inside a function or event body.
pub(crate) fn in_body(node: SyntaxNode<'_>) -> bool {
    std::iter::once(node).chain(node.ancestors()).any(|n| n.kind() == SyntaxKind::BLOCK)
}
