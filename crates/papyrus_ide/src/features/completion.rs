//! Code completion.
//!
//! The context is decided from the tokens left of the cursor:
//!
//! - after `receiver.`: the members of the receiver type;
//! - after `extends`, `Import`, `As` or `New`: script names;
//! - after `Event` or `Function` at script level: overridable declarations of
//!   the parent scripts, inserted with their full signature;
//! - inside a body: locals, members, imported global functions, scripts and
//!   statement keywords;
//! - at script level: declaration keywords, types and flags.

use papyrus_intern::Name;
use papyrus_parser::{
    SyntaxKind, SyntaxToken, TextRange, TextSize,
    ast::{self, AstNode},
};

use super::{in_body, member_kind_label, with_sema};
use crate::{
    FileId, Workspace,
    index::{Member, MemberKind, render_param},
    sema::{FileSource, LocalKind, MemberItem, Sema, Ty},
};

/// The kind of a completion item, mapped to an icon by editors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    /// A keyword.
    Keyword,
    /// A function.
    Function,
    /// An event.
    Event,
    /// A property.
    Property,
    /// A script variable.
    Variable,
    /// A local variable or parameter.
    Local,
    /// A script (type).
    Script,
    /// A primitive type.
    Type,
    /// A declaration flag.
    Flag,
    /// A state.
    State,
    /// A code snippet.
    Snippet,
}

/// A completion proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    /// The displayed text, also inserted when `snippet` is `None`.
    pub label: String,
    /// The kind.
    pub kind: CompletionKind,
    /// A one line description, e.g. the signature.
    pub detail: Option<String>,
    /// Markdown documentation.
    pub documentation: Option<String>,
    /// Text in LSP snippet syntax (`$1`, `$0`), to insert instead of `label`.
    pub snippet: Option<String>,
    /// The range replaced by the completion.
    pub replace_range: TextRange,
    /// Sort key; lower sorts first.
    pub sort_text: String,
    /// A member whose documentation can be loaded lazily with [`member_docs`].
    pub resolve: Option<(FileId, u32)>,
}

/// The result of [`completions`].
#[derive(Clone, Debug, Default)]
pub struct Completions {
    /// The proposals.
    pub items: Vec<CompletionItem>,
    /// `true` if typing more characters may yield new proposals.
    pub is_incomplete: bool,
}

/// Maximum number of script names proposed at once.
const MAX_SCRIPTS: usize = 300;

const STATEMENT_KEYWORDS: &[&str] = &[
    "If", "ElseIf", "Else", "EndIf", "While", "EndWhile", "Return", "New", "As", "True", "False",
    "None", "Self", "Parent",
];
const DECL_KEYWORDS: &[&str] = &[
    "Function",
    "EndFunction",
    "Event",
    "EndEvent",
    "Property",
    "EndProperty",
    "State",
    "EndState",
    "Auto",
    "Import",
    "ScriptName",
    "Extends",
];
const PRIMITIVES: &[&str] = &["Bool", "Int", "Float", "String"];

/// Completion proposals at `offset`.
pub fn completions(ws: &Workspace, file: FileId, offset: TextSize) -> Completions {
    with_sema(ws, file, |sema| Ctx::new(sema, offset).complete()).unwrap_or_default()
}

/// Loads the documentation of a member, for lazily resolved items.
pub fn member_docs(ws: &Workspace, file: FileId, index: u32) -> Option<String> {
    let member = ws.file(file).index.members.get(index as usize)?;
    let src = FileSource::load(ws, file)?;
    let docs = crate::index::render_docs(src.text(), &member.docs);
    (!docs.is_empty()).then_some(docs)
}

struct Ctx<'s, 'a> {
    sema: &'s Sema<'a>,
    offset: TextSize,
    replace_range: TextRange,
    prefix: String,
    /// The significant token before the word being completed.
    prev: Option<SyntaxToken<'a>>,
    out: Completions,
}

impl<'s, 'a> Ctx<'s, 'a> {
    fn new(sema: &'s Sema<'a>, offset: TextSize) -> Ctx<'s, 'a> {
        let left = sema.root.token_at_offset(offset).left_biased();
        let word = left.filter(|t| {
            (t.kind() == SyntaxKind::IDENT || t.kind().is_keyword())
                && t.text_range().start() < offset
        });
        let (replace_range, prefix, before) = match word {
            Some(w) => {
                let range = TextRange::new(w.text_range().start(), offset);
                (w.text_range(), sema.text[range].to_owned(), w.prev_token())
            }
            None => (TextRange::empty(offset), String::new(), left),
        };
        let mut prev = before;
        while let Some(t) = prev {
            if t.kind() == SyntaxKind::WHITESPACE || t.kind().is_comment() {
                prev = t.prev_token();
            } else {
                break;
            }
        }
        // `left` may be the newline right after the word when offset == end.
        Ctx { sema, offset, replace_range, prefix, prev, out: Completions::default() }
    }

    fn complete(mut self) -> Completions {
        let prev_kind = self.prev.map(|t| t.kind());
        if self.in_comment_or_string() {
            return self.out;
        }
        match prev_kind {
            Some(SyntaxKind::DOT) => self.complete_members(),
            Some(SyntaxKind::EXTENDS_KW | SyntaxKind::IMPORT_KW) => self.add_scripts(false),
            Some(SyntaxKind::AS_KW | SyntaxKind::NEW_KW) => {
                self.add_primitives();
                self.add_scripts(false);
            }
            Some(SyntaxKind::EVENT_KW) if !self.in_body() => self.add_overrides(MemberKind::Event),
            Some(SyntaxKind::FUNCTION_KW) if !self.in_body() => {
                self.add_overrides(MemberKind::Function);
            }
            Some(SyntaxKind::R_PAREN) if self.on_declaration_header() => self.add_flags(),
            _ if self.in_body() => self.complete_body(),
            _ => self.complete_item(),
        }
        self.out
    }

    fn in_comment_or_string(&self) -> bool {
        let Some(tok) = self.sema.root.token_at_offset(self.offset).left_biased() else {
            return false;
        };
        let inside = tok.text_range().start() < self.offset && self.offset < tok.text_range().end();
        (tok.kind().is_comment() || tok.kind() == SyntaxKind::STRING) && inside
            || tok.kind() == SyntaxKind::LINE_COMMENT && self.offset == tok.text_range().end()
    }

    fn anchor(&self) -> Option<papyrus_parser::SyntaxNode<'a>> {
        let tok = self.sema.root.token_at_offset(self.offset).left_biased()?;
        tok.parent()
    }

    fn in_body(&self) -> bool {
        self.anchor().is_some_and(in_body)
    }

    fn on_declaration_header(&self) -> bool {
        self.prev.and_then(|t| t.parent()).is_some_and(|p| p.kind() == SyntaxKind::PARAM_LIST)
    }

    fn push(
        &mut self,
        label: impl Into<String>,
        kind: CompletionKind,
        sort: u8,
    ) -> &mut CompletionItem {
        let label = label.into();
        let sort_text = format!("{sort}{}", label.to_ascii_lowercase());
        self.out.items.push(CompletionItem {
            label,
            kind,
            detail: None,
            documentation: None,
            snippet: None,
            replace_range: self.replace_range,
            sort_text,
            resolve: None,
        });
        self.out.items.last_mut().unwrap()
    }

    fn push_member(&mut self, file: FileId, index: u32, m: &Member, sort: u8) {
        let kind = match m.kind {
            MemberKind::Function => CompletionKind::Function,
            MemberKind::Event => CompletionKind::Event,
            MemberKind::Property => CompletionKind::Property,
            MemberKind::Variable => CompletionKind::Variable,
            MemberKind::State => CompletionKind::State,
        };
        let snippet = m.is_callable().then(|| call_snippet(m.name.as_str(), !m.params.is_empty()));
        let item = self.push(m.name.as_str(), kind, sort);
        item.detail = Some(m.signature());
        item.snippet = snippet;
        item.resolve = (!m.docs.is_empty()).then_some((file, index));
    }

    fn complete_members(&mut self) {
        let Some(dot) = self.prev else { return };
        let Some(field) = dot.parent().and_then(ast::FieldExpr::cast) else { return };
        let Some(recv) = field.receiver().and_then(|r| self.sema.infer(r)) else { return };
        for item in self.sema.members_of(recv) {
            match item {
                MemberItem::Member(file, index, m) => self.push_member(file, index, m, 0),
                MemberItem::Builtin(b, elem) => {
                    let kind = if b.is_callable() {
                        CompletionKind::Function
                    } else {
                        CompletionKind::Property
                    };
                    let snippet =
                        b.is_callable().then(|| call_snippet(b.name(), !b.params(elem).is_empty()));
                    let item = self.push(b.name(), kind, 1);
                    item.detail = Some(b.signature(elem));
                    item.documentation = Some(b.docs().to_owned());
                    item.snippet = snippet;
                }
            }
        }
    }

    fn complete_body(&mut self) {
        let sema = self.sema;
        if let Some(anchor) = self.anchor() {
            let mut seen: Vec<Name> = Vec::new();
            for (name, _, ty, kind) in sema.locals_at(anchor, self.offset).into_iter().rev() {
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name);
                let item = self.push(name.as_str(), CompletionKind::Local, 0);
                item.detail = Some(match (ty, kind) {
                    (Some(ty), LocalKind::Parameter) => format!("{ty} {name} (parameter)"),
                    (Some(ty), LocalKind::Variable) => format!("{ty} {name}"),
                    (None, _) => name.to_string(),
                });
            }
        }

        if let Some(script) = sema.script_name() {
            let ty =
                Ty::Value(crate::index::TypeRef::scalar(crate::index::BaseType::Object(script)));
            for item in sema.members_of(ty) {
                match item {
                    MemberItem::Member(file, index, m) => self.push_member(file, index, m, 1),
                    MemberItem::Builtin(b, _) => {
                        let item = self.push(b.name(), CompletionKind::Function, 3);
                        item.detail = Some(b.signature(None));
                        item.snippet = Some(call_snippet(b.name(), !b.params(None).is_empty()));
                    }
                }
            }
            // Global functions of the own script are not part of `members_of`.
            for (file, index) in sema.chain(script).take(1) {
                for (i, m) in index.empty_state_members() {
                    if m.is_callable() && m.is_global() {
                        self.push_member(file, i as u32, m, 1);
                    }
                }
            }
        }

        for &import in sema.index.imports.iter() {
            let Some(file) = sema.ws.script_near(import, sema.file) else { continue };
            let index = &sema.ws.file(file).index;
            for (i, m) in index.empty_state_members() {
                if m.is_callable() && m.is_global() {
                    self.push_member(file, i as u32, m, 2);
                }
            }
        }

        for kw in STATEMENT_KEYWORDS {
            self.push(*kw, CompletionKind::Keyword, 4);
        }
        self.add_primitives();
        self.add_scripts(true);
    }

    fn complete_item(&mut self) {
        for kw in DECL_KEYWORDS {
            self.push(*kw, CompletionKind::Keyword, 1);
        }
        self.add_primitives();
        if self.prev.is_some_and(|t| t.kind() != SyntaxKind::NEWLINE) {
            self.add_flags();
        }
        self.add_scripts(true);
    }

    fn add_primitives(&mut self) {
        for ty in PRIMITIVES {
            self.push(*ty, CompletionKind::Type, 2);
        }
    }

    fn add_flags(&mut self) {
        for flag in ["Global", "Native", "Auto", "AutoReadOnly", "Hidden", "Conditional"] {
            self.push(flag, CompletionKind::Flag, 0);
        }
    }

    fn add_scripts(&mut self, low_priority: bool) {
        let first = self.prefix.chars().next().map(|c| c.to_ascii_lowercase());
        let mut matching: Vec<_> = self
            .sema
            .ws
            .all_scripts()
            .filter(|(name, _)| {
                first.is_none_or(|c| {
                    name.as_str().chars().next().map(|f| f.to_ascii_lowercase()) == Some(c)
                })
            })
            .collect();
        if matching.len() > MAX_SCRIPTS {
            // Prefer names that start with the whole prefix.
            let prefix = self.prefix.to_ascii_lowercase();
            matching.sort_by_key(|(name, _)| {
                (!name.as_str().to_ascii_lowercase().starts_with(&prefix), name.as_str().len())
            });
            matching.truncate(MAX_SCRIPTS);
            self.out.is_incomplete = true;
        }
        let sort = if low_priority { 5 } else { 0 };
        for (name, file) in matching {
            let detail = self.sema.ws.file(file).index.extends.map(|e| format!("extends {e}"));
            let item = self.push(name.as_str(), CompletionKind::Script, sort);
            item.detail = detail;
        }
    }

    /// Events and functions of the parent scripts, inserted as a full
    /// declaration with body.
    fn add_overrides(&mut self, kind: MemberKind) {
        let sema = self.sema;
        let Some(parent) = sema.index.extends else { return };
        let mut seen: Vec<Name> =
            sema.index.members.iter().filter(|m| m.kind == kind).map(|m| m.name).collect();
        let end = if kind == MemberKind::Event { "EndEvent" } else { "EndFunction" };
        for (file, index) in sema.chain(parent) {
            for (i, m) in index.empty_state_members() {
                if m.kind != kind || seen.contains(&m.name) || m.is_global() {
                    continue;
                }
                seen.push(m.name);
                let mut params = String::new();
                for (j, p) in m.params.iter().enumerate() {
                    if j > 0 {
                        params.push_str(", ");
                    }
                    render_param(&mut params, p);
                }
                // `Native` declarations of the parent are overridden by a
                // normal declaration with a body.
                let snippet = format!("{}({})\n\t$0\n{end}", m.name, escape_snippet(&params));
                let label = format!("{}({params})", m.name);
                let kind = if kind == MemberKind::Event {
                    CompletionKind::Event
                } else {
                    CompletionKind::Function
                };
                let item = self.push(label, kind, 0);
                item.detail =
                    Some(format!("{} of {}", member_kind_label(m.kind), index.name_str()));
                item.snippet = Some(snippet);
                item.resolve = (!m.docs.is_empty()).then_some((file, i as u32));
            }
        }
    }
}

fn call_snippet(name: &str, has_params: bool) -> String {
    if has_params { format!("{name}($1)$0") } else { format!("{name}()$0") }
}

fn escape_snippet(s: &str) -> String {
    s.replace('\\', "\\\\").replace('$', "\\$").replace('}', "\\}")
}
