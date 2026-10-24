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
    if !info.docs.is_empty() {
        markdown.push_str("\n\n---\n\n");
        markdown.push_str(&info.docs);
    }
    Some(HoverResult { range, markdown })
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
