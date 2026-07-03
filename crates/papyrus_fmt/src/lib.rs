//! Papyrus source formatter.

use papyrus_cst::ast;
use papyrus_cst::{
    ast::{AstCast, AstNode, Block, Event, File, Function, ReturnStmt, Script, Statement, support},
    cst::{Child, Tree, TreeKind},
};
use papyrus_token::keyword::KeywordCase;
use papyrus_token::token::{Token, TokenKind, TriviaKind};

// ── Public API ────────────────────────────────────────────────────────────────

pub fn format(src: &str, tree: &Tree, options: &FormatOptions) -> String {
    let Some(file) = File::cast(tree) else {
        return src.to_string();
    };
    let mut f = Formatter::new(src, options);
    f.file(file);
    f.finish()
}

// ── Options ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    Spaces(u8),
    Tabs,
}

impl Default for Indent {
    fn default() -> Self {
        Indent::Spaces(4)
    }
}

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub indent: Indent,
    pub keyword_case: KeywordCase,
    pub blank_lines_between_decls: u32,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent: Indent::Spaces(4),
            keyword_case: KeywordCase::Pascal,
            blank_lines_between_decls: 1,
        }
    }
}

// ── Formatter ─────────────────────────────────────────────────────────────────

struct Formatter<'a> {
    src: &'a str,
    options: &'a FormatOptions,
    out: String,
    depth: usize,
    at_line_start: bool,
    prev_kind: Option<TokenKind>,
}

impl<'a> Formatter<'a> {
    fn new(src: &'a str, options: &'a FormatOptions) -> Self {
        Self {
            src,
            options,
            out: String::new(),
            depth: 0,
            at_line_start: true,
            prev_kind: None,
        }
    }

    fn finish(mut self) -> String {
        while self.out.ends_with("\n\n") {
            self.out.pop();
        }
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out
    }

    // ── AST-level formatters ──────────────────────────────────────────────────

    fn file(&mut self, file: File<'a>) {
        if let Some(script) = file.script() {
            self.script(script);
        }
    }

    fn script(&mut self, script: Script<'a>) {
        let tree = script.syntax();
        let is_body = |k: TreeKind| {
            matches!(
                k,
                TreeKind::Function | TreeKind::Event | TreeKind::Property | TreeKind::State
            )
        };

        // Header: emit tokens/subtrees up to the first body declaration.
        for child in &tree.children {
            match child {
                Child::Tree(t) if is_body(t.kind) => break,
                Child::Tree(t) => self.emit_tree(t),
                Child::Token(t) => self.emit_token(t),
            }
        }
        self.newline();

        // One blank line between the header and the first body declaration.
        let has_body = tree
            .children
            .iter()
            .any(|c| matches!(c, Child::Tree(t) if is_body(t.kind)));
        if has_body {
            self.out.push('\n');
        }

        // Body declarations separated by blank lines.
        let mut first = true;
        for child in &tree.children {
            if let Child::Tree(t) = child
                && is_body(t.kind)
            {
                if !first {
                    self.blank_lines(self.options.blank_lines_between_decls);
                }
                self.emit_decl(t);
                first = false;
            }
        }
    }

    fn emit_decl(&mut self, tree: &'a Tree) {
        match tree.kind {
            TreeKind::Function => {
                if let Some(f) = Function::cast(tree) {
                    self.function(f);
                } else {
                    self.emit_tree(tree);
                }
            }
            TreeKind::Event => {
                if let Some(e) = Event::cast(tree) {
                    self.event(e);
                } else {
                    self.emit_tree(tree);
                }
            }
            TreeKind::Property | TreeKind::State => {
                self.emit_tree(tree);
                self.newline();
            }
            _ => self.emit_tree(tree),
        }
    }

    fn function(&mut self, func: Function<'a>) {
        self.write_indent();
        self.emit_until_block(func.syntax());
        self.newline();

        if let Some(block) = func.body() {
            self.block(block);
        }

        self.write_indent();
        self.emit_closing_token(func.syntax(), TokenKind::EndFunction);
        self.newline();
    }

    fn event(&mut self, ev: Event<'a>) {
        self.write_indent();
        self.emit_until_block(ev.syntax());
        self.newline();

        if let Some(block) = ev.body() {
            self.block(block);
        }

        self.write_indent();
        self.emit_closing_token(ev.syntax(), TokenKind::EndEvent);
        self.newline();
    }

    fn block(&mut self, block: Block<'a>) {
        self.depth += 1;
        for stmt in block.statements() {
            // Do NOT call write_indent() here: emit_token() handles indentation
            // for the first token on each line (including after leading comments).
            self.statement(stmt);
            self.newline();
        }
        self.depth -= 1;
    }

    fn statement(&mut self, stmt: Statement<'a>) {
        let tree = stmt.syntax();
        self.emit_stmt_tree(tree);
    }

    fn return_stmt(&mut self, stmt: ReturnStmt<'_>) {
        if let Some(tok) = support::token(stmt.syntax(), TokenKind::Return) {
            self.emit_token(tok);
        }
        if let Some(expr) = stmt.value() {
            self.emit_tree(expr.syntax());
        }
    }

    fn if_stmt(&mut self, stmt: ast::IfStmt<'_>) {
        let tree = stmt.syntax();
        let mut block_seen = false;

        for child in &tree.children {
            match child {
                Child::Token(t) if t.kind == TokenKind::EndIf => {
                    self.write_indent();
                    self.emit_token(t);
                }
                Child::Token(t) => self.emit_token(t),
                Child::Tree(t) if t.kind == TreeKind::Block => {
                    if !block_seen {
                        self.newline();
                        block_seen = true;
                    }
                    self.emit_block_tree(t);
                }
                Child::Tree(t) if t.kind == TreeKind::StmtElseIf => {
                    self.write_indent();
                    self.emit_clause_header_then_block(t);
                }
                Child::Tree(t) if t.kind == TreeKind::StmtElse => {
                    self.write_indent();
                    for child2 in &t.children {
                        match child2 {
                            Child::Token(t2) => {
                                self.emit_token(t2);
                                self.newline();
                            }
                            Child::Tree(t2) if t2.kind == TreeKind::Block => {
                                self.emit_block_tree(t2);
                            }
                            Child::Tree(t2) => self.emit_tree(t2),
                        }
                    }
                }
                Child::Tree(t) => self.emit_tree(t),
            }
        }
    }

    fn while_stmt(&mut self, stmt: ast::WhileStmt<'_>) {
        let tree = stmt.syntax();
        let mut block_seen = false;

        for child in &tree.children {
            match child {
                Child::Token(t) if t.kind == TokenKind::EndWhile => {
                    self.write_indent();
                    self.emit_token(t);
                }
                Child::Token(t) => self.emit_token(t),
                Child::Tree(t) if t.kind == TreeKind::Block => {
                    if !block_seen {
                        self.newline();
                        block_seen = true;
                    }
                    self.emit_block_tree(t);
                }
                Child::Tree(t) => self.emit_tree(t),
            }
        }
    }

    /// Emit a raw `Block` CST node at `depth + 1`.
    ///
    /// Unlike [`block`], which uses the typed AST iterator, this walks raw
    /// children so it works for `StmtWhile`/`StmtIf` blocks that aren't
    /// reached through the AST `Block` wrapper.
    ///
    /// Indentation for each statement is handled entirely inside
    /// [`emit_token`]: when `at_line_start` is true, the first token on a
    /// line automatically receives the current indent.  This avoids the
    /// double-indent that occurs when a statement's first token carries a
    /// leading `LineComment` (the comment emits its own indent; if we also
    /// called `write_indent()` here we'd indent twice).
    fn emit_block_tree(&mut self, tree: &Tree) {
        self.depth += 1;
        for child in &tree.children {
            match child {
                Child::Token(t) => self.emit_token(t),
                Child::Tree(t) => {
                    self.emit_stmt_tree(t);
                    self.newline();
                }
            }
        }
        self.depth -= 1;
    }

    fn emit_stmt_tree(&mut self, tree: &Tree) {
        match tree.kind {
            TreeKind::StmtWhile => {
                if let Some(s) = ast::WhileStmt::cast(tree) {
                    self.while_stmt(s);
                } else {
                    self.emit_tree(tree);
                }
            }
            TreeKind::StmtIf => {
                if let Some(s) = ast::IfStmt::cast(tree) {
                    self.if_stmt(s);
                } else {
                    self.emit_tree(tree);
                }
            }
            TreeKind::StmtReturn => {
                if let Some(s) = ast::ReturnStmt::cast(tree) {
                    self.return_stmt(s);
                } else {
                    self.emit_tree(tree);
                }
            }
            _ => self.emit_tree(tree),
        }
    }

    fn emit_clause_header_then_block(&mut self, tree: &Tree) {
        let mut block_seen = false;
        for child in &tree.children {
            match child {
                Child::Tree(t) if t.kind == TreeKind::Block => {
                    if !block_seen {
                        self.newline();
                        block_seen = true;
                    }
                    self.emit_block_tree(t);
                }
                Child::Tree(t) => self.emit_tree(t),
                Child::Token(t) => self.emit_token(t),
            }
        }
    }

    // ── Low-level CST emitters ────────────────────────────────────────────────

    fn emit_tree(&mut self, tree: &Tree) {
        for child in &tree.children {
            match child {
                Child::Token(t) => self.emit_token(t),
                Child::Tree(t) => self.emit_tree(t),
            }
        }
    }

    fn emit_until_block(&mut self, tree: &Tree) {
        for child in &tree.children {
            match child {
                Child::Tree(t) if t.kind == TreeKind::Block => break,
                Child::Tree(t) => self.emit_tree(t),
                Child::Token(t) => self.emit_token(t),
            }
        }
    }

    fn emit_closing_token(&mut self, tree: &Tree, kind: TokenKind) {
        if let Some(tok) = support::token(tree, kind) {
            self.emit_token(tok);
        }
    }

    /// Core token emitter.
    ///
    /// Trivia strategy:
    /// - Leading `LineComment` / `BlockComment` → own line(s) before the token.
    /// - Trailing `LineComment` → same line as the token, no trailing `\n`
    ///   (the caller's `newline()` closes the line).
    /// - Trailing `BlockComment` → inline, no trailing `\n`.
    /// - `LineContinuation` → verbatim in place.
    /// - `Whitespace` / `Newline` → discarded; regenerated by the formatter.
    ///
    /// Indentation: when `at_line_start` is true and we are about to emit
    /// the token text itself, `write_indent()` is called first.  This means
    /// callers do **not** need to call `write_indent()` before `emit_token`.
    fn emit_token(&mut self, token: &Token) {
        // ── Leading trivia ────────────────────────────────────────────────────
        for trivia in &token.leading_trivia {
            match trivia.kind {
                // Line and block comments each occupy their own line.
                TriviaKind::LineComment | TriviaKind::BlockComment => {
                    if !self.at_line_start {
                        self.out.push('\n');
                        self.at_line_start = true;
                        self.prev_kind = None;
                    }
                    self.write_indent();
                    self.raw_push(&self.src[trivia.span.as_range()]);
                    self.out.push('\n');
                    self.at_line_start = true;
                    self.prev_kind = None;
                }
                TriviaKind::LineContinuation => {
                    self.raw_push(&self.src[trivia.span.as_range()]);
                }
                TriviaKind::Whitespace | TriviaKind::Newline => {}
            }
        }

        // ── Inter-token space ─────────────────────────────────────────────────
        // Auto-indent when we're at the start of a line (handles the case
        // where no leading trivia was present).
        if self.at_line_start {
            self.write_indent();
        } else {
            let last = self.out.as_bytes().last().copied();
            let already_spaced = matches!(last, Some(b' ') | Some(b'\n') | Some(b'\t') | None);
            if !already_spaced && needs_space_between(self.prev_kind, token.kind) {
                self.out.push(' ');
            }
        }

        // ── Token text ────────────────────────────────────────────────────────
        let text = &self.src[token.span.as_range()];
        if let Some(normalized) = token.kind.keyword(text, self.options.keyword_case) {
            self.raw_push(&normalized);
        } else {
            self.raw_push(text);
        }

        self.prev_kind = Some(token.kind);

        // ── Trailing trivia ───────────────────────────────────────────────────
        for trivia in &token.trailing_trivia {
            match trivia.kind {
                // A trailing line comment stays on the same line; the caller's
                // newline() will close it.  We only add a leading space.
                TriviaKind::LineComment => {
                    if !matches!(self.out.as_bytes().last(), Some(b' ')) {
                        self.out.push(' ');
                    }
                    self.raw_push(&self.src[trivia.span.as_range()]);
                    // No '\n' — caller owns the line ending.
                }
                // A trailing block comment stays inline.
                TriviaKind::BlockComment => {
                    if !matches!(self.out.as_bytes().last(), Some(b' ')) {
                        self.out.push(' ');
                    }
                    self.raw_push(&self.src[trivia.span.as_range()]);
                }
                TriviaKind::LineContinuation => {
                    self.raw_push(&self.src[trivia.span.as_range()]);
                }
                TriviaKind::Whitespace | TriviaKind::Newline => {}
            }
        }
    }

    // ── Output primitives ─────────────────────────────────────────────────────

    fn raw_push(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.out.push_str(s);
        self.at_line_start = s.ends_with('\n');
    }

    fn newline(&mut self) {
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.at_line_start = true;
        self.prev_kind = None;
    }

    fn blank_lines(&mut self, n: u32) {
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        for _ in 0..n {
            self.out.push('\n');
        }
        self.at_line_start = true;
        self.prev_kind = None;
    }

    fn write_indent(&mut self) {
        if !self.at_line_start {
            return;
        }
        let unit: &str = match self.options.indent {
            Indent::Tabs => "\t",
            Indent::Spaces(n) => match n {
                1 => " ",
                2 => "  ",
                3 => "   ",
                4 => "    ",
                5 => "     ",
                6 => "      ",
                7 => "       ",
                8 => "        ",
                _ => " ",
            },
        };
        for _ in 0..self.depth {
            self.out.push_str(unit);
        }
        // Even depth-0 tokens clear at_line_start so subsequent tokens on the
        // same line don't re-indent.
        self.at_line_start = false;
    }
}

// ── Spacing rules ─────────────────────────────────────────────────────────────

fn needs_space_between(prev: Option<TokenKind>, cur: TokenKind) -> bool {
    if let Some(p) = prev
        && matches!(p, TokenKind::LParen | TokenKind::LBracket | TokenKind::Dot)
    {
        return false;
    }
    if matches!(
        cur,
        TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::Dot
            | TokenKind::Comma
            | TokenKind::Eof
    ) {
        return false;
    }
    true
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use papyrus_cst::{display_error, parse_papyrus};

    use super::*;

    fn parse_and_format(src: &str, options: &FormatOptions) -> String {
        let (tree, errors) = parse_papyrus(src);
        if !errors.is_empty() {
            panic!(
                "parse errors:\n{}",
                display_error::display_errors(src, "<test>", &errors)
            );
        }
        format(src, &tree, options)
    }

    fn fmt(src: &str) -> String {
        parse_and_format(src, &FormatOptions::default())
    }

    #[test]
    fn scriptname_header_preserved() {
        let src = "ScriptName Foo Extends Bar\n";
        let out = fmt(src);
        assert!(out.starts_with("ScriptName Foo Extends Bar"), "got:\n{out}");
    }

    #[test]
    fn function_indents_body() {
        let src = "Function Noop()\nReturn\nEndFunction\n";
        let out = fmt(src);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[1].starts_with("    "), "body not indented:\n{out}");
    }

    #[test]
    fn typed_function() {
        let src = "Int Function Add(Int a, Int b)\nReturn a + b\nEndFunction\n";
        let out = fmt(src);
        assert!(
            out.contains("Int Function Add"),
            "prefix return type lost:\n{out}"
        );
        assert!(out.contains("Return a + b"), "return stmt lost:\n{out}");
        assert!(out.contains("EndFunction"), "EndFunction lost:\n{out}");
    }

    #[test]
    fn member_call_no_spaces() {
        let src = "Function F()\nDebug.Trace(\"hi\")\nEndFunction\n";
        let out = fmt(src);
        assert!(out.contains("Debug.Trace("), "got:\n{out}");
        assert!(!out.contains("Debug .Trace"), "space after Debug:\n{out}");
        assert!(!out.contains("Trace( "), "space after LParen:\n{out}");
    }

    #[test]
    fn keyword_case_lower() {
        let src = "Function F()\nReturn\nEndFunction\n";
        let out = parse_and_format(
            src,
            &FormatOptions {
                keyword_case: KeywordCase::Lower,
                ..Default::default()
            },
        );
        assert!(out.contains("function"), "got:\n{out}");
        assert!(out.contains("endfunction"), "got:\n{out}");
    }

    #[test]
    fn comments_preserved() {
        let src = "Function F() ; inline comment\nReturn ; return comment\nEndFunction\n";
        let out = fmt(src);
        assert!(out.contains("; inline comment"), "got:\n{out}");
        assert!(out.contains("; return comment"), "got:\n{out}");
    }

    #[test]
    fn blank_lines_between_functions() {
        let src = "Function A()\nEndFunction\nFunction B()\nEndFunction\n";
        let out = parse_and_format(
            src,
            &FormatOptions {
                blank_lines_between_decls: 1,
                ..Default::default()
            },
        );
        assert!(
            out.contains("EndFunction\n\nFunction"),
            "expected blank line:\n{out}"
        );
    }

    #[test]
    fn full_fixture() {
        let path = "../../tests/simple/test.psc";
        let path = display_error::to_filename(path);
        let Ok(src) = std::fs::read_to_string(&path) else {
            return; // Skip if fixture not present.
        };
        let (tree, errors) = parse_papyrus(&src);
        if !errors.is_empty() {
            panic!("{}", display_error::display_errors(&src, &path, &errors));
        }
        let formatted_path = "../../target/formatted.psc";
        let formatted_path = display_error::to_filename(formatted_path);

        let formatted = format(&src, &tree, &FormatOptions::default());
        let _ = std::fs::write(&formatted_path, &formatted);

        // Idempotency: formatting the output must yield the same result.
        let (tree2, errors2) = parse_papyrus(&formatted);
        assert!(
            errors2.is_empty(),
            "re-parse errors:\n{}",
            display_error::display_errors(&formatted, &formatted_path, &errors2)
        );
        let formatted2 = format(&formatted, &tree2, &FormatOptions::default());
        assert_eq!(formatted, formatted2, "formatter is not idempotent");
    }
}
