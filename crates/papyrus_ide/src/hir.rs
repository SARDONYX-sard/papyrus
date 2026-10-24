//! High-level IR of function and event bodies.
//!
//! A [`Body`] is the result of lowering the syntax of one function or event:
//! expressions and statements live in arenas indexed by [`ExprId`] and
//! [`StmtId`], names are resolved ([`Def`]) and local variables are
//! numbered ([`LocalId`]). Every node keeps its source range, so analyses on
//! the HIR (such as [type checking](crate::typeck)) report precise
//! locations. Syntax errors lower to [`Expr::Missing`].

use papyrus_intern::Name;
use papyrus_parser::{
    SyntaxKind, SyntaxNode, TextRange,
    ast::{self, AstNode},
};

use crate::{
    index::TypeRef,
    sema::{Def, Sema},
};

/// Index of an expression of a [`Body`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

/// Index of a statement of a [`Body`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StmtId(pub u32);

/// Index of a local variable or parameter of a [`Body`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

/// A literal value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Literal {
    /// `42`
    Int,
    /// `1.5`
    Float,
    /// `"text"`
    String,
    /// `True`, `False`
    Bool,
    /// `None`
    None,
}

/// A unary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-x`
    Neg,
    /// `!x`
    Not,
}

/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
}

impl BinaryOp {
    fn from_token(kind: SyntaxKind) -> Option<BinaryOp> {
        use SyntaxKind::*;
        Some(match kind {
            PLUS | PLUSEQ => BinaryOp::Add,
            MINUS | MINUSEQ => BinaryOp::Sub,
            STAR | STAREQ => BinaryOp::Mul,
            SLASH | SLASHEQ => BinaryOp::Div,
            PERCENT | PERCENTEQ => BinaryOp::Rem,
            EQ2 => BinaryOp::Eq,
            NEQ => BinaryOp::Ne,
            LT => BinaryOp::Lt,
            LTEQ => BinaryOp::Le,
            GT => BinaryOp::Gt,
            GTEQ => BinaryOp::Ge,
            AMP2 => BinaryOp::And,
            PIPE2 => BinaryOp::Or,
            _ => return None,
        })
    }

    /// The operator as written.
    pub fn as_str(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

/// An argument of a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arg {
    /// The parameter name of a named argument.
    pub name: Option<Name>,
    /// The value.
    pub value: ExprId,
    /// The range of the whole argument.
    pub range: TextRange,
}

/// An expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// Absent or unparsable.
    Missing,
    /// A literal.
    Literal(Literal),
    /// A local variable or parameter.
    Local(LocalId),
    /// A name resolved outside the body: a script member, a script used as a
    /// receiver, or a built-in.
    Def(Def),
    /// A name that does not resolve.
    Unresolved(Name),
    /// `Self`
    SelfRef,
    /// `Parent`
    ParentRef,
    /// `receiver.name`, `def` being the resolved member.
    Field {
        /// The receiver.
        receiver: ExprId,
        /// The member name.
        name: Name,
        /// The member, when it resolves.
        def: Option<Def>,
    },
    /// `callee(args)` or `receiver.callee(args)`.
    Call {
        /// The receiver of a method call.
        receiver: Option<ExprId>,
        /// The called name.
        name: Name,
        /// The function, when it resolves.
        def: Option<Def>,
        /// The arguments.
        args: Vec<Arg>,
    },
    /// `base[index]`
    Index {
        /// The array.
        base: ExprId,
        /// The index.
        index: ExprId,
    },
    /// `expr as Type`
    Cast {
        /// The converted value.
        expr: ExprId,
        /// The target type.
        ty: Option<TypeRef>,
    },
    /// `New Type[size]`
    New {
        /// The element type.
        elem: Option<TypeRef>,
        /// The size.
        size: ExprId,
    },
    /// `-x`, `!x`
    Unary {
        /// The operator.
        op: UnaryOp,
        /// The operand.
        expr: ExprId,
    },
    /// `lhs op rhs`
    Binary {
        /// The operator.
        op: BinaryOp,
        /// The left operand.
        lhs: ExprId,
        /// The right operand.
        rhs: ExprId,
    },
}

/// A statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    /// `Type name = init`
    Local {
        /// The declared variable.
        local: LocalId,
        /// The initial value.
        init: Option<ExprId>,
    },
    /// `target op= value`; `op` is `None` for a plain `=`.
    Assign {
        /// The compound operator.
        op: Option<BinaryOp>,
        /// The assigned place.
        target: ExprId,
        /// The value.
        value: ExprId,
    },
    /// An expression evaluated for its effects.
    Expr(ExprId),
    /// `Return value`
    Return(Option<ExprId>),
    /// `If` with its `ElseIf` branches, and the `Else` statements.
    If {
        /// `(condition, statements)` of `If` and `ElseIf`.
        branches: Vec<(ExprId, Vec<StmtId>)>,
        /// The `Else` statements.
        else_branch: Option<Vec<StmtId>>,
    },
    /// `While condition ... EndWhile`
    While {
        /// The condition.
        condition: ExprId,
        /// The loop body.
        body: Vec<StmtId>,
    },
}

/// A local variable or parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalData {
    /// The name.
    pub name: Name,
    /// The declared type.
    pub ty: Option<TypeRef>,
    /// The range of the declaring name.
    pub range: TextRange,
    /// Parameters come first in [`Body::locals`].
    pub is_param: bool,
}

/// What kind of callable a body belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// A function, with its return type.
    Function {
        /// The return type, `None` for functions returning nothing.
        ret: Option<TypeRef>,
        /// `Global` function: no `Self`.
        global: bool,
    },
    /// An event.
    Event,
}

/// The lowered body of a function or event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    /// Function or event.
    pub kind: BodyKind,
    /// Expressions, with their source range.
    pub exprs: Vec<(Expr, TextRange)>,
    /// Statements, with their source range.
    pub stmts: Vec<(Stmt, TextRange)>,
    /// Parameters, then local variables, in declaration order.
    pub locals: Vec<LocalData>,
    /// Top level statements.
    pub root: Vec<StmtId>,
}

impl Body {
    /// An expression.
    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize].0
    }

    /// The source range of an expression.
    pub fn expr_range(&self, id: ExprId) -> TextRange {
        self.exprs[id.0 as usize].1
    }

    /// A statement.
    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.0 as usize].0
    }

    /// The source range of a statement.
    pub fn stmt_range(&self, id: StmtId) -> TextRange {
        self.stmts[id.0 as usize].1
    }

    /// A local variable.
    pub fn local(&self, id: LocalId) -> &LocalData {
        &self.locals[id.0 as usize]
    }

    /// Lowers a `FUNCTION` or `EVENT` node. Native callables have no body
    /// and return `None`.
    pub fn lower(sema: &Sema<'_>, node: SyntaxNode<'_>) -> Option<Body> {
        let (kind, params, block) = if let Some(f) = ast::Function::cast(node) {
            let kind = BodyKind::Function {
                ret: f.return_type().and_then(TypeRef::from_ast),
                global: f.is_global(),
            };
            (kind, f.param_list(), f.body()?)
        } else {
            let e = ast::Event::cast(node)?;
            (BodyKind::Event, e.param_list(), e.body()?)
        };
        let mut lower = Lower {
            sema,
            body: Body {
                kind,
                exprs: Vec::new(),
                stmts: Vec::new(),
                locals: Vec::new(),
                root: Vec::new(),
            },
        };
        for p in params.iter().flat_map(|l| l.params()) {
            let Some(name) = p.name() else { continue };
            lower.body.locals.push(LocalData {
                name: Name::new(name.text()),
                ty: p.ty().and_then(TypeRef::from_ast),
                range: name.syntax().text_range(),
                is_param: true,
            });
        }
        // Locals are hoisted to the start of the body by the compiler; declare
        // them all first so that uses before the declaration resolve too.
        for var in block.syntax().descendants().filter_map(ast::VarDecl::cast) {
            let Some(name) = var.name() else { continue };
            lower.body.locals.push(LocalData {
                name: Name::new(name.text()),
                ty: var.ty().and_then(TypeRef::from_ast),
                range: name.syntax().text_range(),
                is_param: false,
            });
        }
        let root = lower.block(Some(block));
        lower.body.root = root;
        Some(lower.body)
    }
}

struct Lower<'s, 'a> {
    sema: &'s Sema<'a>,
    body: Body,
}

impl<'a> Lower<'_, 'a> {
    fn alloc_expr(&mut self, expr: Expr, range: TextRange) -> ExprId {
        self.body.exprs.push((expr, range));
        ExprId(self.body.exprs.len() as u32 - 1)
    }

    fn alloc_stmt(&mut self, stmt: Stmt, range: TextRange) -> StmtId {
        self.body.stmts.push((stmt, range));
        StmtId(self.body.stmts.len() as u32 - 1)
    }

    fn missing(&mut self, range: TextRange) -> ExprId {
        self.alloc_expr(Expr::Missing, range)
    }

    fn local_at(&self, range: TextRange) -> Option<LocalId> {
        self.body.locals.iter().position(|l| l.range == range).map(|i| LocalId(i as u32))
    }

    fn block(&mut self, block: Option<ast::Block<'a>>) -> Vec<StmtId> {
        let Some(block) = block else { return Vec::new() };
        block.stmts().map(|s| self.stmt(s)).collect()
    }

    fn stmt(&mut self, stmt: ast::Stmt<'a>) -> StmtId {
        let range = stmt.syntax().trimmed_range();
        let lowered = match stmt {
            ast::Stmt::VarDecl(var) => {
                let local = var.name().and_then(|n| self.local_at(n.syntax().text_range()));
                let init = var.initializer().map(|e| self.expr(Some(e)));
                match local {
                    Some(local) => Stmt::Local { local, init },
                    None => Stmt::Expr(init.unwrap_or_else(|| self.missing(range))),
                }
            }
            ast::Stmt::Assign(a) => {
                let op = a.op().and_then(|t| BinaryOp::from_token(t.kind()));
                let target = self.expr(a.target());
                let value = self.expr(a.value());
                Stmt::Assign { op, target, value }
            }
            ast::Stmt::Expr(e) => Stmt::Expr(self.expr(e.expr())),
            ast::Stmt::Return(r) => Stmt::Return(r.expr().map(|e| self.expr(Some(e)))),
            ast::Stmt::If(i) => {
                let mut branches = vec![(self.expr(i.condition()), self.block(i.then_block()))];
                for clause in i.else_ifs() {
                    branches.push((self.expr(clause.condition()), self.block(clause.block())));
                }
                let else_branch = i.else_clause().map(|c| self.block(c.block()));
                Stmt::If { branches, else_branch }
            }
            ast::Stmt::While(w) => {
                let condition = self.expr(w.condition());
                Stmt::While { condition, body: self.block(w.body()) }
            }
        };
        self.alloc_stmt(lowered, range)
    }

    fn expr(&mut self, expr: Option<ast::Expr<'a>>) -> ExprId {
        let Some(expr) = expr else { return self.missing(TextRange::default()) };
        let range = expr.syntax().trimmed_range();
        let lowered = match expr {
            ast::Expr::Literal(lit) => match lit.kind() {
                Some(ast::LiteralKind::Int) => Expr::Literal(Literal::Int),
                Some(ast::LiteralKind::Float) => Expr::Literal(Literal::Float),
                Some(ast::LiteralKind::String) => Expr::Literal(Literal::String),
                Some(ast::LiteralKind::Bool) => Expr::Literal(Literal::Bool),
                Some(ast::LiteralKind::None) => Expr::Literal(Literal::None),
                None => Expr::Missing,
            },
            ast::Expr::NameRef(nr) if nr.is_self() => Expr::SelfRef,
            ast::Expr::NameRef(nr) if nr.is_parent() => Expr::ParentRef,
            ast::Expr::NameRef(nr) => self.name_ref(nr),
            ast::Expr::Paren(p) => return self.expr(p.expr()),
            ast::Expr::Field(f) => {
                let receiver = self.expr(f.receiver());
                let name_ref = f.name_ref();
                let name = Name::new(name_ref.map_or("", |n| n.text()));
                let def = name_ref.and_then(|n| self.sema.resolve_name_ref(n));
                Expr::Field { receiver, name, def }
            }
            ast::Expr::Call(call) => {
                let (receiver, name_ref) = match call.callee() {
                    Some(ast::Expr::Field(f)) => (Some(self.expr(f.receiver())), f.name_ref()),
                    Some(ast::Expr::NameRef(nr)) => (None, Some(nr)),
                    _ => (None, None),
                };
                let name = Name::new(name_ref.map_or("", |n| n.text()));
                let def = name_ref.and_then(|n| self.sema.resolve_name_ref(n));
                let args = call
                    .arg_list()
                    .into_iter()
                    .flat_map(|l| l.args())
                    .map(|a| Arg {
                        name: a.name().map(|n| Name::new(n.text())),
                        value: self.expr(a.value()),
                        range: a.syntax().trimmed_range(),
                    })
                    .collect();
                Expr::Call { receiver, name, def, args }
            }
            ast::Expr::Index(i) => {
                let base = self.expr(i.base());
                let index = self.expr(i.index());
                Expr::Index { base, index }
            }
            ast::Expr::Cast(c) => {
                let expr = self.expr(c.expr());
                Expr::Cast { expr, ty: c.ty().and_then(TypeRef::from_ast) }
            }
            ast::Expr::New(n) => {
                let size = self.expr(n.size());
                Expr::New { elem: n.ty().and_then(TypeRef::from_ast), size }
            }
            ast::Expr::Prefix(p) => {
                let op = match p.op().map(|t| t.kind()) {
                    Some(SyntaxKind::BANG) => UnaryOp::Not,
                    _ => UnaryOp::Neg,
                };
                Expr::Unary { op, expr: self.expr(p.expr()) }
            }
            ast::Expr::Bin(b) => {
                let lhs = self.expr(b.lhs());
                let rhs = self.expr(b.rhs());
                match b.op().and_then(|t| BinaryOp::from_token(t.kind())) {
                    Some(op) => Expr::Binary { op, lhs, rhs },
                    None => Expr::Missing,
                }
            }
        };
        self.alloc_expr(lowered, range)
    }

    fn name_ref(&mut self, nr: ast::NameRef<'a>) -> Expr {
        match self.sema.resolve_name_ref(nr) {
            Some(Def::Local { range, .. }) => match self.local_at(range) {
                Some(local) => Expr::Local(local),
                None => Expr::Unresolved(Name::new(nr.text())),
            },
            Some(def) => Expr::Def(def),
            None => Expr::Unresolved(Name::new(nr.text())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{Workspace, sema::FileSource};

    fn lower(src: &str) -> Body {
        let mut ws = Workspace::new();
        let file = ws.set_open("/ws/T.psc".into(), Arc::from(src), 0);
        let source = FileSource::load(&ws, file).unwrap();
        let sema = source.sema(&ws, file);
        let node = sema.root.descendants().find(|n| n.kind() == SyntaxKind::FUNCTION).unwrap();
        Body::lower(&sema, node).unwrap()
    }

    #[test]
    fn lowers_statements_and_locals() {
        let body = lower(
            "ScriptName T\nInt counter\nInt Function F(Int a)\n  Int b = a + counter\n  If b > 1\n    b -= 1\n  Else\n    Return Foo(x = 1)\n  EndIf\n  Return (b as Float) as Int\nEndFunction\n",
        );
        assert_eq!(body.locals.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(body.locals[0].is_param);
        assert_eq!(body.root.len(), 3);
        let Stmt::Local { init: Some(init), .. } = body.stmt(body.root[0]) else { panic!() };
        let Expr::Binary { op: BinaryOp::Add, lhs, rhs } = body.expr(*init) else { panic!() };
        assert_eq!(body.expr(*lhs), &Expr::Local(LocalId(0)));
        assert!(matches!(body.expr(*rhs), Expr::Def(Def::Member { .. })));
        let Stmt::If { branches, else_branch: Some(else_branch) } = body.stmt(body.root[1]) else {
            panic!()
        };
        assert_eq!(branches.len(), 1);
        let Stmt::Return(Some(call)) = body.stmt(else_branch[0]) else { panic!() };
        let Expr::Call { def: None, args, .. } = body.expr(*call) else { panic!() };
        assert_eq!(args[0].name.unwrap().as_str(), "x");
    }
}
