//! Allowing lints with attributes in comments, like in Rust (see
//! [`papyrus_parser::attrs`]):
//!
//! ```papyrus
//! ; #![allow(unchecked_cast)]            ; the whole file
//!
//! ; #[allow(unused_local)]               ; the next declaration or statement
//! Function F()
//!     Int unused = 1
//!     Int other = 2  ; #[allow(all)]     ; this line
//! EndFunction
//! ```
//!
//! The arguments are lints (`unused_local` or `unused-local`), groups or
//! `all`; without any, every lint. Unknown names are reported (`unknown-lint`):
//! compiler errors such as `invalid_cast` cannot be allowed.
//!
//! Inside an attribute, completion offers the lints, groups and attributes.

use papyrus_ide::{
    Diagnostic, TextRange, TextSize,
    assists::{Assist, AssistKind, TextEdit},
};
use papyrus_parser::attrs::attributes;

use crate::{Group, LINTS, is_known_name};

/// The attributes, by path.
const ATTRIBUTES: &[(&str, &str)] = &[
    ("allow", "allow lints: `allow(unused_local, style)`"),
    ("fmt::skip", "keep the code as written by the formatter"),
];

/// Whether `names` (`None`: every lint) cover the lint `code`.
fn covers(names: Option<&[String]>, code: &str) -> bool {
    let Some(lint) = LINTS.iter().find(|l| l.code == code) else { return false };
    names.is_none_or(|names| {
        names.iter().any(|n| {
            n.eq_ignore_ascii_case(code)
                || n.eq_ignore_ascii_case(lint.group.name())
                || n.eq_ignore_ascii_case("all")
        })
    })
}

/// A lint name of an attribute, `unused_local` as `unused-local`.
fn lint_name(arg: &str) -> String {
    arg.replace('_', "-")
}

/// The lints allowed by the attributes of a file.
#[derive(Default)]
pub(crate) struct Ignores {
    /// The code `#[allow]` attributes apply to, with their lints (`None`:
    /// every lint).
    ranges: Vec<(TextRange, Option<Vec<String>>)>,
}

impl Ignores {
    pub(crate) fn of(root: papyrus_parser::SyntaxNode<'_>) -> Ignores {
        let ranges = attributes(root)
            .into_iter()
            .filter(|a| a.path.eq_ignore_ascii_case("allow"))
            .map(|a| {
                let names = (!a.args.is_empty()).then(|| {
                    a.args.iter().map(|n| lint_name(n)).filter(|n| is_known_name(n)).collect()
                });
                (a.target, names)
            })
            .collect();
        Ignores { ranges }
    }

    /// Whether the attributes allow `d`.
    pub(crate) fn silence(&self, d: &Diagnostic) -> bool {
        self.ranges.iter().any(|(range, names)| {
            range.contains_inclusive(d.range.start()) && covers(names.as_deref(), d.code)
        })
    }
}

/// The range of `part`, a slice of the file text `text`.
fn range_of(text: &str, part: &str) -> TextRange {
    let start = part.as_ptr() as usize - text.as_ptr() as usize;
    TextRange::at(TextSize::new(start as u32), TextSize::of(part))
}

/// Warnings about the attributes: unknown attributes and lints.
pub(crate) fn check_attributes(root: papyrus_parser::SyntaxNode<'_>) -> Vec<Diagnostic> {
    let text = root.text();
    let mut out = Vec::new();
    for a in attributes(root) {
        if !ATTRIBUTES.iter().any(|(p, _)| a.path.eq_ignore_ascii_case(p)) {
            let expected: Vec<String> = ATTRIBUTES.iter().map(|(p, _)| format!("`{p}`")).collect();
            out.push(Diagnostic::warning(
                range_of(text, a.path),
                "unknown-attribute",
                format!("unknown attribute `{}`: expected {}", a.path, expected.join(" or ")),
            ));
            continue;
        }
        if !a.path.eq_ignore_ascii_case("allow") {
            continue;
        }
        for arg in &a.args {
            let name = lint_name(arg);
            if is_known_name(&name) {
                continue;
            }
            let message = if papyrus_ide::COMPILER_CODES
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&name))
            {
                format!(
                    "`{arg}` is a compile error, not a lint: it cannot be allowed (the compiler rejects the script)"
                )
            } else {
                format!("unknown lint `{arg}`: see `papyrus_analyzer lint --list`")
            };
            out.push(Diagnostic::warning(range_of(text, arg), "unknown-lint", message));
        }
    }
    out
}

/// The hover of the attribute part at `offset` of `text`: the attribute (with
/// the lines it applies to) or the lint under the cursor.
pub fn attribute_hover(text: &str, offset: TextSize) -> Option<(TextRange, String)> {
    // Every hover comes here first: parse only for lines with an attribute.
    let at = usize::from(offset).min(text.len());
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
    if !text[line_start..line_end].contains("#[") && !text[line_start..line_end].contains("#![") {
        return None;
    }
    let parse = papyrus_parser::parse(text);
    let root = parse.syntax_node();
    let at = usize::from(offset);
    let comment = root.descendant_tokens().find(|t| {
        t.kind() == papyrus_parser::SyntaxKind::LINE_COMMENT
            && t.text_range().contains_inclusive(offset)
    })?;
    let attr = attributes(root).into_iter().find(|a| {
        let start = range_of(text, a.path).start();
        comment.text_range().contains(start)
    })?;
    let lines = |r: TextRange| {
        let line = |o: TextSize| text[..usize::from(o)].matches('\n').count() + 1;
        let (first, last) = (line(r.start()), line(r.end()));
        if first == last { format!("line {first}") } else { format!("lines {first}–{last}") }
    };
    let scope = if attr.inner {
        "the whole file".to_owned()
    } else {
        format!(
            "{} (`{}`)",
            lines(attr.target),
            text[attr.target].lines().next().unwrap_or("").trim()
        )
    };
    // A lint argument.
    for arg in &attr.args {
        let range = range_of(text, arg);
        if !range.contains_inclusive(offset) {
            continue;
        }
        let name = lint_name(arg);
        let doc = if let Some(lint) = LINTS.iter().find(|l| l.code.eq_ignore_ascii_case(&name)) {
            format!(
                "```papyrus\n{}\n```\n*lint · {} · {} by default*\n\n{}",
                lint.code.replace('-', "_"),
                lint.group.name(),
                lint.group.default_level(),
                lint.summary
            )
        } else if let Some(group) = Group::ALL.iter().find(|g| g.name().eq_ignore_ascii_case(&name))
        {
            let lints: Vec<String> = LINTS
                .iter()
                .filter(|l| l.group == *group)
                .map(|l| format!("`{}`", l.code.replace('-', "_")))
                .collect();
            format!(
                "```papyrus\n{}\n```\n*lint group · {} by default*\n\n{}",
                group.name(),
                group.default_level(),
                if lints.is_empty() { "No lints yet.".to_owned() } else { lints.join(" · ") }
            )
        } else if name.eq_ignore_ascii_case("all") {
            "```papyrus\nall\n```\nEvery lint.".to_owned()
        } else {
            return None;
        };
        return Some((range, doc));
    }
    // The attribute itself.
    let path_range = range_of(text, attr.path);
    let what = if attr.path.eq_ignore_ascii_case("allow") {
        "Allows the listed lints (lints, groups or `all`; none: every lint)."
    } else if attr.path.eq_ignore_ascii_case("fmt::skip") {
        "Keeps the code as written: the formatter leaves it alone."
    } else {
        return None;
    };
    let range = if path_range.contains_inclusive(offset) {
        path_range
    } else {
        // The brackets: the whole attribute.
        let start = text[..usize::from(path_range.start())].rfind('#')?;
        let end = text[at..].find(']').map(|i| at + i + 1)?;
        if at < start || end > usize::from(comment.text_range().end()) {
            return None;
        }
        TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32))
    };
    let syntax = if attr.inner { "#![...]" } else { "#[...]" };
    Some((
        range,
        format!(
            "```papyrus\n{}\n```\n*attribute `{syntax}`*\n\n{what}\n\nApplies to {scope}.",
            attr.path
        ),
    ))
}

/// Completion inside an attribute comment at `offset` of `text`: the range
/// to replace, and the items as `(label, detail)`.
pub fn attribute_completions(
    text: &str,
    offset: TextSize,
) -> Option<(TextRange, Vec<(String, String)>)> {
    let at = usize::from(offset).min(text.len());
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[line_start..at];
    let comment = &line[line.find(';')? + 1..];
    let body = comment.trim_start();
    let body = body.strip_prefix("#![").or_else(|| body.strip_prefix("#["))?;
    if body.contains(']') {
        return None;
    }
    // The word being typed.
    let word_len = body.len()
        - body
            .trim_end_matches(|c: char| {
                c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ':'
            })
            .len();
    let replace = TextRange::new(TextSize::new((at - word_len) as u32), offset);
    let before = &body[..body.len() - word_len];
    let items = if before.is_empty() {
        ATTRIBUTES.iter().map(|(p, d)| (p.to_string(), d.to_string())).collect()
    } else if before.trim_start().to_ascii_lowercase().starts_with("allow(")
        && before.trim_end().ends_with(['(', ','])
    {
        let mut items: Vec<(String, String)> = LINTS
            .iter()
            .map(|l| (l.code.replace('-', "_"), format!("{} · {}", l.group.name(), l.summary)))
            .collect();
        items.extend(Group::ALL.iter().map(|g| (g.name().to_owned(), "lint group".to_owned())));
        items.push(("all".to_owned(), "every lint".to_owned()));
        items
    } else {
        return None;
    };
    Some((replace, items))
}

/// Quick fixes adding an attribute allowing a lint problem: on its
/// statement, or in the whole file.
pub(crate) fn ignore_fixes(text: &str, d: &Diagnostic) -> Vec<Assist> {
    let start = usize::from(d.range.start());
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[line_start..];
    let indent = &line[..line.len() - line.trim_start().len()];
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let name = d.code.replace('-', "_");
    let at = |offset: usize| TextRange::empty(TextSize::new(offset as u32));
    vec![
        Assist {
            id: "allow_lint",
            label: format!("Allow `{name}` here"),
            kind: AssistKind::QuickFix,
            target: d.range,
            edits: vec![TextEdit {
                range: at(line_start),
                new_text: format!("{indent}; #[allow({name})]{newline}"),
            }],
            fixes: Some(d.code),
        },
        Assist {
            id: "allow_lint_file",
            label: format!("Allow `{name}` in this file"),
            kind: AssistKind::QuickFix,
            target: d.range,
            edits: vec![TextEdit {
                range: at(0),
                new_text: format!("; #![allow({name})]{newline}"),
            }],
            fixes: Some(d.code),
        },
    ]
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check;

    const BODY: &str = "Function F()\n    Int a = 1\n    Int b = 2  ; #[allow(unused_local)]\n    ; #[allow(suspicious)] because\n    Int c = 3\n    ; #[allow(bool-literal-comparison)]\n    Int d = 4\nEndFunction\n";

    #[test]
    fn outer_attributes_allow_their_item() {
        let src = format!("ScriptName T\n{BODY}");
        assert_eq!(check("unused-local", &src), ["3: `a` is never read", "8: `d` is never read"]);
        // Before a function: the whole function.
        let func = format!("ScriptName T\n; #[allow(unused_local)]\n{BODY}");
        assert_eq!(check("unused-local", &func), Vec::<String>::new());
    }

    #[test]
    fn reports_unknown_attributes_and_lints() {
        let src = "ScriptName T\n; #[allow(invalid_cast, unused_lokal, unused_local)]\n; #[alow]\nFunction F()\nEndFunction\n";
        let mut ws = papyrus_ide::Workspace::new();
        let file = ws.set_open(std::path::PathBuf::from("/ws/T.psc"), std::sync::Arc::from(src), 0);
        let found: Vec<(&str, String)> = crate::lint(&ws, file, &crate::LintConfig::default())
            .into_iter()
            .map(|d| (d.code, src[d.range].to_owned()))
            .collect();
        assert_eq!(
            found,
            [
                ("unknown-lint", "invalid_cast".into()),
                ("unknown-lint", "unused_lokal".into()),
                ("unknown-attribute", "alow".into())
            ]
        );
    }

    #[test]
    fn hovers_attributes_and_lints() {
        let src = "ScriptName T\n; #[allow(unused_local, style)] why\nFunction F()\nEndFunction\n";
        let hover = |needle: &str| {
            let offset = papyrus_ide::TextSize::new((src.find(needle).unwrap() + 1) as u32);
            super::attribute_hover(src, offset).map(|(r, md)| (src[r].to_owned(), md))
        };
        let (word, md) = hover("unused_local").unwrap();
        assert_eq!(word, "unused_local");
        assert!(md.contains("suspicious · warn by default"), "{md}");
        let (_, md) = hover("style").unwrap();
        assert!(md.contains("lint group"), "{md}");
        let (word, md) = hover("allow").unwrap();
        assert_eq!(word, "allow");
        assert!(md.contains("Applies to lines 3–4 (`Function F()`)"), "{md}");
        let (word, _) = hover("#[").unwrap();
        assert_eq!(word, "#[allow(unused_local, style)]");
        assert!(hover("why").is_none());
    }

    #[test]
    fn completes_attributes_and_lints() {
        let names = |src: &str| {
            let offset = papyrus_ide::TextSize::of(src);
            super::attribute_completions(src, offset)
                .map(|(_, items)| items.into_iter().map(|i| i.0).collect::<Vec<_>>())
        };
        assert_eq!(names("; #[").unwrap(), ["allow", "fmt::skip"]);
        let lints = names("Int x ; #[allow(unused_local, un").unwrap();
        assert!(
            lints.contains(&"unchecked_cast".to_owned()) && lints.contains(&"style".to_owned())
        );
        assert_eq!(names("; just a comment"), None);
        assert_eq!(names("; #[allow(x)] "), None);
    }

    #[test]
    fn inner_attributes_allow_the_file() {
        let src = format!("; #![allow(unused_local)]\nScriptName T\n{BODY}");
        assert_eq!(check("unused-local", &src), Vec::<String>::new());
        let all = format!("ScriptName T\n; #![allow(all)]\n{BODY}");
        assert_eq!(check("unused-local", &all), Vec::<String>::new());
        let other = format!("ScriptName T\n; #![allow(unreachable_code)]\n{BODY}");
        assert_eq!(check("unused-local", &other).len(), 2);
    }
}
