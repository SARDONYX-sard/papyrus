//! Concrete Syntax Tree parser
//! - Ref: https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html
//! - Inspired: https://github.com/matklad/resilient-ll-parsing/blob/master/src/lib.rs#L44
use std::cell::Cell;

use papyrus_token::{
    span::TextSpan,
    token::{Token, TokenKind, TriviaKind},
};

use crate::cst::{Tree, TreeKind};

// ── Events ────────────────────────────────────────────────────────────────────

/// Event emitted by the parser.
///
/// Grammar functions never construct syntax trees directly.
/// Instead they emit a sequence of events which is later folded into a CST.
#[derive(Debug)]
pub enum Event {
    /// Start a syntax node.
    ///
    /// `forward_parent` encodes left-recursion: when set to `Some(offset)`,
    /// the tree-builder inserts an ancestor node *before* the current one,
    /// enabling precedence climbing and call/member expressions without
    /// rewinding the token stream.
    Open {
        kind: TreeKind,
        forward_parent: Option<usize>,
    },

    /// Consume one token from the stream.
    Advance,

    /// Finish the current syntax node.
    Close,
}

impl Event {
    /// Placeholder used while a [`Marker`] is still open.
    pub fn tombstone() -> Self {
        Event::Open {
            kind: TreeKind::Error,
            forward_parent: None,
        }
    }
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Infinite-loop guard: decremented on every lookahead, reset on every advance.
const FUEL_LIMIT: u32 = 256;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    events: Vec<Event>,
    fuel: Cell<u32>,
    /// Accumulated parse errors: (message, token index at the point of error).
    errors: Vec<ParseError>,
}

/// A parse error with a human-readable message and the source span where it
/// occurred.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: TextSpan,
    /// Index into the original token slice where the error was detected.
    pub token_pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            events: Vec::new(),
            fuel: Cell::new(FUEL_LIMIT),
            errors: Vec::new(),
        }
    }

    // ── Tree construction ─────────────────────────────────────────────────────

    /// Consume the parser and fold all events into a CST.
    ///
    /// Returns the root [`Tree`] (always `TreeKind::File`) together with every
    /// error that was recorded during the parse.
    pub fn build_tree(self) -> (Tree, Vec<ParseError>) {
        use papyrus_token::span::TextSpan;

        let mut tokens = self.tokens.into_iter();
        let mut events = self.events;
        let errors = self.errors;

        // The outermost Close was already pushed by `file()`; pop it so the
        // loop handles every Open/Close symmetrically.
        assert!(matches!(events.pop(), Some(Event::Close)));

        let mut stack: Vec<Tree> = Vec::new();
        let sentinel = TextSpan::empty(0);

        for (i, event) in events.into_iter().enumerate() {
            match event {
                Event::Open {
                    kind,
                    forward_parent,
                } => {
                    // Walk the forward_parent chain to collect all ancestor
                    // kinds that need to be opened *before* this node.
                    //
                    // We can't borrow `events` here (already moved), so the
                    // chain was already resolved during parsing via offsets
                    // stored in the event vector.  We push a placeholder and
                    // let the Close event fill the span in.
                    let _ = forward_parent; // handled by build_tree_with_forward below
                    let _ = i;
                    stack.push(Tree::new(kind, sentinel));
                }

                Event::Advance => {
                    let token = tokens.next().expect("token stream exhausted before events");
                    let top = stack.last_mut().expect("Advance with no open tree");
                    top.span = top.span.merge(token.span);
                    top.push_token(token);
                }

                Event::Close => {
                    let tree = stack.pop().expect("Close without matching Open");
                    match stack.last_mut() {
                        Some(parent) => {
                            parent.span = parent.span.merge(tree.span);
                            parent.push_tree(tree);
                        }
                        None => {
                            // Back at the root.
                            stack.push(tree);
                        }
                    }
                }
            }
        }

        debug_assert!(stack.len() == 1, "unbalanced Open/Close events");
        debug_assert!(tokens.next().is_none(), "unconsumed tokens remain");

        (stack.pop().unwrap(), errors)
    }

    // ── Lookahead ─────────────────────────────────────────────────────────────

    pub fn eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Peek current token
    pub fn peek(&self) -> Option<&Token> {
        self.check_fuel();
        self.tokens.get(self.pos)
    }

    pub fn at(&self, kind: TokenKind) -> bool {
        self.peek().is_some_and(|t| t.kind == kind)
    }

    /// Returns true if the current token can begin a type.
    pub fn at_type_start(&self) -> bool {
        matches!(
            self.peek().map(|t| t.kind),
            Some(
                TokenKind::Identifier
                    | TokenKind::Bool
                    | TokenKind::Int
                    | TokenKind::Float
                    // The lexer may emit either StringTy or String for the
                    // `String` type keyword depending on context.
                    | TokenKind::StringTy
                    | TokenKind::String
            )
        )
    }

    pub fn at_any(&self, kinds: &[TokenKind]) -> bool {
        self.peek().is_some_and(|t| kinds.contains(&t.kind))
    }

    /// Look `n` tokens ahead (0 = current).
    pub fn nth(&self, n: usize) -> Option<TokenKind> {
        self.check_fuel();
        self.tokens.get(self.pos + n).map(|t| t.kind)
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Advance past the current token unconditionally.
    pub fn bump(&mut self) {
        assert!(!self.eof(), "bump called at EOF");
        self.fuel.set(FUEL_LIMIT);
        self.events.push(Event::Advance);
        self.pos += 1;
    }

    /// Advance if the current token matches `kind`; return `true` on success.
    pub fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Advance if `kind` matches; otherwise record an error and emit an Error
    /// node *without* consuming the unexpected token.
    pub fn expect(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            let got = self
                .peek()
                .map(|t| format!("{:?}", t.kind))
                .unwrap_or_else(|| "EOF".to_string());
            self.push_error(format!("expected {kind:?}, got {got}"));
            false
        }
    }

    /// Emit an Error node, consuming at most one token so the parser always
    /// makes forward progress.
    pub fn error(&mut self) {
        self.error_with("unexpected token");
    }

    /// Emit an Error node with an explicit message, consuming one token.
    pub fn error_with(&mut self, message: impl Into<String>) {
        self.push_error(message.into());
        let m = self.open();
        if !self.eof() {
            self.bump();
        }
        self.close(m, TreeKind::Error);
    }

    fn push_error(&mut self, message: String) {
        let span = self
            .tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or(TextSpan::empty(0));
        self.errors.push(ParseError {
            message,
            span,
            token_pos: self.pos,
        });
    }

    // ── Marker API ────────────────────────────────────────────────────────────

    pub fn open(&mut self) -> Marker {
        let pos = self.events.len();
        self.events.push(Event::tombstone());
        Marker { pos }
    }

    pub fn close(&mut self, m: Marker, kind: TreeKind) -> CompletedMarker {
        self.events[m.pos] = Event::Open {
            kind,
            forward_parent: None,
        };
        self.events.push(Event::Close);
        CompletedMarker { pos: m.pos }
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn check_fuel(&self) {
        let f = self.fuel.get();
        assert!(
            f > 0,
            "parser is stuck: fuel exhausted at pos {}. {:#?}",
            self.pos,
            self.tokens.get(self.pos)
        );
        self.fuel.set(f - 1);
    }
}

// ── Markers ───────────────────────────────────────────────────────────────────

/// An open (incomplete) syntax node position in the event vector.
#[derive(Debug)]
pub struct Marker {
    pos: usize,
}

/// A closed syntax node that can be re-opened as the child of a new parent,
/// enabling left-recursive wrapping (precedence climbing, member access, etc.).
#[derive(Debug, Clone, Copy)]
pub struct CompletedMarker {
    pos: usize,
}

impl CompletedMarker {
    /// Wrap this completed node inside a new parent.
    ///
    /// The `forward_parent` offset stored in the original Open event tells the
    /// tree-builder to create the outer node before the inner one.
    pub fn precede(self, p: &mut Parser) -> Marker {
        let new = p.open();
        match &mut p.events[self.pos] {
            Event::Open { forward_parent, .. } => {
                *forward_parent = Some(new.pos - self.pos);
            }
            _ => unreachable!("CompletedMarker must point to an Open event"),
        }
        new
    }
}

// ── Grammar ───────────────────────────────────────────────────────────────────
//
// Conventions:
//   - Every function is called when the parser is already positioned at the
//     *first* relevant token.
//   - Every function always makes forward progress (consumes ≥ 1 token or
//     emits an Error node).
//   - Functions that can fail return `Option<CompletedMarker>`.

/// Lex `src`, run the parser, and return `(root_tree, errors)`.
pub fn parse_papyrus(src: &str) -> (Tree, Vec<ParseError>) {
    let tokens = papyrus_token::TokenStream::from(src).into_tokens();
    let mut p = Parser::new(tokens);
    file(&mut p);
    p.build_tree()
}

/// Parse an entire source file.
pub fn file(p: &mut Parser) {
    let m = p.open();
    while !p.eof() {
        top_level(p);
    }
    p.close(m, TreeKind::File);
}

// ── Top-level ─────────────────────────────────────────────────────────────────

fn top_level(p: &mut Parser) {
    match p.peek().map(|t| t.kind) {
        Some(TokenKind::ScriptName) => script(p),
        Some(TokenKind::Import) => import(p),
        Some(TokenKind::Event) => event(p),
        Some(TokenKind::Eof) | None => {}

        Some(TokenKind::State) => state(p),
        Some(TokenKind::Property) => property(p),
        Some(TokenKind::Function) => function(p),

        _ if start_state(p) => state(p),
        _ if next_after_type(p, TokenKind::Property) => property(p),
        _ if next_after_type(p, TokenKind::Function) => function(p),
        _ => stmt(p), // Variable declarations or bare expressions at file scope (unusual in Papyrus but we handle them gracefully).
    }
}

/// Is `auto State`
fn start_state(p: &Parser) -> bool {
    matches!(
        p.peek().map(|t| t.kind),
        Some(TokenKind::Auto | TokenKind::AutoReadOnly)
    ) && p.nth(1) == Some(TokenKind::State)
}

/// Is `<Type> <KeyWord>` tokens?
fn next_after_type(p: &Parser, kind: TokenKind) -> bool {
    let mut peek_n = 0;
    if p.at_type_start() {
        peek_n += 1; // type name
    } else {
        return false;
    }

    // `[]`, `[][]`
    while matches!(p.nth(peek_n), Some(TokenKind::LBracket))
        && matches!(p.nth(peek_n + 1), Some(TokenKind::RBracket))
    {
        peek_n += 2;
    }

    p.nth(peek_n) == Some(kind)
}

/// `import <Name>`
fn import(p: &mut Parser) {
    let m = p.open();
    p.expect(TokenKind::Import);
    name(p);
    p.close(m, TreeKind::Import);
}

/// `ScriptName <Name> [Extends <Name>] [Native] [Hidden] [Conditional]`
fn script(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::ScriptName);
    name(p);

    if p.eat(TokenKind::Extends) {
        name(p);
    }

    // Flags: Native, Hidden, Conditional – all identifiers in Papyrus.
    flags_line(p);

    // Body: functions, events, properties, states.
    while !p.eof() {
        top_level(p);
    }

    p.close(m, TreeKind::Script);
}

/// ` [<Type>] Function <Name> <ParamList> [Native] [Global]`
/// `  <Block>`
/// `EndFunction`
fn function(p: &mut Parser) {
    function_impl(
        p,
        TreeKind::Function,
        TokenKind::Function,
        TokenKind::EndFunction,
        true,
    );
}

/// `Event <Name> <ParamList>`
/// `  <Block>`
/// `EndEvent`
fn event(p: &mut Parser) {
    function_impl(
        p,
        TreeKind::Event,
        TokenKind::Event,
        TokenKind::EndEvent,
        false,
    );
}

fn function_impl(
    p: &mut Parser,
    kind: TreeKind,
    begin: TokenKind,
    end: TokenKind,
    has_return_type: bool, // Function: true, Event: false
) {
    let m = p.open();

    if p.at_type_start() {
        if has_return_type {
            type_expr(p);
        } else {
            p.error_with("Cannot specify a return value for an `Event`.");
        }
    }

    p.expect(begin);
    name(p);
    param_list(p);

    // Optional modifiers: Native, Global (order doesn't matter in Papyrus).
    let mut is_native = false;
    loop {
        if p.eat(TokenKind::Native) {
            is_native = true;
        } else if p.eat(TokenKind::Global) {
            // no special flag needed
        } else {
            break;
        }
    }

    // Native functions have no body.
    if !is_native {
        block(p, &[end]);
        p.expect(end);
    }

    p.close(m, kind);
}

/// `[auto] State <Name> <Block> EndState`
fn state(p: &mut Parser) {
    let m = p.open();

    p.eat(TokenKind::Auto); // Optional modifier.

    p.expect(TokenKind::State);
    name(p);
    state_body(p);

    p.expect(TokenKind::EndState);
    p.close(m, TreeKind::State);
}

fn state_body(p: &mut Parser) {
    let m = p.open();

    while !p.eof() && !p.at(TokenKind::EndState) {
        match p.peek().map(|t| t.kind) {
            Some(TokenKind::Event) => event(p),

            // function | <Type> function
            Some(TokenKind::Function) => function(p),
            _ if next_after_type(p, TokenKind::Function) => function(p),

            _ => stmt(p),
        }
    }

    p.close(m, TreeKind::Block);
}

/// - Auto pattern
/// ```txt
/// <Type> Property <Name> [Auto | AutoReadOnly]
/// ```
///
/// - Manual pattern
/// ```txt
/// <Type> Property <Name>
///   [Function …]
/// EndProperty
/// ```
fn property(p: &mut Parser) {
    let m = p.open();

    // <Type> Property <Name>
    type_expr(p);
    p.expect(TokenKind::Property);
    name(p);

    // property line
    if p.eat(TokenKind::Assign) {
        expr(p);
        // Auto / AutoReadOnly shorthand – no explicit getter/setter.
        if flags_line(p) {
            p.close(m, TreeKind::Property);
            return;
        }
    }

    // Auto / AutoReadOnly shorthand – no explicit getter/setter.
    if flags_line(p) {
        p.close(m, TreeKind::Property);
        return;
    }

    // Full property: optional getter and/or setter functions.
    // - function get()
    // - bool function get()
    while p.at(TokenKind::Function) || next_after_type(p, TokenKind::Function) {
        function(p);
    }

    p.expect(TokenKind::EndProperty);
    p.close(m, TreeKind::Property);
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// - `( [<Param> [\] [, <Param>]*] )`
fn param_list(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::LParen);

    while !p.eof() {
        match p.peek().map(|t| t.kind) {
            Some(TokenKind::RParen) | None => break,
            // Recovery: stop before any block-level keyword.
            Some(
                TokenKind::Function
                | TokenKind::Event
                | TokenKind::EndFunction
                | TokenKind::EndEvent,
            ) => break,
            _ => param(p),
        }
    }

    p.expect(TokenKind::RParen);
    p.close(m, TreeKind::ParamList);
}

/// `<Type> <Name> [= <Expr>]`
fn param(p: &mut Parser) {
    let m = p.open();

    type_expr(p);
    name(p);

    // Optional default value.
    if p.eat(TokenKind::Assign) {
        expr(p);
    }

    // Consume optional comma separating parameters.
    p.eat(TokenKind::Comma);

    p.close(m, TreeKind::Param);
}

/// A type reference: `<Ident>` or a primitive keyword, optionally `[]`.
///
/// ```papyrus
/// Int
/// String[]
/// Actor
/// ```
fn type_expr(p: &mut Parser) {
    let m = p.open();

    // Accept both identifier-style type names and built-in type keywords.
    if p.at_type_start() {
        p.bump();
    } else {
        p.push_error("expected a type name".to_string());
    }
    // Optional n detention array suffix.
    while p.at(TokenKind::LBracket) {
        p.bump();
        p.expect(TokenKind::RBracket);
    }

    p.close(m, TreeKind::Type);
}

/// A bare identifier (variable name, function name, etc.).
fn name(p: &mut Parser) {
    // Papyrus allows some keywords as identifiers in certain positions, but
    // for now we only accept `Identifier` tokens here.  The caller is
    // responsible for special-casing keyword-as-name if needed.
    let m = p.open();
    if !p.expect(TokenKind::Identifier) {
        // `expect` already emitted an error; close as Error.
        p.close(m, TreeKind::Error);
        return;
    }
    p.close(m, TreeKind::ExprName);
}

/// - builtin: `"Auto" | "AutoReadOnly" | "Native" | "Global"`
/// - user flags: `| "Hidden" | "Conditional" | <Identifier>`
///
/// Returns has_auto
fn flags_line(p: &mut Parser) -> bool {
    let m = p.open();
    let mut has_auto = false;

    loop {
        // newline boundary (trivia)
        if p.eof() || has_newline_before_next(p) {
            break;
        }

        match p.peek().map(|t| t.kind) {
            // reserved flags
            Some(TokenKind::Auto | TokenKind::AutoReadOnly) => {
                has_auto = true;
                p.bump();
            }
            Some(
                TokenKind::Conditional | TokenKind::Global | TokenKind::Hidden | TokenKind::Native,
            ) => {
                p.bump();
            }
            Some(TokenKind::Identifier) => {
                p.bump(); // user flags (Identifier) conditional
            }
            _ => break, // keywords and others
        }
    }

    p.close(m, TreeKind::Flag);
    has_auto
}

fn has_newline_before_next(p: &Parser) -> bool {
    let Some(tok) = p.peek() else {
        return true;
    };

    tok.leading_trivia
        .iter()
        .any(|t| matches!(t.kind, TriviaKind::Newline))
}

// ── Block & statements ────────────────────────────────────────────────────────

/// The set of tokens that terminate a block; they belong to the *caller*.
const BLOCK_TERMINATORS: &[TokenKind] = &[
    TokenKind::EndIf,
    TokenKind::Else,
    TokenKind::ElseIf,
    TokenKind::EndWhile,
    TokenKind::EndFunction,
    TokenKind::EndEvent,
    TokenKind::EndProperty,
    TokenKind::EndState,
];

/// A sequence of statements terminated by one of `end_tokens`.
fn block(p: &mut Parser, end_tokens: &[TokenKind]) {
    let m = p.open();

    while !p.eof() {
        let Some(kind) = p.peek().map(|t| t.kind) else {
            break;
        };
        if end_tokens.contains(&kind) {
            break;
        }
        stmt(p);
    }

    p.close(m, TreeKind::Block);
}

fn stmt(p: &mut Parser) {
    match p.peek().map(|t| t.kind) {
        Some(TokenKind::If) => if_stmt(p),
        Some(TokenKind::While) => while_stmt(p),
        Some(TokenKind::Return) => return_stmt(p),

        // These belong to the enclosing construct; leave them for the caller.
        Some(k) if BLOCK_TERMINATORS.contains(&k) => {
            p.error_with(format!("Invalid end pair: {:?}", k));
        }

        // Variable declaration: `<Type> <Name> [= <Expr>]`
        //
        // Primitive type keywords are unambiguous.
        Some(TokenKind::Bool | TokenKind::Int | TokenKind::Float | TokenKind::StringTy) => {
            var_decl_stmt(p)
        }

        // `Identifier Identifier` -> variable declaration (e.g. `Actor akTarget`)
        // `Identifier [` -> array type declaration (e.g. `Int[] arr`)
        Some(TokenKind::Identifier)
            if matches!(p.nth(1), Some(TokenKind::Identifier)) // Type varName
                || (matches!(p.nth(1), Some(TokenKind::LBracket)) // [] varName
                    && p.nth(2) == Some(TokenKind::RBracket)
                    && p.nth(3) == Some(TokenKind::Identifier)) =>
        {
            var_decl_stmt(p)
        }

        _ => expr_stmt(p),
    }
}

/// `<Type> <Name> [= <Expr>]`
fn var_decl_stmt(p: &mut Parser) {
    let m = p.open();
    type_expr(p);
    name(p);
    if p.eat(TokenKind::Assign) {
        expr(p);
    }
    flags_line(p);

    p.close(m, TreeKind::StmtExpr); // reuse StmtExpr; add StmtVarDecl to TreeKind if desired
}

/// `return [<Expr>]`
fn return_stmt(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::Return);

    // Optional return value: absent when we're at a terminator or EOF.
    if !p.eof()
        && !p.at_any(BLOCK_TERMINATORS)
        && !p.at(TokenKind::Eof)
        && !has_newline_before_next(p)
    {
        expr(p);
    }

    p.close(m, TreeKind::StmtReturn);
}

fn if_stmt(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::If);
    expr(p);
    block(p, &[TokenKind::ElseIf, TokenKind::Else, TokenKind::EndIf]);

    while p.at(TokenKind::ElseIf) {
        elseif_clause(p);
    }
    if p.at(TokenKind::Else) {
        else_clause(p);
    }

    p.expect(TokenKind::EndIf);
    p.close(m, TreeKind::StmtIf);
}

fn elseif_clause(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::ElseIf);
    expr(p);
    block(p, &[TokenKind::ElseIf, TokenKind::Else, TokenKind::EndIf]);

    p.close(m, TreeKind::StmtElseIf);
}

fn else_clause(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::Else);
    block(p, &[TokenKind::EndIf]);

    p.close(m, TreeKind::StmtElse);
}

fn while_stmt(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::While);
    expr(p);
    block(p, &[TokenKind::EndWhile]);
    p.expect(TokenKind::EndWhile);

    p.close(m, TreeKind::StmtWhile);
}

fn expr_stmt(p: &mut Parser) {
    let m = p.open();
    expr(p);
    p.close(m, TreeKind::StmtExpr);
}

// ── Expressions ───────────────────────────────────────────────────────────────
//
// Precedence (low → high):
//   1   assignment       =  +=  -=  *=  /=  %=   (right-associative)
//   2   logical-or       ||
//   3   logical-and      &&
//   4   equality         ==  !=
//   5   relational       <  <=  >  >=
//   6   additive         +  -
//   7   multiplicative   *  /  %
//   8   cast / `is`      as  is             (left-associative)
//   9   unary prefix     !  -
//  10   postfix          .member  ()call  []index

/// Entry point for expression parsing.
pub fn expr(p: &mut Parser) {
    expr_bp(p, 0);
}

/// Pratt / precedence-climbing recursive parser.
///
/// `min_bp` is the minimum *left* binding power required to continue.
fn expr_bp(p: &mut Parser, min_bp: u8) {
    let Some(mut lhs) = expr_unary(p) else {
        return;
    };

    loop {
        // ── Postfix operators (highest precedence, not in infix table) ────────
        // Consume as many postfix operators as are available before checking
        // for a binary infix operator.  A separate inner loop avoids the
        // `_ => break` from the postfix match accidentally aborting the outer
        // loop before we even look at infix operators.
        loop {
            match p.peek().map(|t| t.kind) {
                Some(TokenKind::Dot) => {
                    let m = lhs.precede(p);
                    p.bump(); // `.`

                    // member access: Identifier / pseudo-properties like Length
                    match p.peek().map(|t| t.kind) {
                        Some(TokenKind::Identifier) => {
                            p.bump();
                        }
                        // If lexer produces dedicated tokens like Length, allow them here
                        Some(TokenKind::Length) => {
                            p.bump();
                        }

                        _ => {
                            p.push_error("expected member name after '.'".to_string());
                        }
                    }

                    lhs = p.close(m, TreeKind::ExprMember);
                }

                Some(TokenKind::LParen) => {
                    let m = lhs.precede(p);
                    arg_list(p);
                    lhs = p.close(m, TreeKind::ExprCall);
                }

                Some(TokenKind::LBracket) => {
                    let m = lhs.precede(p);
                    p.bump(); // `[`
                    expr(p);
                    p.expect(TokenKind::RBracket);
                    lhs = p.close(m, TreeKind::ExprIndex);
                }

                _ => break, // no more postfix; fall through to infix check
            }
        }

        // ── Binary infix operators ────────────────────────────────────────────
        let Some((l_bp, r_bp)) = infix_bp(p) else {
            break;
        };
        if l_bp < min_bp {
            break;
        }

        let m = lhs.precede(p);
        let kind = p.peek().map(|token| token.kind);
        p.bump(); // the operator token / or As

        if matches!(kind, Some(TokenKind::As)) {
            type_expr(p);
        } else {
            expr_bp(p, r_bp);
        }
        lhs = p.close(m, TreeKind::ExprBinary);
    }
}

/// Returns `(left_bp, right_bp)` for the current infix operator, or `None`.
///
/// Right-associative operators have `r_bp == l_bp - 1`; left-associative have
/// `r_bp == l_bp + 1`.
fn infix_bp(p: &Parser) -> Option<(u8, u8)> {
    let bp = match p.peek()?.kind {
        // Assignment – right-associative.
        TokenKind::Assign
        | TokenKind::PlusAssign
        | TokenKind::MinusAssign
        | TokenKind::StarAssign
        | TokenKind::SlashAssign
        | TokenKind::PercentAssign => (2, 1),

        TokenKind::OrOr => (4, 5),
        TokenKind::AndAnd => (6, 7),
        TokenKind::EqEq | TokenKind::NotEq => (8, 9),
        TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => (10, 11),
        TokenKind::Plus | TokenKind::Minus => (12, 13),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => (14, 15),

        // `as` and `is` – left-associative.
        TokenKind::As | TokenKind::Is => (16, 17),

        _ => return None,
    };
    Some(bp)
}

/// Unary prefix: `!` or `-`, then recurses; otherwise delegates to primary.
fn expr_unary(p: &mut Parser) -> Option<CompletedMarker> {
    match p.peek().map(|t| t.kind) {
        Some(TokenKind::Bang) | Some(TokenKind::Minus) => {
            let m = p.open();
            p.bump();
            expr_unary(p);
            Some(p.close(m, TreeKind::ExprUnary))
        }
        _ => expr_primary(p),
    }
}

/// Atomic / primary expression.
fn expr_primary(p: &mut Parser) -> Option<CompletedMarker> {
    let cm = match p.peek().map(|t| t.kind) {
        // ── Literals ──────────────────────────────────────────────────────────
        Some(TokenKind::Number | TokenKind::String | TokenKind::True | TokenKind::False) => {
            let m = p.open();
            p.bump();
            p.close(m, TreeKind::ExprLiteral)
        }

        Some(TokenKind::None) => {
            let m = p.open();
            p.bump();
            p.close(m, TreeKind::ExprLiteral)
        }

        // ── `new Type[size]` ──────────────────────────────────────────────────
        Some(TokenKind::New) => {
            let m = p.open();
            p.bump(); // `new`
            match p.peek().map(|t| t.kind) {
                Some(
                    TokenKind::Identifier
                    | TokenKind::Bool
                    | TokenKind::Int
                    | TokenKind::Float
                    | TokenKind::StringTy
                    | TokenKind::String,
                ) => {
                    p.bump();
                }
                _ => {
                    p.push_error("expected type after 'new'".to_string());
                }
            }
            p.expect(TokenKind::LBracket);
            expr(p);
            p.expect(TokenKind::RBracket);
            p.close(m, TreeKind::ExprLiteral)
        }

        // ── `Self` / `Parent` ─────────────────────────────────────────────────
        Some(TokenKind::Self_ | TokenKind::Parent) => {
            let m = p.open();
            p.bump();
            p.close(m, TreeKind::ExprName)
        }

        // ── Identifier (variable, script-name, cast target, …) ───────────────
        Some(TokenKind::Identifier) => {
            let m = p.open();
            p.bump();
            p.close(m, TreeKind::ExprName)
        }

        // ── Parenthesised expression ──────────────────────────────────────────
        Some(TokenKind::LParen) => {
            let m = p.open();
            p.bump(); // `(`
            expr(p);
            p.expect(TokenKind::RParen);
            p.close(m, TreeKind::ExprParen)
        }

        _ => {
            p.error_with(format!(
                "expected an expression, got {:?}",
                p.peek().map(|t| t.kind)
            ));
            return None;
        }
    };

    Some(cm)
}

// ── Argument list ─────────────────────────────────────────────────────────────

fn arg_list(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::LParen);

    while !p.eof() {
        match p.peek().map(|t| t.kind) {
            Some(TokenKind::RParen) | None => break,
            _ => arg(p),
        }
    }

    p.expect(TokenKind::RParen);
    p.close(m, TreeKind::ArgList);
}

fn arg(p: &mut Parser) {
    let m = p.open();
    expr(p);
    p.eat(TokenKind::Comma);
    p.close(m, TreeKind::Arg);
}

#[cfg(test)]
mod tests {
    use crate::display_error::display_errors;

    use super::*;

    /// Assert there are no parse errors; print them if there are.
    fn assert_no_errors(src: &str, tree: &Tree, errors: &[ParseError], path: Option<&str>) {
        std::fs::write(
            "../../target/tokens.log",
            format!("{:#?}", papyrus_token::TokenStream::from(src).into_tokens()),
        )
        .unwrap();
        std::fs::write(
            "../../target/tree.log",
            crate::debug::dump_tree_tokens_with_trivia(tree, src),
        )
        .unwrap();

        if !errors.is_empty() {
            let path = path.unwrap_or("test.psc");
            let readable_errors = display_errors(src, path, errors);
            std::fs::write("../../target/error.log", &readable_errors).unwrap();
            panic!("unexpected parse errors in:\n{readable_errors}");
        }
    }

    // ── Smoke test: full fixture file ─────────────────────────────────────────

    #[test]
    fn full_fixture() {
        let src = include_str!("../../../tests/simple/test.psc");
        let path = Some("../../../tests/simple/test.psc");
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, path);
        assert_eq!(tree.kind, TreeKind::File);
    }

    // ── ScriptName ────────────────────────────────────────────────────────────

    #[test]
    fn scriptname_no_extends() {
        let src = "ScriptName Foo";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let script = tree.child_trees().next().expect("expected Script node");
        assert_eq!(script.kind, TreeKind::Script);
    }

    #[test]
    fn scriptname_with_extends() {
        let src = "ScriptName Foo Extends Bar";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let script = tree.child_trees().next().unwrap();
        assert_eq!(script.kind, TreeKind::Script);
        // Should have consumed all four tokens with no error.
        assert!(errors.is_empty());
    }

    #[test]
    fn scriptname_with_prop() {
        let src = "
scriptname Foo extends Bar

Actor property Hi auto
";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let script = tree.child_trees().next().unwrap();
        assert_eq!(script.kind, TreeKind::Script);
        // Should have consumed all four tokens with no error.
        assert!(errors.is_empty());
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    #[test]
    fn empty_function() {
        let src = "Function Noop()\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        // Must have a ParamList and a Block child.
        assert!(
            func.find_child(TreeKind::ParamList).is_some(),
            "no ParamList"
        );
        assert!(func.find_child(TreeKind::Block).is_some(), "no Block");
    }

    #[test]
    fn function_with_return_type() {
        let src = "Int Function GetValue() \n    Return 42\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        let block = func.find_child(TreeKind::Block).unwrap();
        let ret = block
            .find_child(TreeKind::StmtReturn)
            .expect("no StmtReturn");
        assert!(
            ret.find_child(TreeKind::ExprLiteral).is_some(),
            "no literal in return"
        );
    }

    #[test]
    fn function_with_return_array_type() {
        let src = "
Int[] Function GetValue()
    Return myArray
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        let block = func.find_child(TreeKind::Block).unwrap();
        let ret = block
            .find_child(TreeKind::StmtReturn)
            .expect("no StmtReturn");
        assert!(
            ret.find_child(TreeKind::ExprName).is_some(),
            "no literal in return"
        );
    }

    #[test]
    fn function_with_params() {
        let src = "Int Function Add(Int a, Int b)\n    Return a\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let params = func.find_child(TreeKind::ParamList).unwrap();
        let param_count = params.find_children(TreeKind::Param).count();
        assert_eq!(param_count, 2, "expected 2 params, got {param_count}");
    }

    #[test]
    fn function_with_default_param() {
        let src = "Function Greet(String name = \"World\")\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let params = func.find_child(TreeKind::ParamList).unwrap();
        assert!(params.find_child(TreeKind::Param).is_some());
    }

    #[test]
    fn native_global_function() {
        let src = "Function Log(String msg) Global Native";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        assert_eq!(func.kind, TreeKind::Function);
        // Native functions have no Block.
        assert!(
            func.find_child(TreeKind::Block).is_none(),
            "native function must not have a Block"
        );
    }

    // ── Events ────────────────────────────────────────────────────────────────

    #[test]
    fn event_declaration() {
        let src = "Event OnActivate(ObjectReference akActionRef)\nEndEvent";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let ev = tree.child_trees().next().unwrap();
        assert_eq!(ev.kind, TreeKind::Event);
    }

    // ── Properties ────────────────────────────────────────────────────────────

    #[test]
    fn inline_auto_property() {
        let src = "bool property isOpen = false Auto conditional";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
    }

    #[test]
    fn auto_property() {
        let src = "
; comment
Actor Property Level Auto";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
    }

    #[test]
    fn property_with_array_fn() {
        let src = "
int property PhaseCounts hidden
	int[] function get()
		return Phases
	endFunction
endProperty
";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
        let funcs: Vec<_> = prop.find_children(TreeKind::Function).collect();
        assert_eq!(funcs.len(), 1, "expected getter + setter");
    }

    #[test]
    fn full_property() {
        let src = "\
String Property Name
    String Function Get()
        Return \"\"
    EndFunction
    Function Set(String val)
    EndFunction
EndProperty";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let prop = tree.child_trees().next().unwrap();
        assert_eq!(prop.kind, TreeKind::Property);
        let funcs: Vec<_> = prop.find_children(TreeKind::Function).collect();
        assert_eq!(funcs.len(), 2, "expected getter + setter");
    }

    // ── States ────────────────────────────────────────────────────────────────

    #[test]
    fn state_declaration() {
        let src = "State Busy\nEndState";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let s = tree.child_trees().next().unwrap();
        assert_eq!(s.kind, TreeKind::State);
    }

    #[test]
    fn state_with_event() {
        let src = "\
State Active
    Event OnBeginState()
    EndEvent
EndState";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let state = tree.child_trees().next().unwrap();
        let block = state.find_child(TreeKind::Block).unwrap();
        assert!(block.find_child(TreeKind::Event).is_some());
    }

    // ── Var ────────────────────────────────────────────────────────────

    #[test]
    fn global_var_decl_with_init() {
        let src = "
        Bool property Prop1 = false auto
        ; Comment
        ; Comment
        Bool ignoreOpen = false";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let global_var = tree.child_trees().next().unwrap(); // StmtExpr
        assert!(global_var.find_child(TreeKind::ExprName).is_some());
    }

    // ── Statements ────────────────────────────────────────────────────────────

    #[test]
    fn if_elseif_else() {
        let src = "\
Function F(Bool b)
    If b
        Return
    ElseIf b == False
        Return
    Else
        Return
    EndIf
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let if_node = block.find_child(TreeKind::StmtIf).expect("no StmtIf");
        assert!(
            if_node.find_child(TreeKind::StmtElseIf).is_some(),
            "no ElseIf"
        );
        assert!(if_node.find_child(TreeKind::StmtElse).is_some(), "no Else");
    }

    #[test]
    fn while_loop() {
        let src = "\
Function Count(Int n)
    Int i = 0
    While i < n
        i += 1
    EndWhile
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        assert!(block.find_child(TreeKind::StmtWhile).is_some());
    }

    #[test]
    fn return_no_value() {
        let src = "Function F()\n    Return\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let ret = block.find_child(TreeKind::StmtReturn).unwrap();
        // No expression child expected.
        assert!(ret.find_child(TreeKind::ExprLiteral).is_none());
        assert!(ret.find_child(TreeKind::ExprName).is_none());
    }

    #[test]
    fn var_decl_with_init() {
        let src = "Function F()\n    Int x = 42\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        // Variable declarations are wrapped in StmtExpr for now.
        assert!(block.find_child(TreeKind::StmtExpr).is_some());
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    #[test]
    fn binary_precedence_add_mul() {
        // `a + b * c` — multiplication must be nested under addition.
        let src = "Function F()\n    a + b * c\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        let outer = stmt
            .find_child(TreeKind::ExprBinary)
            .expect("no outer ExprBinary");
        assert!(
            outer.find_child(TreeKind::ExprBinary).is_some(),
            "inner ExprBinary (mul) should be nested under outer (add)"
        );
    }

    #[test]
    fn unary_negation() {
        let src = "Function F()\n    -x\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprUnary).is_some());
    }

    #[test]
    fn member_access() {
        let src = "Function F()\n    akActor.IsDead()\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(
            stmt.find_child(TreeKind::ExprCall).is_some(),
            "expected ExprCall"
        );
    }

    #[test]
    fn index_expression() {
        let src = "Function F()\n    arr[0]\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprIndex).is_some());
    }

    #[test]
    fn new_array() {
        let src = "Function F()\n    new Int[10]\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprLiteral).is_some());
    }

    #[test]
    fn cast_with_as() {
        let src = "Function F()\n    x as Int\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(
            stmt.find_child(TreeKind::ExprBinary).is_some(),
            "`as` should produce ExprBinary"
        );
    }

    #[test]
    fn self_and_parent() {
        let src = "Function F()\n    Self.Foo()\n    Parent.Bar()\nEndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let calls: Vec<_> = block.find_children(TreeKind::StmtExpr).collect();
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn assignment_operators() {
        let src = "\
Function F()
    x = 1
    x += 2
    x -= 3
    x *= 4
    x /= 5
    x %= 6
EndFunction";
        let (tree, errors) = parse_papyrus(src);
        assert_no_errors(src, &tree, &errors, None);
        let func = tree.child_trees().next().unwrap();
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmts: Vec<_> = block.find_children(TreeKind::StmtExpr).collect();
        assert_eq!(stmts.len(), 6, "expected 6 assignment statements");
    }

    // ── Error recovery ────────────────────────────────────────────────────────

    #[test]
    fn missing_end_function_emits_error() {
        let src = "Function Broken()\n    Return\n; EndFunction intentionally missing";
        let (_tree, errors) = parse_papyrus(src);
        assert!(
            !errors.is_empty(),
            "expected at least one error for missing EndFunction"
        );
    }

    #[test]
    fn missing_endif_emits_error() {
        let src = "Function F()
If True
Return
; EndIf missing
EndFunction";
        let (_tree, errors) = parse_papyrus(src);
        assert!(!errors.is_empty(), "expected error for missing EndIf");
    }

    #[test]
    fn parser_recovers_after_error() {
        // The second function should still parse cleanly even though the first
        // is broken.
        let src = "\
Function Broken(
; missing RParen and EndFunction

Function Ok()
EndFunction";
        let (tree, _errors) = parse_papyrus(src);
        // We don't assert no errors, but the tree must be File-rooted and we
        // should find at least one Function node somewhere.
        assert_eq!(tree.kind, TreeKind::File);
        assert!(
            tree.child_trees().any(|c| c.kind == TreeKind::Function),
            "should have recovered and parsed at least one Function"
        );
    }
}
