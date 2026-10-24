//! Hover information: the declaration and the documentation of a name.

use papyrus_parser::{TextRange, TextSize};

use super::{def_at, def_info, with_sema};
use crate::{FileId, Workspace};

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
    with_sema(ws, file, |sema| {
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
    })
    .flatten()
}
