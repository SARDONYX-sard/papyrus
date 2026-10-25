//! Code snippets offered by completion: `if`, `while`, `function`, ...
//!
//! Each snippet is a file of the `snippets` directory, written as Papyrus
//! with LSP placeholders (`${1:name}`, `$0` for the final cursor) and a
//! header of comment lines:
//!
//! ```text
//! ; Run a block when a condition holds.     <- the description
//! ; prefix: if                              <- what to type
//! ; context: body                           <- body (statements) or item (declarations)
//! If ${1:condition}
//!     $0
//! EndIf
//! ```
//!
//! Tabs indent; editors convert them to the indentation of the file. To add
//! a snippet, add a file and its name to [`SNIPPETS`]. The tests check every
//! file parses once the placeholders are filled.

/// Where a snippet is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SnippetContext {
    /// Inside a function or event: statements.
    Body,
    /// At script level or in a state: declarations.
    Item,
}

/// One snippet.
pub(crate) struct Snippet {
    /// What to type, the completion label.
    pub(crate) prefix: &'static str,
    /// The description.
    pub(crate) description: &'static str,
    /// Where it is offered.
    pub(crate) context: SnippetContext,
    /// The text, in LSP snippet syntax.
    pub(crate) body: &'static str,
}

macro_rules! snippet_files {
    ($($file:literal),* $(,)?) => {
        &[$(include_str!(concat!("snippets/", $file))),*]
    };
}

/// The snippet files, in completion order.
const SNIPPETS: &[&str] = snippet_files![
    "if.psc",
    "ifelse.psc",
    "while.psc",
    "for.psc",
    "foreach.psc",
    "trace.psc",
    "function.psc",
    "funcret.psc",
    "event.psc",
    "property.psc",
    "autoproperty.psc",
    "state.psc",
    "autostate.psc",
    "onupdate.psc",
];

/// Parses a snippet file.
fn parse(file: &'static str) -> Option<Snippet> {
    let mut description = "";
    let mut prefix = None;
    let mut context = None;
    let mut body_start = 0;
    for line in file.split_inclusive('\n') {
        let Some(comment) = line.strip_prefix(';') else { break };
        body_start += line.len();
        let comment = comment.trim();
        if let Some(p) = comment.strip_prefix("prefix:") {
            prefix = Some(p.trim());
        } else if let Some(c) = comment.strip_prefix("context:") {
            context = match c.trim() {
                "body" => Some(SnippetContext::Body),
                "item" => Some(SnippetContext::Item),
                _ => None,
            };
        } else if description.is_empty() {
            description = comment;
        }
    }
    let body = file[body_start..].trim_end_matches(['\r', '\n']);
    Some(Snippet { prefix: prefix?, description, context: context?, body })
}

/// The snippets offered in `context`.
pub(crate) fn snippets(context: SnippetContext) -> impl Iterator<Item = Snippet> {
    SNIPPETS.iter().filter_map(|f| parse(f)).filter(move |s| s.context == context)
}

/// A snippet as it reads once inserted: placeholders replaced by their
/// default text, tabs by four spaces.
pub(crate) fn preview(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        if let Some(inner) = rest.strip_prefix('{')
            && let Some(end) = inner.find('}')
        {
            out.push_str(inner[..end].split_once(':').map_or("", |(_, v)| v));
            rest = &inner[end + 1..];
        } else {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        }
    }
    out.push_str(rest);
    out.replace('\t', "    ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_snippet_is_valid_papyrus() {
        assert_eq!(SNIPPETS.iter().filter_map(|f| parse(f)).count(), SNIPPETS.len());
        for file in SNIPPETS {
            let s = parse(file).unwrap();
            let code = preview(s.body);
            let src = match s.context {
                SnippetContext::Item => format!("ScriptName T\n{code}\n"),
                SnippetContext::Body => format!(
                    "ScriptName T\nFunction F(Bool condition, Int count, Form[] array)\n{code}\nEndFunction\n"
                ),
            };
            let parse = papyrus_parser::parse(&src);
            assert!(parse.errors().is_empty(), "{}: {:?}\n{src}", s.prefix, parse.errors());
        }
    }
}
