//! Type checking of [`Body`]s.
//!
//! The rules follow the Papyrus compiler (Skyrim):
//!
//! - implicit conversions: `Int` to `Float`, anything to `Bool` and `String`,
//!   objects to their parent scripts, `None` to objects and arrays;
//! - `as` converts between `Bool`, `Int`, `Float` and `String`, and between
//!   related objects (up or down the `extends` chain);
//! - `+` concatenates when either side is a `String`, and otherwise needs
//!   numbers like `-`, `*` and `/`; `%` needs `Int`s; `<`, `<=`, `>`, `>=`
//!   compare numbers or strings;
//! - calls need the right number of arguments, of assignable types;
//! - `Return` needs a value exactly in functions with a return type;
//! - `AutoReadOnly` properties and full properties without `Set` cannot be
//!   assigned;
//! - `==` and `!=` compare objects only when one extends the other;
//! - `New Type[size]` takes an integer literal from 1 to 128;
//! - `Self` and `Parent` do not exist in `Global` functions.
//!
//! The compiler types a call of a function without return type as `None`:
//! `x = voidCall()` and `Return voidCall()` in a function without return
//! type compile, so they are accepted here too.
//!
//! A type that cannot be known (an unresolved name, a script of an incomplete
//! `extends` chain, ...) is [`Ty::Unknown`] and is compatible with everything,
//! so that one unknown name does not cause a cascade of errors.

use papyrus_intern::Name;
use papyrus_parser::TextRange;

use crate::{
    hir::{Arg, BinaryOp, Body, BodyKind, Expr, ExprId, Literal, Stmt, StmtId, UnaryOp},
    index::{BaseType, Flags, MemberKind, TypeRef},
    sema::{Builtin, Def, Sema},
};

/// The type of an expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    /// Not known; compatible with everything.
    Unknown,
    /// The `None` literal, and calls of functions without return type.
    None,
    /// A script used as a receiver, as in `Debug.Trace()`.
    Script(Name),
    /// A value.
    Value(TypeRef),
}

impl Ty {
    const BOOL: Ty = Ty::Value(TypeRef::BOOL);
    const INT: Ty = Ty::Value(TypeRef::INT);
    const FLOAT: Ty = Ty::Value(TypeRef::FLOAT);
    const STRING: Ty = Ty::Value(TypeRef::STRING);

    fn of(ty: Option<TypeRef>) -> Ty {
        ty.map_or(Ty::Unknown, Ty::Value)
    }

    fn is_numeric(self) -> bool {
        matches!(self, Ty::Value(TypeRef { base: BaseType::Int | BaseType::Float, array: false }))
    }

    /// The type as written in scripts, for messages.
    pub fn display(self) -> String {
        match self {
            Ty::Unknown => "?".to_owned(),
            Ty::None => "None".to_owned(),
            Ty::Script(name) => format!("script {name}"),
            Ty::Value(t) => t.to_string(),
        }
    }
}

/// A type error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeError {
    /// The location.
    pub range: TextRange,
    /// A stable identifier of the check.
    pub code: &'static str,
    /// The message.
    pub message: String,
}

/// The result of checking a body: the type of every expression, and errors.
#[derive(Clone, Debug, Default)]
pub struct TypeckResult {
    /// The type of each expression, indexed by [`ExprId`].
    pub types: Vec<Ty>,
    /// The errors.
    pub errors: Vec<TypeError>,
}

/// Type checks a body.
pub fn check_body(sema: &Sema<'_>, body: &Body) -> TypeckResult {
    let mut cx = Checker { sema, body, types: vec![None; body.exprs.len()], errors: Vec::new() };
    for &stmt in &body.root {
        cx.stmt(stmt);
    }
    let types = cx.types.iter().map(|t| t.unwrap_or(Ty::Unknown)).collect();
    TypeckResult { types, errors: cx.errors }
}

/// Checks the initial value of a declaration (a script variable, a property
/// or a parameter default). The compiler only accepts a literal (numbers
/// possibly negated) of exactly the declared type, without conversion: a
/// `Float` cannot be initialized with `1`, nor a `String` with `None`; only
/// objects and arrays take `None`. `AutoReadOnly` properties are the
/// exception (`convert`): their value is returned by a getter, with the
/// implicit conversions of `Return`.
pub fn check_initializer(
    declared: Option<TypeRef>,
    value: papyrus_parser::ast::Expr<'_>,
    convert: bool,
) -> Option<TypeError> {
    use papyrus_parser::ast::{AstNode, Expr, LiteralKind};

    let range = value.syntax().trimmed_range();
    let literal = match value {
        Expr::Literal(lit) => lit.kind(),
        Expr::Prefix(p)
            if p.op().is_some_and(|t| t.kind() == papyrus_parser::SyntaxKind::MINUS) =>
        {
            match p.expr() {
                Some(Expr::Literal(lit))
                    if matches!(lit.kind(), Some(LiteralKind::Int | LiteralKind::Float)) =>
                {
                    lit.kind()
                }
                _ => None,
            }
        }
        _ => None,
    };
    let Some(kind) = literal else {
        return Some(TypeError {
            range,
            code: "non-literal-initializer",
            message: "initial values must be literals".to_owned(),
        });
    };
    let declared = declared?;
    let from = match kind {
        LiteralKind::None => None,
        LiteralKind::Int => Some(TypeRef::INT),
        LiteralKind::Float => Some(TypeRef::FLOAT),
        LiteralKind::String => Some(TypeRef::STRING),
        LiteralKind::Bool => Some(TypeRef::BOOL),
    };
    let exact = match from {
        None => declared.array || declared.object().is_some(),
        Some(t) => t == declared,
    };
    // The implicit conversions to `Bool`, `String` and `Int` to `Float`.
    let converted = convert
        && !declared.array
        && (matches!(declared.base, BaseType::Bool | BaseType::String)
            || (declared == TypeRef::FLOAT && from == Some(TypeRef::INT)));
    let ok = exact || converted;
    (!ok).then(|| TypeError {
        range,
        code: "initializer-mismatch",
        message: format!(
            "cannot initialize a `{declared}` with `{}`: initial values are not converted",
            &value.syntax().source()[range]
        ),
    })
}

struct Checker<'s, 'a> {
    sema: &'s Sema<'a>,
    body: &'s Body,
    types: Vec<Option<Ty>>,
    errors: Vec<TypeError>,
}

impl Checker<'_, '_> {
    fn error(&mut self, range: TextRange, code: &'static str, message: String) {
        self.errors.push(TypeError { range, code, message });
    }

    fn mismatch(&mut self, expr: ExprId, expected: TypeRef, found: Ty) {
        let range = self.body.expr_range(expr);
        self.error(
            range,
            "type-mismatch",
            format!("expected `{expected}`, found `{}`", found.display()),
        );
    }

    // === Statements ===

    fn stmts(&mut self, stmts: &[StmtId]) {
        for &s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, id: StmtId) {
        match self.body.stmt(id).clone() {
            Stmt::Local { local, init } => {
                if let Some(init) = init {
                    let ty = self.expr(init);
                    if let Some(declared) = self.body.local(local).ty {
                        self.expect(init, ty, declared);
                    }
                }
            }
            Stmt::Assign { op, target, value } => self.assign(op, target, value),
            Stmt::Expr(e) => {
                self.expr(e);
            }
            Stmt::Return(value) => self.ret(id, value),
            Stmt::If { branches, else_branch } => {
                for (condition, block) in &branches {
                    self.condition(*condition);
                    self.stmts(block);
                }
                if let Some(block) = &else_branch {
                    self.stmts(block);
                }
            }
            Stmt::While { condition, body } => {
                self.condition(condition);
                self.stmts(&body);
            }
        }
    }

    fn condition(&mut self, cond: ExprId) {
        // Any value converts to `Bool`.
        self.expr(cond);
    }

    fn ret(&mut self, stmt: StmtId, value: Option<ExprId>) {
        let range = self.body.stmt_range(stmt);
        let ty = value.map(|v| (v, self.expr(v)));
        match (self.body.kind, ty) {
            (BodyKind::Function { ret: Some(declared), .. }, Some((v, ty))) => {
                self.expect(v, ty, declared)
            }
            (BodyKind::Function { ret: Some(declared), .. }, None) => {
                self.error(
                    range,
                    "missing-return-value",
                    format!("this function must return a `{declared}`"),
                );
            }
            (BodyKind::Function { ret: None, .. }, Some((v, ty)))
                if !matches!(ty, Ty::None | Ty::Unknown) =>
            {
                let range = self.body.expr_range(v);
                self.error(
                    range,
                    "unexpected-return-value",
                    "this function does not return a value".to_owned(),
                );
            }
            (BodyKind::Event, Some((v, ty))) if !matches!(ty, Ty::None | Ty::Unknown) => {
                let range = self.body.expr_range(v);
                self.error(
                    range,
                    "unexpected-return-value",
                    "events cannot return a value".to_owned(),
                );
            }
            _ => {}
        }
    }

    fn assign(&mut self, op: Option<BinaryOp>, target: ExprId, value: ExprId) {
        let target_ty = self.expr(target);
        let value_ty = self.expr(value);
        self.check_place(target);
        let result = match op {
            None => value_ty,
            Some(op) => self.binary(op, target, target_ty, value, value_ty),
        };
        if let Ty::Value(t) = target_ty {
            self.expect(value, result, t);
        }
    }

    /// Reports assignments to things that are not variables, and to read-only
    /// properties.
    fn check_place(&mut self, target: ExprId) {
        let range = self.body.expr_range(target);
        let def = match self.body.expr(target) {
            Expr::Local(_) | Expr::Index { .. } | Expr::Missing | Expr::Unresolved(_) => return,
            Expr::Def(def) => Some(*def),
            Expr::Field { def, .. } => *def,
            _ => {
                self.error(
                    range,
                    "invalid-assignment",
                    "this expression cannot be assigned".to_owned(),
                );
                return;
            }
        };
        let Some(Def::Member { file, index }) = def else { return };
        let member = self.sema.member(file, index);
        if member.kind != MemberKind::Property {
            return;
        }
        let read_only = member.flags.contains(Flags::AUTO_READONLY)
            || (!member.flags.contains(Flags::AUTO)
                && !self.sema.ws.file(file).index.members.iter().any(|m| {
                    m.property == Some(member.name) && m.name.as_str().eq_ignore_ascii_case("Set")
                }));
        if read_only {
            self.error(range, "read-only", format!("property `{}` is read-only", member.name));
        }
    }

    // === Expressions ===

    fn expect(&mut self, expr: ExprId, found: Ty, expected: TypeRef) {
        if !self.assignable(found, expected) {
            self.mismatch(expr, expected, found);
        }
    }

    fn expr(&mut self, id: ExprId) -> Ty {
        if let Some(ty) = self.types[id.0 as usize] {
            return ty;
        }
        let ty = self.infer(id);
        self.types[id.0 as usize] = Some(ty);
        ty
    }

    fn infer(&mut self, id: ExprId) -> Ty {
        let range = self.body.expr_range(id);
        match self.body.expr(id).clone() {
            Expr::Missing | Expr::Unresolved(_) => Ty::Unknown,
            Expr::Literal(lit) => match lit {
                Literal::Int => Ty::INT,
                Literal::Float => Ty::FLOAT,
                Literal::String => Ty::STRING,
                Literal::Bool => Ty::BOOL,
                Literal::None => Ty::None,
            },
            Expr::Local(local) => Ty::of(self.body.local(local).ty),
            Expr::Def(def) => self.def_value_ty(def),
            Expr::SelfRef | Expr::ParentRef
                if matches!(self.body.kind, BodyKind::Function { global: true, .. }) =>
            {
                let name = if self.body.expr(id) == &Expr::SelfRef { "Self" } else { "Parent" };
                self.error(
                    range,
                    "global-self",
                    format!("`{name}` does not exist in a Global function"),
                );
                Ty::Unknown
            }
            Expr::SelfRef => match self.sema.script_name() {
                Some(name) => Ty::Value(TypeRef::scalar(BaseType::Object(name))),
                None => Ty::Unknown,
            },
            Expr::ParentRef => match self.sema.index.extends {
                Some(name) => Ty::Value(TypeRef::scalar(BaseType::Object(name))),
                None => Ty::Unknown,
            },
            Expr::Field { receiver, def, .. } => {
                let recv = self.expr(receiver);
                match (recv, def) {
                    (_, Some(def)) => self.def_value_ty(def),
                    (Ty::Value(t), None) if !t.array && t.object().is_none() => {
                        self.error(range, "invalid-member", format!("`{t}` has no members"));
                        Ty::Unknown
                    }
                    _ => Ty::Unknown,
                }
            }
            Expr::Call { receiver, def, args, .. } => {
                let recv = receiver.map(|r| self.expr(r));
                self.call(range, recv, def, &args)
            }
            Expr::Index { base, index } => {
                let base_ty = self.expr(base);
                let index_ty = self.expr(index);
                self.expect(index, index_ty, TypeRef::INT);
                match base_ty {
                    Ty::Value(t) if t.array => Ty::Value(t.element().expect("array")),
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        let range = self.body.expr_range(base);
                        self.error(
                            range,
                            "not-an-array",
                            format!("`{}` is not an array", other.display()),
                        );
                        Ty::Unknown
                    }
                }
            }
            Expr::Cast { expr, ty } => {
                let from = self.expr(expr);
                if let Some(to) = ty
                    && !self.castable(from, to)
                {
                    self.error(
                        range,
                        "invalid-cast",
                        format!("cannot cast `{}` to `{to}`", from.display()),
                    );
                }
                Ty::of(ty)
            }
            Expr::New { elem, size } => {
                self.expr(size);
                self.array_size(size);
                match elem {
                    Some(e) => Ty::Value(TypeRef { base: e.base, array: true }),
                    None => Ty::Unknown,
                }
            }
            Expr::Unary { op, expr } => {
                let ty = self.expr(expr);
                match op {
                    UnaryOp::Not => Ty::BOOL,
                    UnaryOp::Neg if ty.is_numeric() || ty == Ty::Unknown => ty,
                    UnaryOp::Neg => {
                        self.error(
                            range,
                            "invalid-operator",
                            format!("cannot negate `{}`", ty.display()),
                        );
                        Ty::Unknown
                    }
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.expr(lhs);
                let r = self.expr(rhs);
                self.binary(op, lhs, l, rhs, r)
            }
        }
    }

    fn binary(&mut self, op: BinaryOp, lhs: ExprId, l: Ty, rhs: ExprId, r: Ty) -> Ty {
        let range = self.body.expr_range(lhs).cover(self.body.expr_range(rhs));
        let unknown = l == Ty::Unknown || r == Ty::Unknown;
        let numeric =
            |l: Ty, r: Ty| if l == Ty::FLOAT || r == Ty::FLOAT { Ty::FLOAT } else { Ty::INT };
        let invalid = |cx: &mut Self| {
            cx.error(
                range,
                "invalid-operator",
                format!(
                    "cannot apply `{}` to `{}` and `{}`",
                    op.as_str(),
                    l.display(),
                    r.display()
                ),
            );
            Ty::Unknown
        };
        match op {
            BinaryOp::Eq | BinaryOp::Ne => {
                // Objects compare only along the `extends` chain.
                if let (Ty::Value(a), Ty::Value(b)) = (l, r)
                    && let (Some(a), Some(b)) = (a.object(), b.object())
                    && self.extends(a, b) == Some(false)
                    && self.extends(b, a) == Some(false)
                {
                    invalid(self);
                }
                Ty::BOOL
            }
            BinaryOp::And | BinaryOp::Or => Ty::BOOL,
            BinaryOp::Add if l == Ty::STRING || r == Ty::STRING => Ty::STRING,
            _ if unknown => match op {
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Ty::BOOL,
                _ if l.is_numeric() || r.is_numeric() => numeric(l, r),
                _ => Ty::Unknown,
            },
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                if l.is_numeric() && r.is_numeric() { numeric(l, r) } else { invalid(self) }
            }
            BinaryOp::Rem => {
                if l == Ty::INT && r == Ty::INT {
                    Ty::INT
                } else {
                    invalid(self)
                }
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                if (l.is_numeric() && r.is_numeric()) || (l == Ty::STRING && r == Ty::STRING) {
                    Ty::BOOL
                } else {
                    invalid(self)
                }
            }
        }
    }

    fn call(&mut self, range: TextRange, recv: Option<Ty>, def: Option<Def>, args: &[Arg]) -> Ty {
        let arg_types: Vec<Ty> = args.iter().map(|a| self.expr(a.value)).collect();
        match def {
            Some(Def::Member { file, index }) => {
                let member = self.sema.member(file, index);
                if !member.is_callable() {
                    return Ty::Unknown;
                }
                if let Some(Ty::Script(script)) = recv
                    && !member.is_global()
                {
                    self.error(
                        range,
                        "not-global",
                        format!("`{}` is not a Global function and needs an object, not the script `{script}`", member.name),
                    );
                }
                let params: Vec<(Name, Option<TypeRef>, bool)> =
                    member.params.iter().map(|p| (p.name, p.ty, p.default.is_some())).collect();
                self.check_args(range, member.name, &params, args, &arg_types);
                if member.kind == MemberKind::Event {
                    Ty::None
                } else {
                    member.ty.map_or(Ty::None, Ty::Value)
                }
            }
            Some(Def::Builtin(b, elem)) => {
                let params: Vec<(Name, Option<TypeRef>, bool)> = match b {
                    Builtin::Find | Builtin::RFind => {
                        vec![
                            (Name::new("akElement"), elem, false),
                            (Name::new("aiStartIndex"), Some(TypeRef::INT), true),
                        ]
                    }
                    Builtin::GoToState => {
                        vec![(Name::new("asNewState"), Some(TypeRef::STRING), false)]
                    }
                    _ => Vec::new(),
                };
                self.check_args(range, Name::new(b.name()), &params, args, &arg_types);
                b.ty().map_or(Ty::None, Ty::Value)
            }
            _ => Ty::Unknown,
        }
    }

    fn check_args(
        &mut self,
        range: TextRange,
        name: Name,
        params: &[(Name, Option<TypeRef>, bool)],
        args: &[Arg],
        arg_types: &[Ty],
    ) {
        let mut given = vec![false; params.len()];
        let mut positional = 0;
        for (arg, &ty) in args.iter().zip(arg_types) {
            let slot = match arg.name {
                Some(n) => match params.iter().position(|p| p.0 == n) {
                    Some(i) => i,
                    // Unknown parameter names are reported by the name checks.
                    None => continue,
                },
                None => {
                    positional += 1;
                    positional - 1
                }
            };
            let Some(param) = params.get(slot) else {
                self.error(
                    arg.range,
                    "wrong-arg-count",
                    format!("`{name}` takes {} arguments, but more were given", params.len()),
                );
                continue;
            };
            given[slot] = true;
            if let Some(expected) = param.1 {
                self.expect(arg.value, ty, expected);
            }
        }
        let missing: Vec<&str> = params
            .iter()
            .zip(&given)
            .filter(|(p, g)| !**g && !p.2)
            .map(|(p, _)| p.0.as_str())
            .collect();
        if !missing.is_empty() {
            self.error(
                range,
                "wrong-arg-count",
                format!("missing arguments for `{name}`: {}", missing.join(", ")),
            );
        }
    }

    fn def_value_ty(&self, def: Def) -> Ty {
        match def {
            Def::Local { ty, .. } => Ty::of(ty),
            Def::Member { file, index } => {
                let m = self.sema.member(file, index);
                match m.kind {
                    MemberKind::Property | MemberKind::Variable => Ty::of(m.ty),
                    _ => Ty::Unknown,
                }
            }
            Def::Param { file, member, param } => {
                Ty::of(self.sema.member(file, member).params[param as usize].ty)
            }
            Def::Script { file } => {
                self.sema.ws.file(file).index.name.map_or(Ty::Unknown, Ty::Script)
            }
            Def::Builtin(b, _) if !b.is_callable() => b.ty().map_or(Ty::Unknown, Ty::Value),
            _ => Ty::Unknown,
        }
    }

    /// The size of `New Type[size]` must be an integer literal from 1 to 128.
    fn array_size(&mut self, size: ExprId) {
        let range = self.body.expr_range(size);
        let value = match self.body.expr(size) {
            Expr::Literal(Literal::Int) => {
                let text = self.sema.text.get(usize::from(range.start())..usize::from(range.end()));
                text.and_then(|t| match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(hex) => i64::from_str_radix(hex, 16).ok(),
                    None => t.parse::<i64>().ok(),
                })
            }
            Expr::Missing => return,
            _ => None,
        };
        match value {
            Some(1..=128) => {}
            Some(_) => self.error(
                range,
                "array-size",
                "arrays must have between 1 and 128 elements".to_owned(),
            ),
            None => self.error(
                range,
                "array-size",
                "the size of a new array must be an integer literal (1 to 128)".to_owned(),
            ),
        }
    }

    // === Type relations ===

    /// Whether `script` is `ancestor` or extends it. `None` when the chain is
    /// not fully known.
    fn extends(&self, script: Name, ancestor: Name) -> Option<bool> {
        let mut len = 0;
        for (_, index) in self.sema.chain(script) {
            len += 1;
            if index.name == Some(ancestor) {
                return Some(true);
            }
        }
        (len > 0 && self.sema.chain_is_complete(script)).then_some(false)
    }

    /// Implicit conversions.
    fn assignable(&self, from: Ty, to: TypeRef) -> bool {
        let from = match from {
            Ty::Unknown => return true,
            Ty::Script(_) => return false,
            Ty::None => {
                return to.array
                    || to.object().is_some()
                    || matches!(to.base, BaseType::Bool | BaseType::String);
            }
            Ty::Value(t) => t,
        };
        if from == to {
            return true;
        }
        if !to.array {
            match to.base {
                BaseType::Bool | BaseType::String => return true,
                BaseType::Float if !from.array && from.base == BaseType::Int => return true,
                _ => {}
            }
        }
        match (from.base, to.base) {
            (BaseType::Object(a), BaseType::Object(b)) if from.array == to.array => {
                self.extends(a, b).unwrap_or(true)
            }
            _ => false,
        }
    }

    /// Explicit conversions with `as`.
    fn castable(&self, from: Ty, to: TypeRef) -> bool {
        let from = match from {
            Ty::Unknown => return true,
            Ty::Script(_) => return false,
            Ty::None => return true,
            Ty::Value(t) => t,
        };
        if self.assignable(Ty::Value(from), to) {
            return true;
        }
        if from.array || to.array {
            return false;
        }
        let primitive = |b: BaseType| !matches!(b, BaseType::Object(_));
        match (from.base, to.base) {
            (a, b) if primitive(a) && primitive(b) => true,
            // Down casts, checked at run time.
            (BaseType::Object(a), BaseType::Object(b)) => self.extends(b, a).unwrap_or(true),
            _ => false,
        }
    }
}
