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

    fn current_token(&self) -> Option<&Token> {
        self.check_fuel();
        self.tokens.get(self.pos)
    }

    pub fn peek(&self) -> Option<&Token> {
        self.current_token()
    }

    pub fn at(&self, kind: TokenKind) -> bool {
        self.current_token().is_some_and(|t| t.kind == kind)
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
        self.current_token()
            .is_some_and(|t| kinds.contains(&t.kind))
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
                .current_token()
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
        assert!(f > 0, "parser is stuck: fuel exhausted at pos {}", self.pos);
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
        Some(TokenKind::Event) => event(p),
        Some(TokenKind::State) => state(p),
        Some(TokenKind::Eof) | None => {}

        Some(TokenKind::Property) => property(p),
        Some(TokenKind::Function) => function(p),
        _ if p.at_type_start() && p.nth(1) == Some(TokenKind::Property) => property(p),
        _ if p.at_type_start() && p.nth(1) == Some(TokenKind::Function) => function(p),
        _ => stmt(p), // Variable declarations or bare expressions at file scope (unusual in Papyrus but we handle them gracefully).
    }
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
        match p.peek().map(|t| t.kind) {
            Some(TokenKind::Function)
            | Some(TokenKind::Event)
            | Some(TokenKind::Property)
            | Some(TokenKind::State) => top_level(p),

            // Has return type function
            Some(TokenKind::Int)
            | Some(TokenKind::Float)
            | Some(TokenKind::Bool)
            | Some(TokenKind::StringTy)
            | Some(TokenKind::Identifier)
                if p.nth(1) == Some(TokenKind::Function) =>
            {
                function(p);
            }
            _ => break,
        }
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
    has_return_type: bool,
) {
    let m = p.open();

    if has_return_type && p.at_type_start() {
        type_expr(p);
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
            _ if p.at_type_start() && p.nth(1) == Some(TokenKind::Function) => function(p),

            _ => stmt(p),
        }
    }

    p.close(m, TreeKind::Block);
}

/// `<LineProperty> | <PropertyBlock>`
fn property(p: &mut Parser) {
    // lookahead only, no consumption logic here
    if is_property_line(p) {
        property_line(p);
    } else {
        property_block(p);
    }
}

/// Is `<Type> "Property" <Name> "="`
fn is_property_line(p: &Parser) -> bool {
    matches!(
        (p.nth(1), p.nth(3)),
        (Some(TokenKind::Property), Some(TokenKind::Assign))
    )
}

/// `<Type> Property <Name> ["=" <Expr>] [Auto | AutoReadOnly]`
fn property_line(p: &mut Parser) {
    loop {
        if !p.at_type_start() {
            break;
        }

        // like var_decl_stmt
        let m = p.open();

        type_expr(p);
        p.expect(TokenKind::Property);
        name(p);

        if p.eat(TokenKind::Assign) {
            expr(p);
        }

        if flags_line(p) {
            p.close(m, TreeKind::Property);
        } else {
            // invalid recovery
            p.close(m, TreeKind::Error);
        }
    }
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
fn property_block(p: &mut Parser) {
    let m = p.open();

    // <Type> Property <Name>
    type_expr(p);
    p.expect(TokenKind::Property);
    name(p);

    // Auto / AutoReadOnly shorthand – no explicit getter/setter.
    if flags_line(p) {
        p.close(m, TreeKind::Property);
        return;
    }

    // Full property: optional getter and/or setter functions.
    // - function get()
    // - bool function get()
    while p.at(TokenKind::Function) || (p.at_type_start() && p.nth(1) == Some(TokenKind::Function))
    {
        function(p);
    }

    p.expect(TokenKind::EndProperty);
    p.close(m, TreeKind::Property);
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// `( [<Param> [, <Param>]*] )`
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

    // Optional array suffix.
    if p.at(TokenKind::LBracket) {
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
    let mut has_auto = false;

    loop {
        match p.peek().map(|t| t.kind) {
            // reserved flags
            Some(TokenKind::Auto | TokenKind::AutoReadOnly) => {
                has_auto = true;
                p.bump();
            }
            Some(TokenKind::Native | TokenKind::Global) => {
                p.bump();
            }
            Some(TokenKind::Identifier) => {
                p.bump(); // user flags (Identifier) conditional
            }
            _ => break, // keywords and others
        }

        // newline boundary (trivia)
        if has_newline_before_next(p) {
            break;
        }
    }
    has_auto
}

fn has_newline_before_next(p: &Parser) -> bool {
    let Some(tok) = p.current_token() else {
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

        // `Identifier Identifier` → variable declaration (e.g. `Actor akTarget`)
        // `Identifier [` → array type declaration (e.g. `Int[] arr`)
        Some(TokenKind::Identifier)
            if matches!(p.nth(1), Some(TokenKind::Identifier))
                || matches!(p.nth(1), Some(TokenKind::LBracket))
                    && p.nth(2) == Some(TokenKind::RBracket)
                    && p.nth(3) == Some(TokenKind::Identifier) =>
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
    p.close(m, TreeKind::StmtExpr); // reuse StmtExpr; add StmtVarDecl to TreeKind if desired
}

fn return_stmt(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::Return);

    // Optional return value: absent when we're at a terminator or EOF.
    if !p.eof() && !p.at_any(BLOCK_TERMINATORS) && !p.at(TokenKind::Eof) {
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
                    name(p);
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use papyrus_token::{span::TextSpan, token::Token, token::TokenKind};

    use super::*;
    use crate::cst::TreeKind;

    // ── Token builder helpers ─────────────────────────────────────────────────

    fn tok(kind: TokenKind) -> Token {
        Token::new(kind, TextSpan::empty(0))
    }

    fn parse_tokens(tokens: Vec<Token>) -> (Tree, Vec<ParseError>) {
        // Always terminate with Eof.
        let mut ts = tokens;
        ts.push(tok(TokenKind::Eof));
        let mut p = Parser::new(ts);
        file(&mut p);
        p.build_tree()
    }

    fn ident() -> Token {
        tok(TokenKind::Identifier)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn assert_root_kind(tree: &Tree, kind: TreeKind) {
        assert_eq!(tree.kind, kind, "root kind mismatch");
    }

    fn first_child_tree(tree: &Tree) -> &Tree {
        tree.child_trees().next().expect("no child tree")
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn empty_file() {
        let (tree, errors) = parse_tokens(vec![]);
        assert_root_kind(&tree, TreeKind::File);
        assert!(errors.is_empty());
        assert_eq!(tree.children.len(), 0);
    }

    #[test]
    fn scriptname_no_extends() {
        let tokens = vec![tok(TokenKind::ScriptName), ident()];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let script = first_child_tree(&tree);
        assert_eq!(script.kind, TreeKind::Script);
    }

    #[test]
    fn scriptname_with_extends() {
        let tokens = vec![
            tok(TokenKind::ScriptName),
            ident(),
            tok(TokenKind::Extends),
            ident(),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let script = first_child_tree(&tree);
        assert_eq!(script.kind, TreeKind::Script);
    }

    #[test]
    fn empty_function() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        assert_eq!(func.kind, TreeKind::Function);
    }

    #[test]
    fn function_with_return_type() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            tok(TokenKind::Colon),
            tok(TokenKind::Int),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        assert_eq!(func.kind, TreeKind::Function);
    }

    #[test]
    fn function_with_params() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(), // name
            tok(TokenKind::LParen),
            tok(TokenKind::Int), // param type
            ident(),             // param name
            tok(TokenKind::Comma),
            tok(TokenKind::Bool), // param type
            ident(),              // param name
            tok(TokenKind::RParen),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        assert_eq!(func.kind, TreeKind::Function);
    }

    #[test]
    fn return_with_value() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            tok(TokenKind::Return),
            tok(TokenKind::Number),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        assert_eq!(func.kind, TreeKind::Function);
        // Block → StmtReturn should exist somewhere inside.
        let block = func.find_child(TreeKind::Block).expect("no Block");
        let ret = block
            .find_child(TreeKind::StmtReturn)
            .expect("no StmtReturn");
        assert!(ret.find_child(TreeKind::ExprLiteral).is_some());
    }

    #[test]
    fn if_elseif_else() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            // if True
            tok(TokenKind::If),
            tok(TokenKind::True),
            // elseif False
            tok(TokenKind::ElseIf),
            tok(TokenKind::False),
            // else
            tok(TokenKind::Else),
            tok(TokenKind::EndIf),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        let block = func.find_child(TreeKind::Block).unwrap();
        let if_stmt = block.find_child(TreeKind::StmtIf).unwrap();
        assert!(if_stmt.find_child(TreeKind::StmtElseIf).is_some());
        assert!(if_stmt.find_child(TreeKind::StmtElse).is_some());
    }

    #[test]
    fn while_loop() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            tok(TokenKind::While),
            tok(TokenKind::True),
            tok(TokenKind::EndWhile),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        let block = func.find_child(TreeKind::Block).unwrap();
        assert!(block.find_child(TreeKind::StmtWhile).is_some());
    }

    #[test]
    fn binary_expr_precedence() {
        // Parses `1 + 2 * 3`; the `*` subtree should be nested inside `+`.
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            tok(TokenKind::Number), // 1
            tok(TokenKind::Plus),
            tok(TokenKind::Number), // 2
            tok(TokenKind::Star),
            tok(TokenKind::Number), // 3
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        // The outer binary expression is `+`.
        let outer_bin = stmt.find_child(TreeKind::ExprBinary).unwrap();
        // Its right-hand operand should be another ExprBinary (`*`).
        assert!(
            outer_bin.find_child(TreeKind::ExprBinary).is_some(),
            "multiplication should be nested under addition"
        );
    }

    #[test]
    fn member_access_and_call() {
        // Parses `foo.Bar()`
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            ident(), // foo
            tok(TokenKind::Dot),
            ident(), // Bar
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            tok(TokenKind::EndFunction),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let func = first_child_tree(&tree);
        let block = func.find_child(TreeKind::Block).unwrap();
        let stmt = block.find_child(TreeKind::StmtExpr).unwrap();
        assert!(stmt.find_child(TreeKind::ExprCall).is_some());
    }

    #[test]
    fn missing_end_function_emits_error() {
        let tokens = vec![
            tok(TokenKind::Function),
            ident(),
            tok(TokenKind::LParen),
            tok(TokenKind::RParen),
            // EndFunction intentionally omitted.
        ];
        let (_tree, errors) = parse_tokens(tokens);
        assert!(
            !errors.is_empty(),
            "expected at least one error for missing EndFunction"
        );
    }

    #[test]
    fn state_declaration() {
        let tokens = vec![tok(TokenKind::State), ident(), tok(TokenKind::EndState)];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let s = first_child_tree(&tree);
        assert_eq!(s.kind, TreeKind::State);
    }

    #[test]
    fn auto_property() {
        let tokens = vec![
            tok(TokenKind::Property),
            tok(TokenKind::Int),
            ident(), // name
            tok(TokenKind::Auto),
        ];
        let (tree, errors) = parse_tokens(tokens);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        let prop = first_child_tree(&tree);
        assert_eq!(prop.kind, TreeKind::Property);
    }
}
