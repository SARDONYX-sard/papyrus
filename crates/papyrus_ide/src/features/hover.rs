//! Hover information: the declaration and the documentation of a name.

use papyrus_parser::{SyntaxKind, TextRange, TextSize};

use super::{
    def_at, def_info,
    keyword_docs::{flag_file_keyword_doc, keyword_doc},
    with_sema,
};
use crate::{FileId, Workspace, sema::Sema};

/// The result of [`hover`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverResult {
    /// The hovered identifier.
    pub range: TextRange,
    /// Markdown contents.
    pub markdown: String,
}

/// Describes the name at `offset`.
pub fn hover(ws: &Workspace, file: FileId, offset: TextSize) -> Option<HoverResult> {
    with_sema(ws, file, |sema| keyword_hover(sema, offset).or_else(|| name_hover(sema, offset)))
        .flatten()
}

/// The documentation of the keyword at `offset`, with an example. For `Self`
/// and `Parent`, the script they refer to follows.
fn keyword_hover(sema: &Sema<'_>, offset: TextSize) -> Option<HoverResult> {
    let tok = sema.root.token_at_offset(offset).max_by_key(|t| t.kind().is_keyword())?;
    let doc = keyword_doc(tok.kind())?;
    let mut markdown = doc.markdown();
    if matches!(tok.kind(), SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW)
        && let Some(target) = name_hover(sema, offset)
    {
        markdown.push_str("\n\n---\n\nRefers to:\n\n");
        markdown.push_str(&target.markdown);
    }
    Some(HoverResult { range: tok.text_range(), markdown })
}

fn name_hover(sema: &Sema<'_>, offset: TextSize) -> Option<HoverResult> {
    let (range, def) = def_at(sema, offset)?;
    let info = def_info(sema, def);
    let mut markdown = format!("```papyrus\n{}\n```", info.signature);
    if let Some(container) = info.container {
        markdown.push_str(&format!("\n*{container}*"));
    }
    if let Some(line) = super::overrides::hover_line(sema, def) {
        markdown.push_str(&format!("\n\n{line}"));
    }
    if let Some(line) = nullness_line(sema, range) {
        markdown.push_str(&format!("\n\n{line}"));
    }
    if !info.docs.is_empty() {
        markdown.push_str("\n\n---\n\n");
        markdown.push_str(&info.docs);
    }
    Some(HoverResult { range, markdown })
}

/// For a local used at `range`, whether it may be `None` there, like the
/// null state of C# hovers, with a link to where the value comes from.
fn nullness_line(sema: &Sema<'_>, range: TextRange) -> Option<String> {
    use crate::{
        hir::{Body, Expr},
        none_flow::{NoneSource, Nullness},
    };
    let node = sema
        .root
        .covering_node(range)
        .ancestors()
        .find(|n| matches!(n.kind(), SyntaxKind::FUNCTION | SyntaxKind::EVENT))?;
    let body = Body::lower(sema, node)?;
    let (i, local) = body.exprs.iter().enumerate().find_map(|(i, (e, r))| match e {
        Expr::Local(l) if *r == range => Some((i, *l)),
        _ => None,
    })?;
    let types = crate::typeck::check_body(sema, &body);
    let flow = crate::none_flow::analyze(sema, &body, &types);
    let name = body.local(local).name;
    let text = |r: TextRange| &sema.text[usize::from(r.start())..usize::from(r.end())];
    let (at, why) = match flow.uses.get(&crate::hir::ExprId(i as u32))? {
        Nullness::NotNone => return Some(format!("`{name}` is not None here.")),
        Nullness::MaybeNone(NoneSource::Cast(at)) => {
            (*at, format!("the cast `{}` gives None when it fails", text(*at)))
        }
        Nullness::MaybeNone(NoneSource::Call(at)) => {
            (*at, format!("`{}` may return None", text(*at)))
        }
    };
    let here = format!("[here]({})", location_link(sema, at));
    Some(format!("`{name}` may be None here: its value comes from {here}, where {why}."))
}

/// A link opening the file of `sema` at `range`: `file:///...#L3,15`.
fn location_link(sema: &Sema<'_>, range: TextRange) -> String {
    let pos = crate::LineIndex::new(sema.text).line_col(sema.text, range.start());
    let path = sema.ws.file(sema.file).path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from(if path.starts_with('/') { "file://" } else { "file:///" });
    for c in path.chars() {
        match c {
            ' ' => uri.push_str("%20"),
            '#' => uri.push_str("%23"),
            '%' => uri.push_str("%25"),
            '(' => uri.push_str("%28"),
            ')' => uri.push_str("%29"),
            c => uri.push(c),
        }
    }
    format!("{uri}#L{},{}", pos.line + 1, pos.col + 1)
}

/// Describes the flag definition at `offset` in the flags file, or the
/// keyword (`Flag`, `Script`, ...) there.
pub fn flag_file_hover(ws: &Workspace, offset: TextSize) -> Option<HoverResult> {
    let flags = ws.flags();
    let Some(i) = flags.def_at(offset) else {
        let text: &str = &flags.text;
        let at = usize::from(offset).min(text.len());
        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let start = text[..at].rfind(|c: char| !is_word(c)).map_or(0, |i| i + 1);
        let end = text[at..].find(|c: char| !is_word(c)).map_or(text.len(), |i| at + i);
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        if start >= end || text[line_start..start].contains("//") {
            return None;
        }
        let doc = flag_file_keyword_doc(&text[start..end])?;
        let range = TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32));
        return Some(HoverResult { range, markdown: doc.markdown() });
    };
    let def = &flags.defs[i];
    let mut markdown = format!("```papyrus\n{}\n```", def.signature());
    if !def.docs.is_empty() {
        markdown.push_str("\n\n---\n\n");
        markdown.push_str(&def.docs);
    }
    Some(HoverResult { range: def.name_range, markdown })
}
