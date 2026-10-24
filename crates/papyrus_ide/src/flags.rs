//! User flags, defined in a flags file such as `TESV_Papyrus_Flags.flg`.
//!
//! The compiler reads the flags allowed after declarations (`Hidden`,
//! `Conditional`, ...) from the file given with `-f=`. Its format is:
//!
//! ```text
//! // comment
//! /* block comment */
//! Flag <name> <index>                        // allowed on all declarations
//! Flag <name> <index> { Script Property }    // allowed on the listed ones
//! ```
//!
//! where the targets are `Script`, `Property`, `Variable` and `Function`, and
//! the index is a bit position between 0 and 31.
//!
//! The keywords `Native`, `Global`, `Auto` and `AutoReadOnly` are part of the
//! language and are not defined here.

use std::{path::PathBuf, sync::Arc};

use papyrus_intern::Name;
use papyrus_parser::{SyntaxKind, TextRange, TextSize};

/// The declarations a flag may be attached to, as a bit set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FlagTargets(u8);

impl FlagTargets {
    /// The script header.
    pub const SCRIPT: FlagTargets = FlagTargets(1);
    /// Properties.
    pub const PROPERTY: FlagTargets = FlagTargets(1 << 1);
    /// Script variables.
    pub const VARIABLE: FlagTargets = FlagTargets(1 << 2);
    /// Functions and events.
    pub const FUNCTION: FlagTargets = FlagTargets(1 << 3);
    /// Every declaration.
    pub const ALL: FlagTargets = FlagTargets(0b1111);

    const NAMES: [(FlagTargets, &'static str); 4] = [
        (FlagTargets::SCRIPT, "Script"),
        (FlagTargets::PROPERTY, "Property"),
        (FlagTargets::VARIABLE, "Variable"),
        (FlagTargets::FUNCTION, "Function"),
    ];

    /// The targets of both sets.
    pub const fn union(self, other: FlagTargets) -> FlagTargets {
        FlagTargets(self.0 | other.0)
    }

    /// Returns `true` if any target of `other` is allowed.
    pub fn intersects(self, other: FlagTargets) -> bool {
        self.0 & other.0 != 0
    }

    /// Parses a target name, ignoring case.
    pub fn from_name(name: &str) -> Option<FlagTargets> {
        FlagTargets::NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|&(t, _)| t)
    }

    /// The names of the targets, e.g. `["Script", "Property"]`.
    pub fn names(self) -> Vec<&'static str> {
        FlagTargets::NAMES.iter().filter(|(t, _)| self.intersects(*t)).map(|&(_, n)| n).collect()
    }

    /// The target of a declaration node, `None` for nodes without flags.
    ///
    /// Auto properties are backed by a hidden variable and therefore accept
    /// variable flags too (`Int Property Foo Auto Conditional`).
    pub fn of_declaration(kind: SyntaxKind, auto_property: bool) -> Option<FlagTargets> {
        Some(match kind {
            SyntaxKind::SCRIPT_HEADER => FlagTargets::SCRIPT,
            SyntaxKind::PROPERTY if auto_property => {
                FlagTargets(FlagTargets::PROPERTY.0 | FlagTargets::VARIABLE.0)
            }
            SyntaxKind::PROPERTY => FlagTargets::PROPERTY,
            SyntaxKind::VAR_DECL => FlagTargets::VARIABLE,
            SyntaxKind::FUNCTION | SyntaxKind::EVENT => FlagTargets::FUNCTION,
            _ => return None,
        })
    }

    fn insert(&mut self, other: FlagTargets) {
        self.0 |= other.0;
    }
}

/// One `Flag` definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagDef {
    /// The flag name.
    pub name: Name,
    /// The bit index.
    pub index: u32,
    /// Where the flag is allowed.
    pub targets: FlagTargets,
    /// Range of the name in the flags file.
    pub name_range: TextRange,
    /// Range of the whole definition.
    pub full_range: TextRange,
    /// The comment lines directly above the definition.
    pub docs: String,
}

impl FlagDef {
    /// Renders the definition as flags file source.
    pub fn signature(&self) -> String {
        if self.targets == FlagTargets::ALL {
            format!("Flag {} {}", self.name, self.index)
        } else {
            format!("Flag {} {} {{ {} }}", self.name, self.index, self.targets.names().join(" "))
        }
    }
}

/// A problem in a flags file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagError {
    /// The location.
    pub range: TextRange,
    /// The message.
    pub message: String,
}

/// The flags known to the workspace.
#[derive(Clone, Debug)]
pub struct FlagTable {
    /// The file the flags come from; `None` for the built-in Skyrim defaults.
    pub path: Option<PathBuf>,
    /// The text of the file.
    pub text: Arc<str>,
    /// `true` while the file is open in the editor.
    pub open: bool,
    /// The definitions, in source order.
    pub defs: Vec<FlagDef>,
    /// Syntax errors.
    pub errors: Vec<FlagError>,
}

/// The content of `TESV_Papyrus_Flags.flg`, used when no flags file is known.
const SKYRIM_FLAGS: &str = "// Flag hides the script or property from the game editor\nFlag Hidden 0\n{\n\tScript\n\tProperty\n}\n\n// Flag on an object designates it as the script the condition system will look at\n// Flag on a variable allows the script variable to be examined by the condition system\nFlag Conditional 1\n{\n\tScript\n\tVariable\n}\n";

impl Default for FlagTable {
    fn default() -> Self {
        FlagTable::parse(None, Arc::from(SKYRIM_FLAGS))
    }
}

impl FlagTable {
    /// Parses a flags file.
    pub fn parse(path: Option<PathBuf>, text: Arc<str>) -> FlagTable {
        let (defs, errors) = parse_flags(&text);
        FlagTable { path, text, open: false, defs, errors }
    }

    /// Looks a flag up by name, ignoring case.
    pub fn get(&self, name: &str) -> Option<(usize, &FlagDef)> {
        self.defs.iter().enumerate().find(|(_, d)| d.name.as_str().eq_ignore_ascii_case(name))
    }

    /// The definition whose name contains `offset`.
    pub fn def_at(&self, offset: TextSize) -> Option<usize> {
        self.defs.iter().position(|d| d.name_range.contains_inclusive(offset))
    }

    /// The display name of the source, e.g. `TESV_Papyrus_Flags.flg`.
    pub fn source_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "built-in Skyrim flags".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok {
    Word,
    Number,
    LBrace,
    RBrace,
    Comment,
    Unknown,
}

fn lex(text: &str) -> Vec<(Tok, TextRange)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let tok = match bytes[i] {
            b if b.is_ascii_whitespace() => {
                i += 1;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                Tok::Comment
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = text[i + 2..].find("*/").map_or(bytes.len(), |e| i + 2 + e + 2);
                Tok::Comment
            }
            b'{' => {
                i += 1;
                Tok::LBrace
            }
            b'}' => {
                i += 1;
                Tok::RBrace
            }
            b'0'..=b'9' => {
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                Tok::Number
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                Tok::Word
            }
            _ => {
                i += text[i..].chars().next().map_or(1, char::len_utf8);
                Tok::Unknown
            }
        };
        out.push((tok, TextRange::new(TextSize::new(start as u32), TextSize::new(i as u32))));
    }
    out
}

/// Parses the definitions of a flags file, recovering from errors.
pub fn parse_flags(text: &str) -> (Vec<FlagDef>, Vec<FlagError>) {
    let tokens = lex(text);
    let mut defs: Vec<FlagDef> = Vec::new();
    let mut errors = Vec::new();
    let mut docs: Vec<TextRange> = Vec::new();
    let mut i = 0;
    let err = |errors: &mut Vec<FlagError>, range, message: &str| {
        errors.push(FlagError { range, message: message.to_owned() });
    };

    while i < tokens.len() {
        let (tok, range) = tokens[i];
        let word = &text[range];
        match tok {
            Tok::Comment => {
                // Only comments directly above a definition document it.
                let gap = docs.last().map_or(range.start(), |r| r.end());
                if text[usize::from(gap)..usize::from(range.start())].matches('\n').count() > 1 {
                    docs.clear();
                }
                docs.push(range);
                i += 1;
                continue;
            }
            Tok::Word if word.eq_ignore_ascii_case("Flag") => {}
            _ => {
                err(&mut errors, range, "expected `Flag <name> <index>`");
                docs.clear();
                i += 1;
                continue;
            }
        }

        let start = range.start();
        let doc_text = match docs.last() {
            Some(last)
                if text[usize::from(last.end())..usize::from(start)].matches('\n').count() <= 1 =>
            {
                render_comments(text, &docs)
            }
            _ => String::new(),
        };
        docs.clear();
        i += 1;

        let Some(&(Tok::Word, name_range)) = tokens.get(i) else {
            err(&mut errors, TextRange::empty(range.end()), "expected a flag name");
            continue;
        };
        i += 1;
        let index = match tokens.get(i) {
            Some(&(Tok::Number, r)) => {
                i += 1;
                let value: u32 = text[r].parse().unwrap_or(u32::MAX);
                if value > 31 {
                    err(&mut errors, r, "flag index must be between 0 and 31");
                }
                value
            }
            _ => {
                err(&mut errors, TextRange::empty(name_range.end()), "expected a flag index");
                continue;
            }
        };

        let mut targets = FlagTargets::default();
        let mut end = tokens[i - 1].1.end();
        if let Some(&(Tok::LBrace, _)) = tokens.get(i) {
            i += 1;
            loop {
                match tokens.get(i) {
                    Some(&(Tok::RBrace, r)) => {
                        end = r.end();
                        i += 1;
                        break;
                    }
                    Some(&(Tok::Word, r)) if !text[r].eq_ignore_ascii_case("Flag") => {
                        match FlagTargets::from_name(&text[r]) {
                            Some(t) => targets.insert(t),
                            None => err(
                                &mut errors,
                                r,
                                "unknown target, expected `Script`, `Property`, `Variable` or `Function`",
                            ),
                        }
                        end = r.end();
                        i += 1;
                    }
                    Some(&(Tok::Comment, _)) => i += 1,
                    _ => {
                        err(&mut errors, TextRange::empty(end), "expected `}`");
                        break;
                    }
                }
            }
        } else {
            targets = FlagTargets::ALL;
        }

        let name = Name::new(&text[name_range]);
        if defs.iter().any(|d| d.name == name) {
            let message = format!("flag `{name}` is already defined");
            err(&mut errors, name_range, &message);
        } else if let Some(other) = defs.iter().find(|d| d.index == index) {
            let message = format!("index {index} is already used by flag `{}`", other.name);
            err(&mut errors, name_range, &message);
        }
        defs.push(FlagDef {
            name,
            index,
            targets,
            name_range,
            full_range: TextRange::new(start, end),
            docs: doc_text,
        });
    }
    (defs, errors)
}

fn render_comments(text: &str, ranges: &[TextRange]) -> String {
    let mut out = String::new();
    for &r in ranges {
        let raw = &text[r];
        let body = match raw.strip_prefix("/*") {
            Some(inner) => inner.strip_suffix("*/").unwrap_or(inner),
            None => raw.trim_start_matches('/'),
        };
        for line in body.lines() {
            let line = line.trim();
            if !line.is_empty() {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skyrim_flags() {
        let table = FlagTable::default();
        let sigs: Vec<_> = table.defs.iter().map(|d| d.signature()).collect();
        assert_eq!(
            sigs,
            ["Flag Hidden 0 { Script Property }", "Flag Conditional 1 { Script Variable }"]
        );
        assert_eq!(table.defs[0].docs, "Flag hides the script or property from the game editor");
        assert!(table.defs[1].docs.contains("condition system will look at\n"));
        assert!(table.errors.is_empty());
    }

    #[test]
    fn all_targets_and_errors() {
        let (defs, errors) = parse_flags(
            "/* header\n*/\n\nFlag A 0\nFlag B 40 { Script Bogus }\nFlag A 2\nFlag C\n",
        );
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].targets, FlagTargets::ALL);
        assert_eq!(defs[0].docs, "", "a blank line separates the header comment");
        let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "flag index must be between 0 and 31",
                "unknown target, expected `Script`, `Property`, `Variable` or `Function`",
                "flag `A` is already defined",
                "expected a flag index",
            ]
        );
    }
}
