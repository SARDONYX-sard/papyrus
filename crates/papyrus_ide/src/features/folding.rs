//! Folding ranges for declarations, blocks and comments.

use papyrus_parser::{SyntaxKind, TextRange};

use super::with_sema;
use crate::{FileId, Workspace};

/// What a fold contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldKind {
    /// A declaration or a block statement.
    Region,
    /// A multi-line comment or a run of line comments.
    Comment,
}

/// A foldable range. The fold spans the lines from the start to the end of
/// `range`; the line of `range.end()` stays visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fold {
    /// The folded range.
    pub range: TextRange,
    /// The kind.
    pub kind: FoldKind,
}

/// All folds of a file.
pub fn folding_ranges(ws: &Workspace, file: FileId) -> Vec<Fold> {
    with_sema(ws, file, |sema| {
        let mut out = Vec::new();
        for node in sema.root.descendants() {
            match node.kind() {
                SyntaxKind::FUNCTION
                | SyntaxKind::EVENT
                | SyntaxKind::STATE
                | SyntaxKind::PROPERTY
                | SyntaxKind::IF_STMT
                | SyntaxKind::ELSE_IF_CLAUSE
                | SyntaxKind::ELSE_CLAUSE
                | SyntaxKind::WHILE_STMT => {
                    // Fold up to the closing keyword (`EndIf`, `ElseIf`, ...), which
                    // stays visible.
                    let range = node.trimmed_range();
                    let end = match node.kind() {
                        SyntaxKind::IF_STMT => node
                            .children()
                            .find(|c| {
                                matches!(
                                    c.kind(),
                                    SyntaxKind::ELSE_IF_CLAUSE | SyntaxKind::ELSE_CLAUSE
                                )
                            })
                            .map_or(range.end(), |c| c.text_range().start()),
                        _ => node.last_token().map_or(range.end(), |t| t.text_range().start()),
                    };
                    if end > range.start() {
                        out.push(Fold {
                            range: TextRange::new(range.start(), end),
                            kind: FoldKind::Region,
                        });
                    }
                }
                _ => {}
            }
        }

        // Comments: multi-line block comments, and runs of line comments.
        let mut run: Option<TextRange> = None;
        let mut lines_in_run = 0;
        for tok in sema.root.descendant_tokens() {
            match tok.kind() {
                SyntaxKind::BLOCK_COMMENT | SyntaxKind::DOC_COMMENT
                    if tok.text().contains('\n') =>
                {
                    out.push(Fold { range: tok.text_range(), kind: FoldKind::Comment });
                }
                SyntaxKind::LINE_COMMENT => {
                    run = Some(run.map_or(tok.text_range(), |r| r.cover(tok.text_range())));
                    lines_in_run += 1;
                }
                SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE => {}
                _ => {
                    if let Some(r) = run.take()
                        && lines_in_run > 1
                    {
                        out.push(Fold { range: r, kind: FoldKind::Comment });
                    }
                    lines_in_run = 0;
                }
            }
        }
        if let Some(r) = run
            && lines_in_run > 1
        {
            out.push(Fold { range: r, kind: FoldKind::Comment });
        }
        out
    })
    .unwrap_or_default()
}
