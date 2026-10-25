//! Attributes in comments, like Rust attributes:
//!
//! ```papyrus
//! ; #![allow(unchecked_cast)]          ; inner: the whole file
//!
//! ; #[allow(unused_local)]             ; outer: the next declaration or statement
//! Function F()
//!     Int unused = 1
//! EndFunction
//!
//! Int x = 1   ; #[fmt::skip]           ; after code: this line
//! ```
//!
//! An outer attribute alone on its line applies to the declaration or
//! statement starting on the next line, all its lines: a whole `Function`,
//! `State` or `If`. After code, it applies to the line of the code. Text
//! after the closing `]` (a reason) is ignored.

use crate::{SyntaxKind, SyntaxNode, TextRange, TextSize};

/// An attribute of a comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute<'a> {
    /// `#![...]`: the whole file.
    pub inner: bool,
    /// The path: `allow`, `fmt::skip`.
    pub path: &'a str,
    /// The arguments between parentheses: `unused_local`.
    pub args: Vec<&'a str>,
    /// The code it applies to: the whole file for inner attributes.
    pub target: TextRange,
}

/// The attributes of the comments of a file.
pub fn attributes<'a>(root: SyntaxNode<'a>) -> Vec<Attribute<'a>> {
    let text = root.text();
    root.descendant_tokens()
        .filter(|t| t.kind() == SyntaxKind::LINE_COMMENT)
        .filter_map(|tok| {
            let (inner, path, args) = parse(tok.text())?;
            let target = if inner {
                root.text_range()
            } else {
                let start = usize::from(tok.text_range().start());
                let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
                if text[line_start..start].trim().is_empty() {
                    next_item(root, tok.text_range().end())?
                } else {
                    // After code: its line.
                    let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
                    TextRange::new(TextSize::new(line_start as u32), TextSize::new(line_end as u32))
                }
            };
            Some(Attribute { inner, path, args, target })
        })
        .collect()
}

/// The canonical spelling of an attribute comment, for the formatter:
/// `;  #[ allow( unused-local ,x ) ]  why` → `; #[allow(unused_local, x)] why`.
/// `None` for other comments.
pub fn normalize(comment: &str) -> Option<String> {
    let (inner, path, args) = parse(comment)?;
    let rest = comment[comment.find(']')? + 1..].trim();
    let mut out = format!("; #{}[{path}", if inner { "!" } else { "" });
    if comment[..comment.find(']')?].contains('(') {
        let args: Vec<String> = args.iter().map(|a| a.replace('-', "_")).collect();
        out.push_str(&format!("({})", args.join(", ")));
    }
    out.push(']');
    if !rest.is_empty() {
        out.push(' ');
        out.push_str(rest);
    }
    Some(out)
}

/// `; #[path(args)]` or `; #![path(args)]`.
fn parse(comment: &str) -> Option<(bool, &str, Vec<&str>)> {
    let rest = comment.strip_prefix(';')?.trim_start();
    let (inner, rest) = match rest.strip_prefix("#![") {
        Some(r) => (true, r),
        None => (false, rest.strip_prefix("#[")?),
    };
    let body = rest[..rest.find(']')?].trim();
    let (path, args) = match body.find('(') {
        Some(open) => {
            let args = body[open + 1..].strip_suffix(')')?;
            let args = args.split(',').map(str::trim).filter(|a| !a.is_empty()).collect();
            (body[..open].trim(), args)
        }
        None => (body, Vec::new()),
    };
    Some((inner, path, args))
}

/// The declaration or statement starting at the first code after `offset`.
fn next_item(root: SyntaxNode<'_>, offset: TextSize) -> Option<TextRange> {
    let tok = root.descendant_tokens().find(|t| {
        t.text_range().start() >= offset && !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE
    })?;
    let start = tok.text_range().start();
    // The outermost node starting there, below the file and the blocks.
    let node = tok
        .parent()?
        .ancestors()
        .take_while(|n| !matches!(n.kind(), SyntaxKind::SOURCE_FILE | SyntaxKind::BLOCK))
        .filter(|n| n.text_range().start() == start)
        .last()
        .or_else(|| tok.parent())?;
    Some(node.trimmed_range())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(src: &str) -> Vec<(bool, String, Vec<String>, String)> {
        let parse = crate::parse(src);
        attributes(parse.syntax_node())
            .into_iter()
            .map(|a| {
                (
                    a.inner,
                    a.path.to_owned(),
                    a.args.iter().map(|s| s.to_string()).collect(),
                    src[a.target].lines().next().unwrap_or("").to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn inner_outer_and_trailing() {
        let src = "; #![allow(unchecked_cast)]\nScriptName T\n; #[allow(unused_local, all)] kept\nFunction F()\n    Int x = 1 ; #[fmt::skip]\nEndFunction\n";
        let found = attrs(src);
        assert!(found[0].0);
        assert_eq!(
            found[1],
            (
                false,
                "allow".into(),
                vec!["unused_local".into(), "all".into()],
                "Function F()".into()
            )
        );
        assert_eq!(
            found[2],
            (false, "fmt::skip".into(), vec![], "    Int x = 1 ; #[fmt::skip]".into())
        );
        // The function, all its lines.
        let parse = crate::parse(src);
        let f = &attributes(parse.syntax_node())[1];
        assert!(src[f.target].ends_with("EndFunction"));
    }
}
