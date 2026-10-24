//! Typed views over the untyped syntax tree.
//!
//! Every type here is a zero-cost, `Copy` wrapper around a [`SyntaxNode`] whose
//! kind has been checked. Accessors return `Option`s because the tree may be
//! incomplete when the source contains errors.

use crate::{
    SyntaxElement,
    SyntaxKind::{self, *},
    SyntaxNode, SyntaxToken,
};

/// A typed syntax node.
pub trait AstNode<'a>: Copy + Sized {
    /// Returns `true` if a node of `kind` can be viewed as `Self`.
    fn can_cast(kind: SyntaxKind) -> bool;

    /// Views `node` as `Self` if its kind matches.
    fn cast(node: SyntaxNode<'a>) -> Option<Self>;

    /// The underlying untyped node.
    fn syntax(&self) -> SyntaxNode<'a>;
}

fn child<'a, N: AstNode<'a>>(node: SyntaxNode<'a>) -> Option<N> {
    node.children().find_map(N::cast)
}

fn children<'a, N: AstNode<'a>>(node: SyntaxNode<'a>) -> impl Iterator<Item = N> + use<'a, N> {
    node.children().filter_map(N::cast)
}

macro_rules! ast_node {
    ($(#[$attr:meta])* $name:ident = $kind:ident) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name<'a>(SyntaxNode<'a>);

        impl<'a> AstNode<'a> for $name<'a> {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                Self::can_cast(node.kind()).then_some($name(node))
            }

            fn syntax(&self) -> SyntaxNode<'a> {
                self.0
            }
        }
    };
}

macro_rules! ast_enum {
    ($(#[$attr:meta])* $name:ident { $($variant:ident($node:ident)),* $(,)? }) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name<'a> {
            $(
                #[doc = concat!("A [`", stringify!($node), "`].")]
                $variant($node<'a>),
            )*
        }

        impl<'a> AstNode<'a> for $name<'a> {
            fn can_cast(kind: SyntaxKind) -> bool {
                $($node::can_cast(kind))||*
            }

            fn cast(node: SyntaxNode<'a>) -> Option<Self> {
                $(if let Some(n) = $node::cast(node) {
                    return Some($name::$variant(n));
                })*
                None
            }

            fn syntax(&self) -> SyntaxNode<'a> {
                match self {
                    $($name::$variant(n) => n.syntax(),)*
                }
            }
        }
    };
}

// === Names and types ===

ast_node!(
    /// The name introduced by a declaration.
    Name = NAME
);
ast_node!(
    /// A reference to a declaration: an identifier, `Self` or `Parent`.
    NameRef = NAME_REF
);
ast_node!(
    /// A type: `Int`, `Actor`, `String[]`.
    Type = TYPE
);
ast_node!(
    /// A trailing flag such as `Global`, `Native`, `Auto` or `Hidden`.
    Flag = FLAG
);

impl<'a> Name<'a> {
    /// The identifier token.
    pub fn ident(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_token(IDENT)
    }

    /// The identifier text.
    pub fn text(&self) -> &'a str {
        self.ident().map_or("", |t| t.text())
    }
}

impl<'a> NameRef<'a> {
    /// The identifier, `Self` or `Parent` token.
    pub fn token(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_tokens().find(|t| matches!(t.kind(), IDENT | SELF_KW | PARENT_KW))
    }

    /// The referenced name.
    pub fn text(&self) -> &'a str {
        self.token().map_or("", |t| t.text())
    }

    /// Returns `true` for `Self`.
    pub fn is_self(&self) -> bool {
        self.token().is_some_and(|t| t.kind() == SELF_KW)
    }

    /// Returns `true` for `Parent`.
    pub fn is_parent(&self) -> bool {
        self.token().is_some_and(|t| t.kind() == PARENT_KW)
    }
}

impl<'a> Type<'a> {
    /// The base type token: a primitive type keyword or an identifier.
    pub fn base(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_tokens().find(|t| t.kind() == IDENT || t.kind().is_primitive_type())
    }

    /// The base type name.
    pub fn base_text(&self) -> &'a str {
        self.base().map_or("", |t| t.text())
    }

    /// Returns `true` for `Bool`, `Float`, `Int` and `String`.
    pub fn is_primitive(&self) -> bool {
        self.base().is_some_and(|t| t.kind().is_primitive_type())
    }

    /// Returns `true` for array types such as `Int[]`.
    pub fn is_array(&self) -> bool {
        self.0.child_token(L_BRACK).is_some()
    }
}

impl<'a> Flag<'a> {
    /// The flag token.
    pub fn token(&self) -> Option<SyntaxToken<'a>> {
        self.0.first_token()
    }

    /// The flag spelling.
    pub fn text(&self) -> &'a str {
        self.token().map_or("", |t| t.text())
    }

    /// Returns `true` if this flag is `name`, ignoring case.
    pub fn is(&self, name: &str) -> bool {
        self.text().eq_ignore_ascii_case(name)
    }
}

fn has_flag<'a>(node: SyntaxNode<'a>, name: &str) -> bool {
    children::<Flag<'a>>(node).any(|f| f.is(name))
}

// === Declarations ===

ast_node!(
    /// The root of a script file.
    SourceFile = SOURCE_FILE
);
ast_node!(
    /// `ScriptName Foo extends Bar Hidden`
    ScriptHeader = SCRIPT_HEADER
);
ast_node!(
    /// `Import Debug`
    Import = IMPORT
);
ast_node!(
    /// A variable declaration: script level (`Int count = 0 Conditional`) or
    /// local (`Actor target = None`).
    VarDecl = VAR_DECL
);
ast_node!(
    /// `Int Property Foo Auto`, or a full property with `Get`/`Set` functions.
    Property = PROPERTY
);
ast_node!(
    /// `Auto State Waiting ... EndState`
    State = STATE
);
ast_node!(
    /// `Int Function Foo(Int a) Global ... EndFunction`
    Function = FUNCTION
);
ast_node!(
    /// `Event OnInit() ... EndEvent`
    Event = EVENT
);
ast_node!(
    /// `(Int a, Float b = 1.0)`
    ParamList = PARAM_LIST
);
ast_node!(
    /// `Float b = 1.0`
    Param = PARAM
);
ast_node!(
    /// `= value`
    Initializer = INITIALIZER
);

ast_enum!(
    /// A declaration that can appear at script level or inside a state.
    Item {
        Header(ScriptHeader),
        Import(Import),
        VarDecl(VarDecl),
        Property(Property),
        State(State),
        Function(Function),
        Event(Event),
    }
);

impl<'a> SourceFile<'a> {
    /// Wraps the root node of a tree.
    pub fn from_root(node: SyntaxNode<'a>) -> SourceFile<'a> {
        debug_assert_eq!(node.kind(), SOURCE_FILE);
        SourceFile(node)
    }

    /// The `ScriptName` declaration.
    pub fn header(&self) -> Option<ScriptHeader<'a>> {
        child(self.0)
    }

    /// All top-level declarations, the header included.
    pub fn items(&self) -> impl Iterator<Item = Item<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> ScriptHeader<'a> {
    /// The script name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// The parent script, from `extends Parent`.
    pub fn extends(&self) -> Option<NameRef<'a>> {
        self.0.child_node(EXTENDS_CLAUSE).and_then(child)
    }

    /// Trailing flags.
    pub fn flags(&self) -> impl Iterator<Item = Flag<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> Import<'a> {
    /// The imported script.
    pub fn name_ref(&self) -> Option<NameRef<'a>> {
        child(self.0)
    }
}

impl<'a> VarDecl<'a> {
    /// The declared type.
    pub fn ty(&self) -> Option<Type<'a>> {
        child(self.0)
    }

    /// The variable name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// The initial value.
    pub fn initializer(&self) -> Option<Expr<'a>> {
        child::<Initializer<'a>>(self.0).and_then(|i| i.value())
    }

    /// Trailing flags (script variables only).
    pub fn flags(&self) -> impl Iterator<Item = Flag<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> Property<'a> {
    /// The property type.
    pub fn ty(&self) -> Option<Type<'a>> {
        child(self.0)
    }

    /// The property name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// The default value.
    pub fn initializer(&self) -> Option<Expr<'a>> {
        child::<Initializer<'a>>(self.0).and_then(|i| i.value())
    }

    /// Trailing flags.
    pub fn flags(&self) -> impl Iterator<Item = Flag<'a>> + use<'a> {
        children(self.0)
    }

    /// `Auto` or `AutoReadOnly` property, without accessor functions.
    pub fn is_auto(&self) -> bool {
        has_flag(self.0, "Auto") || has_flag(self.0, "AutoReadOnly")
    }

    /// `AutoReadOnly` property.
    pub fn is_read_only(&self) -> bool {
        has_flag(self.0, "AutoReadOnly")
    }

    /// The `Get`/`Set` accessors of a full property.
    pub fn functions(&self) -> impl Iterator<Item = Function<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> State<'a> {
    /// The state name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// `Auto State`: the initial state of the script.
    pub fn is_auto(&self) -> bool {
        self.0.child_token(AUTO_KW).is_some()
    }

    /// Declarations inside the state.
    pub fn items(&self) -> impl Iterator<Item = Item<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> Function<'a> {
    /// The return type, `None` for functions returning nothing.
    pub fn return_type(&self) -> Option<Type<'a>> {
        child(self.0)
    }

    /// The function name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// The parameters.
    pub fn param_list(&self) -> Option<ParamList<'a>> {
        child(self.0)
    }

    /// Trailing flags.
    pub fn flags(&self) -> impl Iterator<Item = Flag<'a>> + use<'a> {
        children(self.0)
    }

    /// The body, absent for native functions.
    pub fn body(&self) -> Option<Block<'a>> {
        child(self.0)
    }

    /// `Global` function: no `Self`, callable as `Script.Function()`.
    pub fn is_global(&self) -> bool {
        has_flag(self.0, "Global")
    }

    /// `Native` function, implemented by the game engine.
    pub fn is_native(&self) -> bool {
        has_flag(self.0, "Native")
    }
}

impl<'a> Event<'a> {
    /// The event name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// The parameters.
    pub fn param_list(&self) -> Option<ParamList<'a>> {
        child(self.0)
    }

    /// Trailing flags.
    pub fn flags(&self) -> impl Iterator<Item = Flag<'a>> + use<'a> {
        children(self.0)
    }

    /// The body, absent for native events.
    pub fn body(&self) -> Option<Block<'a>> {
        child(self.0)
    }

    /// `Native` event.
    pub fn is_native(&self) -> bool {
        has_flag(self.0, "Native")
    }
}

impl<'a> ParamList<'a> {
    /// The parameters.
    pub fn params(&self) -> impl Iterator<Item = Param<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> Param<'a> {
    /// The parameter type.
    pub fn ty(&self) -> Option<Type<'a>> {
        child(self.0)
    }

    /// The parameter name.
    pub fn name(&self) -> Option<Name<'a>> {
        child(self.0)
    }

    /// The default value, which makes the parameter optional.
    pub fn default_value(&self) -> Option<Expr<'a>> {
        child::<Initializer<'a>>(self.0).and_then(|i| i.value())
    }
}

impl<'a> Initializer<'a> {
    /// The value.
    pub fn value(&self) -> Option<Expr<'a>> {
        child(self.0)
    }
}

// === Statements ===

ast_node!(
    /// A sequence of statements.
    Block = BLOCK
);
ast_node!(
    /// `target op value`, with `op` one of `=`, `+=`, `-=`, `*=`, `/=`, `%=`.
    AssignStmt = ASSIGN_STMT
);
ast_node!(
    /// An expression evaluated for its side effects, usually a call.
    ExprStmt = EXPR_STMT
);
ast_node!(
    /// `Return value`
    ReturnStmt = RETURN_STMT
);
ast_node!(
    /// `If cond ... ElseIf cond ... Else ... EndIf`
    IfStmt = IF_STMT
);
ast_node!(
    /// `ElseIf cond ...`
    ElseIfClause = ELSE_IF_CLAUSE
);
ast_node!(
    /// `Else ...`
    ElseClause = ELSE_CLAUSE
);
ast_node!(
    /// `While cond ... EndWhile`
    WhileStmt = WHILE_STMT
);

ast_enum!(
    /// A statement.
    Stmt {
        VarDecl(VarDecl),
        Assign(AssignStmt),
        Expr(ExprStmt),
        Return(ReturnStmt),
        If(IfStmt),
        While(WhileStmt),
    }
);

impl<'a> Block<'a> {
    /// The statements.
    pub fn stmts(&self) -> impl Iterator<Item = Stmt<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> AssignStmt<'a> {
    /// The assigned place.
    pub fn target(&self) -> Option<Expr<'a>> {
        children(self.0).next()
    }

    /// The assigned value.
    pub fn value(&self) -> Option<Expr<'a>> {
        children(self.0).nth(1)
    }

    /// The operator token.
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_tokens().find(|t| t.kind().is_assign_op())
    }
}

impl<'a> ExprStmt<'a> {
    /// The expression.
    pub fn expr(&self) -> Option<Expr<'a>> {
        child(self.0)
    }
}

impl<'a> ReturnStmt<'a> {
    /// The returned value.
    pub fn expr(&self) -> Option<Expr<'a>> {
        child(self.0)
    }
}

impl<'a> IfStmt<'a> {
    /// The condition.
    pub fn condition(&self) -> Option<Expr<'a>> {
        child(self.0)
    }

    /// The statements executed when the condition holds.
    pub fn then_block(&self) -> Option<Block<'a>> {
        child(self.0)
    }

    /// The `ElseIf` clauses.
    pub fn else_ifs(&self) -> impl Iterator<Item = ElseIfClause<'a>> + use<'a> {
        children(self.0)
    }

    /// The `Else` clause.
    pub fn else_clause(&self) -> Option<ElseClause<'a>> {
        child(self.0)
    }
}

impl<'a> ElseIfClause<'a> {
    /// The condition.
    pub fn condition(&self) -> Option<Expr<'a>> {
        child(self.0)
    }

    /// The statements.
    pub fn block(&self) -> Option<Block<'a>> {
        child(self.0)
    }
}

impl<'a> ElseClause<'a> {
    /// The statements.
    pub fn block(&self) -> Option<Block<'a>> {
        child(self.0)
    }
}

impl<'a> WhileStmt<'a> {
    /// The condition.
    pub fn condition(&self) -> Option<Expr<'a>> {
        child(self.0)
    }

    /// The loop body.
    pub fn body(&self) -> Option<Block<'a>> {
        child(self.0)
    }
}

// === Expressions ===

ast_node!(
    /// `lhs op rhs`
    BinExpr = BIN_EXPR
);
ast_node!(
    /// `-x` or `!x`
    PrefixExpr = PREFIX_EXPR
);
ast_node!(
    /// `(x)`
    ParenExpr = PAREN_EXPR
);
ast_node!(
    /// `1`, `1.5`, `"text"`, `True`, `False` or `None`.
    Literal = LITERAL
);
ast_node!(
    /// `receiver.member`
    FieldExpr = FIELD_EXPR
);
ast_node!(
    /// `array[index]`
    IndexExpr = INDEX_EXPR
);
ast_node!(
    /// `callee(args)`, where the callee is a name or a field expression.
    CallExpr = CALL_EXPR
);
ast_node!(
    /// `(a, b = 1)`
    ArgList = ARG_LIST
);
ast_node!(
    /// A positional (`a`) or named (`name = a`) argument.
    Arg = ARG
);
ast_node!(
    /// `x As Type`
    CastExpr = CAST_EXPR
);
ast_node!(
    /// `New Type[size]`
    NewExpr = NEW_EXPR
);

ast_enum!(
    /// An expression.
    Expr {
        Bin(BinExpr),
        Prefix(PrefixExpr),
        Paren(ParenExpr),
        Literal(Literal),
        NameRef(NameRef),
        Field(FieldExpr),
        Index(IndexExpr),
        Call(CallExpr),
        Cast(CastExpr),
        New(NewExpr),
    }
);

impl<'a> BinExpr<'a> {
    /// The left operand.
    pub fn lhs(&self) -> Option<Expr<'a>> {
        children(self.0).next()
    }

    /// The right operand.
    pub fn rhs(&self) -> Option<Expr<'a>> {
        children(self.0).nth(1)
    }

    /// The operator token.
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_tokens().find(|t| !t.kind().is_trivia())
    }
}

impl<'a> PrefixExpr<'a> {
    /// The operand.
    pub fn expr(&self) -> Option<Expr<'a>> {
        child(self.0)
    }

    /// The operator token, `-` or `!`.
    pub fn op(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_tokens().find(|t| matches!(t.kind(), MINUS | BANG))
    }
}

impl<'a> ParenExpr<'a> {
    /// The inner expression.
    pub fn expr(&self) -> Option<Expr<'a>> {
        child(self.0)
    }
}

/// The kind of a [`Literal`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiteralKind {
    /// `42`, `0xFF`
    Int,
    /// `1.5`
    Float,
    /// `"text"`
    String,
    /// `True` or `False`
    Bool,
    /// `None`
    None,
}

impl<'a> Literal<'a> {
    /// The literal token.
    pub fn token(&self) -> Option<SyntaxToken<'a>> {
        self.0.child_tokens().find(|t| !t.kind().is_trivia())
    }

    /// The kind of literal.
    pub fn kind(&self) -> Option<LiteralKind> {
        Some(match self.token()?.kind() {
            INT_NUMBER => LiteralKind::Int,
            FLOAT_NUMBER => LiteralKind::Float,
            STRING => LiteralKind::String,
            TRUE_KW | FALSE_KW => LiteralKind::Bool,
            NONE_KW => LiteralKind::None,
            _ => return None,
        })
    }
}

impl<'a> FieldExpr<'a> {
    /// The expression before the dot.
    pub fn receiver(&self) -> Option<Expr<'a>> {
        let first = self.0.children().next()?;
        // A missing receiver cannot happen: the parser only builds field
        // expressions on top of an existing one.
        Expr::cast(first)
    }

    /// The member name after the dot.
    pub fn name_ref(&self) -> Option<NameRef<'a>> {
        after_token(self.0, DOT).find_map(NameRef::cast)
    }
}

impl<'a> IndexExpr<'a> {
    /// The indexed array.
    pub fn base(&self) -> Option<Expr<'a>> {
        children(self.0).next()
    }

    /// The index.
    pub fn index(&self) -> Option<Expr<'a>> {
        after_token(self.0, L_BRACK).find_map(Expr::cast)
    }
}

impl<'a> CallExpr<'a> {
    /// The called function: a [`NameRef`] or a [`FieldExpr`].
    pub fn callee(&self) -> Option<Expr<'a>> {
        children(self.0).next()
    }

    /// The arguments.
    pub fn arg_list(&self) -> Option<ArgList<'a>> {
        child(self.0)
    }

    /// The name of the called function.
    pub fn callee_name(&self) -> Option<NameRef<'a>> {
        match self.callee()? {
            Expr::NameRef(n) => Some(n),
            Expr::Field(f) => f.name_ref(),
            _ => None,
        }
    }
}

impl<'a> ArgList<'a> {
    /// The arguments.
    pub fn args(&self) -> impl Iterator<Item = Arg<'a>> + use<'a> {
        children(self.0)
    }
}

impl<'a> Arg<'a> {
    /// The parameter name of a named argument.
    pub fn name(&self) -> Option<NameRef<'a>> {
        self.0.child_token(EQ)?;
        child(self.0)
    }

    /// The argument value.
    pub fn value(&self) -> Option<Expr<'a>> {
        if self.0.child_token(EQ).is_some() {
            after_token(self.0, EQ).find_map(Expr::cast)
        } else {
            child(self.0)
        }
    }
}

impl<'a> CastExpr<'a> {
    /// The converted expression.
    pub fn expr(&self) -> Option<Expr<'a>> {
        child(self.0)
    }

    /// The target type.
    pub fn ty(&self) -> Option<Type<'a>> {
        child(self.0)
    }
}

impl<'a> NewExpr<'a> {
    /// The element type.
    pub fn ty(&self) -> Option<Type<'a>> {
        child(self.0)
    }

    /// The array size.
    pub fn size(&self) -> Option<Expr<'a>> {
        child(self.0)
    }
}

/// Child nodes following the first child token of `kind`.
fn after_token<'a>(node: SyntaxNode<'a>, kind: SyntaxKind) -> impl Iterator<Item = SyntaxNode<'a>> {
    node.children_with_tokens()
        .skip_while(move |el| el.kind() != kind)
        .filter_map(SyntaxElement::into_node)
}
