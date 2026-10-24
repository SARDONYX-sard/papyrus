//! Inlay hints: parameter names before the arguments of calls.
//!
//! `Debug.Trace(msg, 2)` shows as `Debug.Trace(asTextToPrint: msg, aiSeverity: 2)`.
//! Like rust-analyzer, hints are left out when they would only repeat the
//! argument: named arguments, and arguments spelled like the parameter, the
//! Papyrus type prefixes (`ak`, `ai`, `as`, ...) set aside, so that `akActor`
//! gets no hint for an argument `actor`.

use papyrus_intern::Name;
use papyrus_parser::{
    TextRange, TextSize,
    ast::{self, AstNode},
};

use super::with_sema;
use crate::{
    FileId, Workspace,
    sema::{Builtin, Def, Sema},
};

/// An inlay hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlayHint {
    /// Where the hint is shown: before the argument.
    pub offset: TextSize,
    /// The text, e.g. `akActor:`.
    pub label: String,
    /// The declaration of the parameter, for the tooltip.
    pub tooltip: String,
}

/// Parameter name hints of the calls inside `range` (the whole file when
/// `None`).
pub fn inlay_hints(ws: &Workspace, file: FileId, range: Option<TextRange>) -> Vec<InlayHint> {
    with_sema(ws, file, |sema| {
        let mut out = Vec::new();
        for node in sema.root.descendants() {
            if range.is_some_and(|r| r.intersect(node.text_range()).is_none()) {
                continue;
            }
            if let Some(call) = ast::CallExpr::cast(node) {
                call_hints(sema, call, &mut out);
            }
        }
        out
    })
    .unwrap_or_default()
}

fn call_hints(sema: &Sema<'_>, call: ast::CallExpr<'_>, out: &mut Vec<InlayHint>) {
    let Some(callee) = call.callee_name() else { return };
    let params: Vec<(Name, String)> = match sema.resolve_name_ref(callee) {
        Some(Def::Member { file, index }) => {
            let member = sema.member(file, index);
            if !member.is_callable() {
                return;
            }
            member
                .params
                .iter()
                .map(|p| {
                    let mut decl = String::new();
                    crate::index::render_param(&mut decl, p);
                    (p.name, decl)
                })
                .collect()
        }
        Some(Def::Builtin(b, elem)) => builtin_params(b, elem),
        _ => return,
    };
    let Some(args) = call.arg_list() else { return };
    let positional = args.args().take_while(|a| a.name().is_none());
    for (arg, (name, decl)) in positional.zip(params) {
        let Some(value) = arg.value() else { continue };
        if repeats(value, name) {
            continue;
        }
        out.push(InlayHint {
            offset: value.syntax().trimmed_range().start(),
            label: format!("{name}:"),
            tooltip: decl,
        });
    }
}

fn builtin_params(b: Builtin, elem: Option<crate::index::TypeRef>) -> Vec<(Name, String)> {
    b.params(elem)
        .into_iter()
        .filter_map(|decl| {
            // `Int aiStartIndex = 0`: the name is the second word.
            let name = decl.split_whitespace().nth(1)?.to_owned();
            Some((Name::new(&name), decl))
        })
        .collect()
}

/// Whether the argument already says the parameter name.
fn repeats(value: ast::Expr<'_>, param: Name) -> bool {
    let arg_name = match value {
        ast::Expr::NameRef(n) => n.text(),
        ast::Expr::Field(f) => f.name_ref().map_or("", |n| n.text()),
        ast::Expr::Call(c) => c.callee_name().map_or("", |n| n.text()),
        _ => return false,
    };
    let param = param.as_str();
    arg_name.eq_ignore_ascii_case(param)
        || strip_type_prefix(arg_name).eq_ignore_ascii_case(strip_type_prefix(param))
}

/// `akActor` → `Actor`: Papyrus parameters are prefixed by `a` (argument)
/// and a type letter (`k` object, `i` int, `f` float, `b` bool, `s` string).
fn strip_type_prefix(name: &str) -> &str {
    let bytes = name.as_bytes();
    if bytes.len() > 2
        && bytes[0] == b'a'
        && matches!(bytes[1], b'k' | b'i' | b'f' | b'b' | b's')
        && bytes[2].is_ascii_uppercase()
    {
        &name[2..]
    } else {
        name
    }
}
