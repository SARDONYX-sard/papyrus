//! Code generation of function bodies: HIR to [`FunctionIr`].
//!
//! The generated code follows the conventions of the official compiler:
//!
//! - every intermediate value lives in a temporary `::tempN`, numbered per
//!   script in source order; temporaries are released once their value is
//!   used and recycled for later values of the same type;
//! - calls without result write to `::NoneVar`;
//! - implicit conversions (`Int` to `Float`, anything to `String` or `Bool`)
//!   are explicit `CAST`s;
//! - `&&` and `||` short-circuit, casting both sides to `Bool`;
//! - auto properties of the compiled script are accessed through their
//!   hidden variable, other properties with `PROPGET` / `PROPSET`;
//! - values stored into properties and array elements go through a
//!   temporary;
//! - every `If` branch ends with a jump to the end, even after `Return`.
//!
//! The local table lists, per block, the temporaries first created in it,
//! then the variables it declares, then the nested blocks.

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use papyrus_ide::{
    LineIndex, TextRange,
    hir::{Arg, BinaryOp, Body, Expr, ExprId, Literal, Stmt, StmtId, UnaryOp},
    index::{Flags, MemberKind, TypeRef},
    sema::{Builtin, Def, Sema},
    typeck::{Ty, TypeckResult},
};
use papyrus_intern::Name;
use papyrus_pex::pex::OpCode;

use crate::{
    ir::{FunctionIr, Inst, Label, Operand, Slot},
    literal,
};

/// The state shared by the functions of one script.
pub(crate) struct ScriptCx<'a> {
    pub(crate) sema: Sema<'a>,
    pub(crate) lines: LineIndex,
    /// The next temporary number.
    pub(crate) temps: u32,
}

impl ScriptCx<'_> {
    /// The one-based line of an offset.
    pub(crate) fn line(&self, range: TextRange) -> u32 {
        self.lines.line_col(self.sema.text, range.start()).line + 1
    }
}

/// The type of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VTy {
    /// Not known; only in code with errors.
    Unknown,
    /// `None`, and results of calls without return type.
    None,
    /// A value.
    Value(TypeRef),
}

impl VTy {
    const BOOL: VTy = VTy::Value(TypeRef::BOOL);
    const INT: VTy = VTy::Value(TypeRef::INT);
    const FLOAT: VTy = VTy::Value(TypeRef::FLOAT);
    const STRING: VTy = VTy::Value(TypeRef::STRING);

    fn of(ty: Ty) -> VTy {
        match ty {
            Ty::Value(t) => VTy::Value(t),
            Ty::None => VTy::None,
            Ty::Unknown | Ty::Script(_) => VTy::Unknown,
        }
    }

    fn of_const(op: &Operand) -> VTy {
        match op {
            Operand::Int(_) => VTy::INT,
            Operand::Float(_) => VTy::FLOAT,
            Operand::Bool(_) => VTy::BOOL,
            Operand::Str(_) => VTy::STRING,
            Operand::None | Operand::Var(_) => VTy::None,
        }
    }

    /// The type name used in the `.pex` file.
    pub(crate) fn name(self) -> String {
        match self {
            VTy::Value(t) => t.to_string(),
            VTy::None | VTy::Unknown => "None".to_owned(),
        }
    }

    fn is(self, other: VTy) -> bool {
        self.name().eq_ignore_ascii_case(&other.name())
    }
}

/// The name of a type in the `.pex` file.
pub(crate) fn type_name(ty: Option<TypeRef>) -> String {
    ty.map_or_else(|| "None".to_owned(), |t| t.to_string())
}

/// Converts a constant to `to` at compile time, like the official compiler
/// does for default values. `None` when the conversion is not exact.
pub(crate) fn convert_const(value: &Operand, to: &str) -> Option<Operand> {
    let to = to.to_ascii_lowercase();
    Some(match (value, to.as_str()) {
        (Operand::Var(_), _) => return None,
        (Operand::Int(i), "float") => Operand::Float(*i as f32),
        (Operand::Float(f), "int") => Operand::Int(*f as i32),
        (Operand::Int(i), "string") => Operand::Str(i.to_string()),
        (Operand::Bool(b), "string") => Operand::Str(if *b { "True" } else { "False" }.to_owned()),
        (Operand::None, "string") => Operand::Str("None".to_owned()),
        (Operand::Int(i), "bool") => Operand::Bool(*i != 0),
        (Operand::Float(f), "bool") => Operand::Bool(*f != 0.0),
        (Operand::Str(s), "bool") => Operand::Bool(!s.is_empty()),
        (Operand::None, "bool") => Operand::Bool(false),
        (Operand::Bool(b), "int") => Operand::Int(*b as i32),
        (Operand::Bool(b), "float") => Operand::Float(*b as i32 as f32),
        (v @ Operand::Int(_), "int")
        | (v @ Operand::Float(_), "float")
        | (v @ Operand::Str(_), "string")
        | (v @ Operand::Bool(_), "bool") => v.clone(),
        (Operand::None, t) if !matches!(t, "int" | "float") => Operand::None,
        _ => return None,
    })
}

/// A computed value.
#[derive(Clone, Debug)]
struct Val {
    op: Operand,
    ty: VTy,
    /// The value is in a temporary owned by the consumer.
    temp: bool,
}

impl Val {
    fn constant(op: Operand, ty: VTy) -> Val {
        Val { op, ty, temp: false }
    }

    fn var(name: impl Into<String>, ty: VTy) -> Val {
        Val { op: Operand::Var(name.into()), ty, temp: false }
    }

    fn temp(name: String, ty: VTy) -> Val {
        Val { op: Operand::Var(name), ty, temp: true }
    }
}

#[derive(Default)]
struct Scope {
    temps: Vec<Slot>,
    locals: Vec<Slot>,
    children: Vec<usize>,
}

/// A parameter as seen by a call.
struct ParamSig {
    name: Name,
    ty: Option<TypeRef>,
    default: Option<Operand>,
}

/// Generates the code of one body.
pub(crate) fn function(cx: &mut ScriptCx<'_>, body: &Body, types: &TypeckResult) -> FunctionIr {
    let ret = match body.kind {
        papyrus_ide::hir::BodyKind::Function { ret, .. } => ret,
        papyrus_ide::hir::BodyKind::Event => None,
    };
    let mut g = FnGen {
        cx,
        body,
        types,
        ret,
        code: Vec::new(),
        line: 0,
        labels: 0,
        free: FxHashMap::default(),
        scopes: vec![Scope::default()],
        current: 0,
        none_var: false,
        declared: FxHashSet::default(),
    };
    for &stmt in &body.root {
        g.stmt(stmt);
    }
    let params = body
        .locals
        .iter()
        .filter(|l| l.is_param)
        .map(|l| Slot { name: l.name.as_str().to_owned(), ty: type_name(l.ty) })
        .collect();
    let mut locals = Vec::new();
    g.flatten(0, &mut locals);
    FunctionIr { params, locals, code: g.code }
}

struct FnGen<'c, 'a> {
    cx: &'c mut ScriptCx<'a>,
    body: &'c Body,
    types: &'c TypeckResult,
    ret: Option<TypeRef>,
    code: Vec<(Inst, u32)>,
    line: u32,
    labels: u32,
    /// Released temporaries by lowercase type name, oldest first.
    free: FxHashMap<String, VecDeque<String>>,
    scopes: Vec<Scope>,
    current: usize,
    none_var: bool,
    /// Lowercase names of the declared locals.
    declared: FxHashSet<String>,
}

impl<'a> FnGen<'_, 'a> {
    fn sema(&self) -> Sema<'a> {
        self.cx.sema
    }

    // === Emission ===

    fn op(&mut self, opcode: OpCode, args: Vec<Operand>) {
        self.code.push((Inst::Op { opcode, args }, self.line));
    }

    fn label(&mut self) -> Label {
        self.labels += 1;
        Label(self.labels - 1)
    }

    fn jump(&mut self, opcode: OpCode, cond: Option<Operand>, target: Label) {
        self.code.push((Inst::Jump { opcode, cond, target }, self.line));
    }

    fn place(&mut self, label: Label) {
        self.code.push((Inst::Label(label), self.line));
    }

    fn set_line(&mut self, range: TextRange) {
        self.line = self.cx.line(range);
    }

    // === Slots ===

    fn new_temp(&mut self, ty: &str) -> String {
        let name = format!("::temp{}", self.cx.temps);
        self.cx.temps += 1;
        self.scopes[self.current].temps.push(Slot { name: name.clone(), ty: ty.to_owned() });
        name
    }

    /// A temporary for the result of an operation on `operands`: a released
    /// one, else an operand temporary of the same type, else a new one.
    fn dest(&mut self, ty: VTy, operands: &[&Val]) -> String {
        let name = ty.name();
        if let Some(t) = self.free.get_mut(&name.to_ascii_lowercase()).and_then(VecDeque::pop_front)
        {
            return t;
        }
        for v in operands.iter().rev() {
            if v.temp
                && v.ty.is(ty)
                && let Operand::Var(n) = &v.op
            {
                return n.clone();
            }
        }
        self.new_temp(&name)
    }

    fn release(&mut self, v: &Val) {
        if v.temp
            && let Operand::Var(n) = &v.op
        {
            self.free.entry(v.ty.name().to_ascii_lowercase()).or_default().push_back(n.clone());
        }
    }

    /// Releases `v` unless it is `dest`, which lives on.
    fn release_except(&mut self, v: &Val, dest: &str) {
        if v.op.as_var() != Some(dest) {
            self.release(v);
        }
    }

    fn none_var(&mut self) -> Operand {
        if !self.none_var {
            self.none_var = true;
            self.scopes[self.current]
                .temps
                .push(Slot { name: "::NoneVar".to_owned(), ty: "None".to_owned() });
        }
        Operand::var("::NoneVar")
    }

    fn local_name(&self, local: papyrus_ide::hir::LocalId) -> String {
        self.body.local(local).name.as_str().to_owned()
    }

    fn declare(&mut self, local: papyrus_ide::hir::LocalId) {
        let data = self.body.local(local);
        let name = data.name.as_str().to_owned();
        if self.declared.insert(name.to_ascii_lowercase()) {
            self.scopes[self.current].locals.push(Slot { name, ty: type_name(data.ty) });
        }
    }

    fn flatten(&mut self, scope: usize, out: &mut Vec<Slot>) {
        let s = std::mem::take(&mut self.scopes[scope]);
        out.extend(s.temps);
        out.extend(s.locals);
        for child in s.children {
            self.flatten(child, out);
        }
    }

    fn block(&mut self, stmts: &[StmtId]) {
        let parent = self.current;
        self.scopes.push(Scope::default());
        let id = self.scopes.len() - 1;
        self.scopes[parent].children.push(id);
        self.current = id;
        for &s in stmts {
            self.stmt(s);
        }
        self.current = parent;
    }

    // === Statements ===

    fn stmt(&mut self, id: StmtId) {
        self.set_line(self.body.stmt_range(id));
        match self.body.stmt(id).clone() {
            Stmt::Local { local, init } => {
                self.declare(local);
                if let Some(init) = init {
                    let v = self.expr(init);
                    let v = self.coerce_opt(v, self.body.local(local).ty);
                    self.op(
                        OpCode::Assign,
                        vec![Operand::Var(self.local_name(local)), v.op.clone()],
                    );
                    self.release(&v);
                }
            }
            Stmt::Assign { op, target, value } => self.assign(op, target, value),
            Stmt::Expr(e) => {
                let v = self.expr(e);
                self.release(&v);
            }
            Stmt::Return(value) => match value {
                Some(e) => {
                    let v = self.expr(e);
                    let v = self.coerce_opt(v, self.ret);
                    self.op(OpCode::Ret, vec![v.op.clone()]);
                    self.release(&v);
                }
                None => self.op(OpCode::Ret, vec![Operand::None]),
            },
            Stmt::If { branches, else_branch } => {
                let end = self.label();
                let count = branches.len();
                let mut conds = Vec::new();
                for (i, (cond, block)) in branches.iter().enumerate() {
                    self.set_line(self.body.expr_range(*cond));
                    let c = self.expr(*cond);
                    let next =
                        if i + 1 < count || else_branch.is_some() { self.label() } else { end };
                    self.jump(OpCode::JmpF, Some(c.op.clone()), next);
                    // Like the official compiler, condition temporaries are
                    // only recycled after the whole statement.
                    conds.push(c);
                    self.block(block);
                    self.jump(OpCode::Jmp, None, end);
                    if next != end {
                        self.place(next);
                    }
                }
                if let Some(block) = &else_branch {
                    self.block(block);
                }
                self.place(end);
                for c in &conds {
                    self.release(c);
                }
            }
            Stmt::While { condition, body } => {
                let top = self.label();
                let end = self.label();
                self.place(top);
                self.set_line(self.body.expr_range(condition));
                let c = self.expr(condition);
                self.jump(OpCode::JmpF, Some(c.op.clone()), end);
                self.block(&body);
                self.jump(OpCode::Jmp, None, top);
                self.place(end);
                self.release(&c);
            }
        }
    }

    fn assign(&mut self, op: Option<BinaryOp>, target: ExprId, value: ExprId) {
        let ty = self.ty(target);
        match self.body.expr(target).clone() {
            Expr::Local(local) => self.assign_var(self.local_name(local), ty, op, value),
            Expr::Def(Def::Member { file, index }) => {
                let m = self.sema().member(file, index);
                match self.member_place(file, index) {
                    Some(var) => self.assign_var(var, ty, op, value),
                    None => self.assign_prop(None, m.name.as_str(), ty, op, value),
                }
            }
            Expr::Field { receiver, def: Some(Def::Member { file, index }), .. } => {
                let m = self.sema().member(file, index);
                if m.kind == MemberKind::Variable {
                    self.assign_var(m.name.as_str().to_owned(), ty, op, value);
                } else {
                    self.assign_prop(Some(receiver), m.name.as_str(), ty, op, value);
                }
            }
            Expr::Index { base, index } => {
                let b = self.expr(base);
                let i = self.expr(index);
                let i = self.coerce(i, TypeRef::INT);
                let t = self.dest(ty, &[]);
                let v = self.expr(value);
                let v = match op {
                    None => v,
                    Some(op) => {
                        let cur = self.dest(ty, &[]);
                        self.op(
                            OpCode::ArrayGetElement,
                            vec![Operand::var(&cur), b.op.clone(), i.op.clone()],
                        );
                        self.binary_vals(op, Val::temp(cur, ty), v)
                    }
                };
                let v = self.coerce_vty(v, ty);
                self.op(OpCode::Assign, vec![Operand::var(&t), v.op.clone()]);
                self.release(&v);
                self.op(
                    OpCode::ArraySetElement,
                    vec![b.op.clone(), i.op.clone(), Operand::var(&t)],
                );
                self.release(&Val::temp(t, ty));
                self.release(&b);
                self.release(&i);
            }
            _ => {
                let v = self.expr(value);
                self.release(&v);
            }
        }
    }

    /// The variable holding a script member, for variables and auto
    /// properties of the compiled script; `None` for other properties.
    fn member_place(&self, file: papyrus_ide::FileId, index: u32) -> Option<String> {
        let m = self.sema().member(file, index);
        match m.kind {
            MemberKind::Variable => Some(m.name.as_str().to_owned()),
            MemberKind::Property
                if m.flags.contains(Flags::AUTO) && !m.flags.contains(Flags::AUTO_READONLY) =>
            {
                Some(papyrus_pex::pex::property_auto_var_name(m.name.as_str()))
            }
            _ => None,
        }
    }

    fn assign_var(&mut self, var: String, ty: VTy, op: Option<BinaryOp>, value: ExprId) {
        let v = self.expr(value);
        let v = match op {
            None => v,
            Some(op) => self.binary_vals(op, Val::var(&var, ty), v),
        };
        let v = self.coerce_vty(v, ty);
        self.op(OpCode::Assign, vec![Operand::Var(var), v.op.clone()]);
        self.release(&v);
    }

    /// Assigns property `name` of `recv` (`None`: `Self`). The value is
    /// computed before the receiver, unless the operator reads the property.
    fn assign_prop(
        &mut self,
        recv: Option<ExprId>,
        name: &str,
        ty: VTy,
        op: Option<BinaryOp>,
        value: ExprId,
    ) {
        let receiver = |g: &mut Self| match recv {
            Some(r) => g.expr(r),
            None => Val::var("self", VTy::Unknown),
        };
        let t = self.dest(ty, &[]);
        let (recv, v) = match op {
            None => {
                let v = self.expr(value);
                (None, v)
            }
            Some(op) => {
                let recv = receiver(self);
                let v = self.expr(value);
                let cur = self.dest(ty, &[]);
                self.op(
                    OpCode::PropGet,
                    vec![Operand::var(name), recv.op.clone(), Operand::var(&cur)],
                );
                let v = self.binary_vals(op, Val::temp(cur, ty), v);
                (Some(recv), v)
            }
        };
        let v = self.coerce_vty(v, ty);
        self.op(OpCode::Assign, vec![Operand::var(&t), v.op.clone()]);
        self.release(&v);
        let recv = match recv {
            Some(r) => r,
            None => receiver(self),
        };
        self.op(OpCode::PropSet, vec![Operand::var(name), recv.op.clone(), Operand::var(&t)]);
        self.release(&Val::temp(t, ty));
        self.release(&recv);
    }

    // === Conversions ===

    fn coerce_opt(&mut self, v: Val, to: Option<TypeRef>) -> Val {
        match to {
            Some(t) => self.coerce(v, t),
            None => v,
        }
    }

    fn coerce_vty(&mut self, v: Val, to: VTy) -> Val {
        match to {
            VTy::Value(t) => self.coerce(v, t),
            _ => v,
        }
    }

    /// Applies the implicit conversion of `v` to `to`.
    fn coerce(&mut self, v: Val, to: TypeRef) -> Val {
        let target = VTy::Value(to);
        match v.ty {
            VTy::Unknown => v,
            _ if v.ty.is(target) => v,
            // Arrays take `None` and arrays as they are; everything else,
            // child objects and `None` included, is cast.
            _ if to.array => v,
            _ => self.cast(v, target),
        }
    }

    /// Converts `v` to `to`, nothing when it already has that type.
    fn cast(&mut self, v: Val, to: VTy) -> Val {
        if v.ty.is(to) {
            return Val { ty: to, ..v };
        }
        self.cast_always(v, to)
    }

    /// `CAST`, even to the same type: what `x as T` compiles to.
    fn cast_always(&mut self, v: Val, to: VTy) -> Val {
        let d = self.dest(to, &[&v]);
        self.op(OpCode::Cast, vec![Operand::var(&d), v.op.clone()]);
        self.release_except(&v, &d);
        Val::temp(d, to)
    }

    // === Expressions ===

    fn ty(&self, id: ExprId) -> VTy {
        self.types.types.get(id.0 as usize).map_or(VTy::Unknown, |&t| VTy::of(t))
    }

    fn expr(&mut self, id: ExprId) -> Val {
        let ty = self.ty(id);
        match self.body.expr(id).clone() {
            Expr::Missing | Expr::Unresolved(_) => Val::constant(Operand::None, VTy::None),
            Expr::Literal(lit) => {
                let value = self.literal(id, lit);
                Val::constant(value, ty)
            }
            Expr::Local(local) => Val::var(self.local_name(local), ty),
            Expr::Def(def) => self.def_value(def, ty),
            Expr::SelfRef | Expr::ParentRef => Val::var("self", ty),
            Expr::Field { receiver, def, .. } => self.field(receiver, def, ty),
            Expr::Call { receiver, def, args, .. } => self.call(receiver, def, &args),
            Expr::Index { base, index } => {
                let b = self.expr(base);
                let i = self.expr(index);
                let i = self.coerce(i, TypeRef::INT);
                let d = self.dest(ty, &[&i]);
                self.op(
                    OpCode::ArrayGetElement,
                    vec![Operand::var(&d), b.op.clone(), i.op.clone()],
                );
                self.release(&b);
                self.release_except(&i, &d);
                Val::temp(d, ty)
            }
            Expr::Cast { expr, ty: target } => {
                let v = self.expr(expr);
                match target {
                    Some(t) => self.cast_always(v, VTy::Value(t)),
                    None => v,
                }
            }
            Expr::New { size, .. } => {
                let s = self.expr(size);
                let s = self.coerce(s, TypeRef::INT);
                let d = self.dest(ty, &[]);
                self.op(OpCode::ArrayCreate, vec![Operand::var(&d), s.op.clone()]);
                self.release(&s);
                Val::temp(d, ty)
            }
            Expr::Unary { op, expr } => {
                // Negative literals are constants.
                if op == UnaryOp::Neg
                    && let Expr::Literal(lit) = self.body.expr(expr)
                {
                    match self.literal(expr, *lit) {
                        Operand::Int(i) => {
                            return Val::constant(Operand::Int(i.wrapping_neg()), ty);
                        }
                        Operand::Float(f) => return Val::constant(Operand::Float(-f), ty),
                        _ => {}
                    }
                }
                let v = self.expr(expr);
                let (opcode, ty) = match op {
                    UnaryOp::Not => (OpCode::Not, VTy::BOOL),
                    UnaryOp::Neg if v.ty.is(VTy::FLOAT) => (OpCode::FNeg, VTy::FLOAT),
                    UnaryOp::Neg => (OpCode::INeg, VTy::INT),
                };
                let d = self.dest(ty, &[&v]);
                self.op(opcode, vec![Operand::var(&d), v.op.clone()]);
                self.release_except(&v, &d);
                Val::temp(d, ty)
            }
            Expr::Binary { op: op @ (BinaryOp::And | BinaryOp::Or), lhs, rhs } => {
                self.logical(op, lhs, rhs)
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.expr(lhs);
                let r = self.expr(rhs);
                self.binary_vals(op, l, r)
            }
        }
    }

    fn literal(&self, id: ExprId, lit: Literal) -> Operand {
        let range = self.body.expr_range(id);
        let text = self.sema().text.get(usize::from(range.start())..usize::from(range.end()));
        match lit {
            Literal::None => Operand::None,
            _ => text.and_then(literal::parse).unwrap_or(Operand::None),
        }
    }

    fn def_value(&mut self, def: Def, ty: VTy) -> Val {
        match def {
            Def::Member { file, index } => {
                if let Some(var) = self.member_place(file, index) {
                    return Val::var(var, ty);
                }
                let m = self.sema().member(file, index);
                if m.kind != MemberKind::Property {
                    return Val::constant(Operand::None, VTy::None);
                }
                let d = self.dest(ty, &[]);
                self.op(
                    OpCode::PropGet,
                    vec![Operand::var(m.name.as_str()), Operand::var("self"), Operand::var(&d)],
                );
                Val::temp(d, ty)
            }
            Def::Script { file } => Val::var(self.sema().ws.file(file).index.name_str(), ty),
            _ => Val::constant(Operand::None, VTy::None),
        }
    }

    fn field(&mut self, receiver: ExprId, def: Option<Def>, ty: VTy) -> Val {
        match def {
            Some(Def::Builtin(Builtin::Length, _)) => {
                let b = self.expr(receiver);
                let d = self.dest(VTy::INT, &[]);
                self.op(OpCode::ArrayLength, vec![Operand::var(&d), b.op.clone()]);
                self.release(&b);
                Val::temp(d, VTy::INT)
            }
            Some(Def::Member { file, index }) => {
                let m = self.sema().member(file, index);
                match m.kind {
                    MemberKind::Variable => Val::var(m.name.as_str(), ty),
                    MemberKind::Property => {
                        let r = self.expr(receiver);
                        let d = self.dest(ty, &[]);
                        self.op(
                            OpCode::PropGet,
                            vec![Operand::var(m.name.as_str()), r.op.clone(), Operand::var(&d)],
                        );
                        self.release(&r);
                        Val::temp(d, ty)
                    }
                    _ => Val::constant(Operand::None, VTy::None),
                }
            }
            _ => Val::constant(Operand::None, VTy::None),
        }
    }

    fn binary_vals(&mut self, op: BinaryOp, l: Val, r: Val) -> Val {
        use BinaryOp::*;
        match op {
            Eq | Ne | Lt | Le | Gt | Ge => {
                let (l, r) = self.harmonize(l, r);
                let d = self.dest(VTy::BOOL, &[&l, &r]);
                let opcode = match op {
                    Eq | Ne => OpCode::CmpEq,
                    Lt => OpCode::CmpLt,
                    Le => OpCode::CmpLe,
                    Gt => OpCode::CmpGt,
                    _ => OpCode::CmpGe,
                };
                self.op(opcode, vec![Operand::var(&d), l.op.clone(), r.op.clone()]);
                self.release_except(&l, &d);
                self.release_except(&r, &d);
                if op == Ne {
                    self.op(OpCode::Not, vec![Operand::var(&d), Operand::var(&d)]);
                }
                Val::temp(d, VTy::BOOL)
            }
            Add if l.ty.is(VTy::STRING) || r.ty.is(VTy::STRING) => {
                let l = self.coerce(l, TypeRef::STRING);
                let r = self.coerce(r, TypeRef::STRING);
                self.arith(OpCode::StrCat, VTy::STRING, l, r)
            }
            _ => {
                let float = l.ty.is(VTy::FLOAT) || r.ty.is(VTy::FLOAT);
                let (ty, target) =
                    if float { (VTy::FLOAT, TypeRef::FLOAT) } else { (VTy::INT, TypeRef::INT) };
                let l = self.coerce(l, target);
                let r = self.coerce(r, target);
                let opcode = match (op, float) {
                    (Add, false) => OpCode::IAdd,
                    (Add, true) => OpCode::FAdd,
                    (Sub, false) => OpCode::ISub,
                    (Sub, true) => OpCode::FSub,
                    (Mul, false) => OpCode::IMul,
                    (Mul, true) => OpCode::FMul,
                    (Div, false) => OpCode::IDiv,
                    (Div, true) => OpCode::FDiv,
                    _ => OpCode::IMod,
                };
                self.arith(opcode, ty, l, r)
            }
        }
    }

    fn arith(&mut self, opcode: OpCode, ty: VTy, l: Val, r: Val) -> Val {
        let d = self.dest(ty, &[&l, &r]);
        self.op(opcode, vec![Operand::var(&d), l.op.clone(), r.op.clone()]);
        self.release_except(&l, &d);
        self.release_except(&r, &d);
        Val::temp(d, ty)
    }

    /// Brings the operands of a comparison to the same type.
    fn harmonize(&mut self, l: Val, r: Val) -> (Val, Val) {
        if l.ty == VTy::Unknown || r.ty == VTy::Unknown || l.ty.is(r.ty) {
            return (l, r);
        }
        let rank = |t: VTy| match t {
            VTy::None => 0,
            t if t.is(VTy::STRING) => 4,
            t if t.is(VTy::FLOAT) => 3,
            t if t.is(VTy::BOOL) => 5,
            t if t.is(VTy::INT) => 2,
            _ => 1,
        };
        let (rl, rr) = (rank(l.ty), rank(r.ty));
        if rl == 1 && rr == 1 {
            // Two object types: the child is cast to its parent.
            let (VTy::Value(lt), VTy::Value(rt)) = (l.ty, r.ty) else { return (l, r) };
            return match (lt.object(), rt.object()) {
                (Some(a), Some(b)) if self.extends(b, a) => {
                    let r = self.cast(r, l.ty);
                    (l, r)
                }
                (Some(a), Some(b)) if self.extends(a, b) => {
                    let ty = r.ty;
                    (self.cast(l, ty), r)
                }
                _ => (l, r),
            };
        }
        if rl > rr {
            let ty = l.ty;
            let r = self.cast(r, ty);
            (l, r)
        } else {
            let ty = r.ty;
            let l = self.cast(l, ty);
            (l, r)
        }
    }

    /// Whether script `child` is `ancestor` or extends it.
    fn extends(&self, child: Name, ancestor: Name) -> bool {
        self.sema().chain(child).any(|(_, index)| index.name == Some(ancestor))
    }

    fn logical(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Val {
        let l = self.expr(lhs);
        let r = self.dest(VTy::BOOL, &[&l]);
        self.op(OpCode::Cast, vec![Operand::var(&r), l.op.clone()]);
        self.release_except(&l, &r);
        let end = self.label();
        let jump = if op == BinaryOp::And { OpCode::JmpF } else { OpCode::JmpT };
        self.jump(jump, Some(Operand::var(&r)), end);
        let rv = self.expr(rhs);
        self.op(OpCode::Cast, vec![Operand::var(&r), rv.op.clone()]);
        self.release_except(&rv, &r);
        self.place(end);
        Val::temp(r, VTy::BOOL)
    }

    // === Calls ===

    fn call(&mut self, receiver: Option<ExprId>, def: Option<Def>, args: &[Arg]) -> Val {
        match def {
            Some(Def::Builtin(b @ (Builtin::GoToState | Builtin::GetState), _)) => {
                let recv = match receiver {
                    Some(r) => self.expr(r),
                    None => Val::var("self", VTy::Unknown),
                };
                let (name, params, ret) = if b == Builtin::GoToState {
                    let p = ParamSig {
                        name: Name::new("asNewState"),
                        ty: Some(TypeRef::STRING),
                        default: None,
                    };
                    ("GotoState", vec![p], VTy::None)
                } else {
                    ("GetState", Vec::new(), VTy::STRING)
                };
                let args = self.args(&params, args);
                let dest = self.call_dest(ret, &args);
                let mut ops = vec![Operand::var(name), recv.op.clone(), dest.op.clone()];
                ops.extend(args.iter().map(|a| a.op.clone()));
                self.op(OpCode::CallMethod, ops);
                self.release_all(&args, &dest);
                self.release(&recv);
                dest
            }
            Some(Def::Builtin(b @ (Builtin::Find | Builtin::RFind), elem)) => {
                let Some(receiver) = receiver else {
                    return Val::constant(Operand::None, VTy::None);
                };
                let arr = self.expr(receiver);
                let params = [
                    ParamSig { name: Name::new("akElement"), ty: elem, default: None },
                    ParamSig {
                        name: Name::new("aiStartIndex"),
                        ty: Some(TypeRef::INT),
                        default: Some(Operand::Int(if b == Builtin::Find { 0 } else { -1 })),
                    },
                ];
                let args = self.args(&params, args);
                let d = self.dest(VTy::INT, &[]);
                let opcode = if b == Builtin::Find {
                    OpCode::ArrayFindElement
                } else {
                    OpCode::ArrayRFindElement
                };
                let mut ops = vec![arr.op.clone(), Operand::var(&d)];
                ops.extend(args.iter().map(|a| a.op.clone()));
                self.op(opcode, ops);
                let dest = Val::temp(d, VTy::INT);
                self.release_all(&args, &dest);
                self.release(&arr);
                dest
            }
            Some(Def::Member { file, index }) => {
                let m = self.sema().member(file, index);
                if !m.is_callable() {
                    return Val::constant(Operand::None, VTy::None);
                }
                let params: Vec<ParamSig> = m
                    .params
                    .iter()
                    .map(|p| ParamSig {
                        name: p.name,
                        ty: p.ty,
                        default: p.default.as_deref().and_then(literal::parse).map(|d| {
                            p.ty.and_then(|t| convert_const(&d, &t.to_string())).unwrap_or(d)
                        }),
                    })
                    .collect();
                let ret = match (m.kind, m.ty) {
                    (MemberKind::Function, Some(t)) => VTy::Value(t),
                    _ => VTy::None,
                };
                let name = Operand::var(m.name.as_str());
                let parent_call =
                    receiver.is_some_and(|r| matches!(self.body.expr(r), Expr::ParentRef));
                if m.is_global() {
                    let script = self.sema().ws.file(file).index.name_str();
                    let args = self.args(&params, args);
                    let dest = self.call_dest(ret, &args);
                    let mut ops = vec![Operand::var(script), name, dest.op.clone()];
                    ops.extend(args.iter().map(|a| a.op.clone()));
                    self.op(OpCode::CallStatic, ops);
                    self.release_all(&args, &dest);
                    dest
                } else if parent_call {
                    let args = self.args(&params, args);
                    let dest = self.call_dest(ret, &args);
                    let mut ops = vec![name, dest.op.clone()];
                    ops.extend(args.iter().map(|a| a.op.clone()));
                    self.op(OpCode::CallParent, ops);
                    self.release_all(&args, &dest);
                    dest
                } else {
                    let recv = match receiver {
                        Some(r) => self.expr(r),
                        None => Val::var("self", VTy::Unknown),
                    };
                    let args = self.args(&params, args);
                    let dest = self.call_dest(ret, &args);
                    let mut ops = vec![name, recv.op.clone(), dest.op.clone()];
                    ops.extend(args.iter().map(|a| a.op.clone()));
                    self.op(OpCode::CallMethod, ops);
                    self.release_all(&args, &dest);
                    self.release(&recv);
                    dest
                }
            }
            _ => {
                for a in args {
                    let v = self.expr(a.value);
                    self.release(&v);
                }
                Val::constant(Operand::None, VTy::None)
            }
        }
    }

    fn call_dest(&mut self, ret: VTy, args: &[Val]) -> Val {
        match ret {
            VTy::Value(_) => {
                let refs: Vec<&Val> = args.iter().collect();
                let d = self.dest(ret, &refs);
                Val::temp(d, ret)
            }
            _ => Val::constant(self.none_var(), VTy::None),
        }
    }

    fn release_all(&mut self, args: &[Val], dest: &Val) {
        let keep = dest.op.as_var().map(str::to_owned);
        for a in args {
            match &keep {
                Some(k) if a.temp => self.release_except(a, k),
                _ => self.release(a),
            }
        }
    }

    /// Evaluates the arguments in source order and returns them in parameter
    /// order, defaults filled in and converted to the parameter types.
    fn args(&mut self, params: &[ParamSig], args: &[Arg]) -> Vec<Val> {
        let mut slots: Vec<Option<Val>> = vec![None; params.len()];
        let mut positional = 0;
        for a in args {
            let v = self.expr(a.value);
            let slot = match a.name {
                Some(n) => params.iter().position(|p| p.name == n),
                None => {
                    positional += 1;
                    Some(positional - 1)
                }
            };
            match slot.filter(|&i| i < params.len()) {
                Some(i) => slots[i] = Some(v),
                None => self.release(&v),
            }
        }
        let mut out = Vec::with_capacity(params.len());
        for (p, v) in params.iter().zip(slots) {
            let v = v.unwrap_or_else(|| {
                // Defaults are passed as they are, already of the parameter
                // type.
                let d = p.default.clone().unwrap_or(Operand::None);
                let ty = p.ty.map_or_else(|| VTy::of_const(&d), VTy::Value);
                Val::constant(d, ty)
            });
            out.push(self.coerce_opt(v, p.ty));
        }
        out
    }
}
