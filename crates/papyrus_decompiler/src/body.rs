//! Decompilation of function bodies.
//!
//! Compilers keep every intermediate value in a temporary (`::tempN`) that is
//! written once and read once, right after. Expressions are rebuilt by
//! keeping the value of each temporary pending until an instruction reads it;
//! instructions writing real variables, and calls whose result is not used,
//! become statements.
//!
//! Control flow follows the shapes compilers generate:
//!
//! - `If`: `JMPF c, next; then; JMP end; next: else...`, `ElseIf` being an
//!   `If` alone in the `Else` part;
//! - `While`: `top: cond; JMPF c, end; body; JMP top; end:`;
//! - `&&` / `||`: `CAST r, a; JMPF/JMPT r, end; b...; CAST r, b; end:`.

use papyrus_pex::pex::{FunctionInfo, Instruction, OpCode, VariableValue};
use rustc_hash::FxHashMap;

use crate::{
    ScriptCx,
    ast::{BinOp, Expr, Stmt},
};

/// Decompiles the code of a function.
pub(crate) fn decompile(cx: &ScriptCx<'_>, info: &FunctionInfo) -> Vec<Stmt> {
    let mut types = cx.member_types.clone();
    for v in info.params.iter().chain(&info.locals) {
        types.insert(cx.str(v.name).to_ascii_lowercase(), cx.str(v.ty));
    }
    let mut b = Body { cx, insts: &info.instructions, types, pending: Vec::new() };
    let mut out = Vec::new();
    b.block(0, info.instructions.len(), &mut out, true);
    // A void `Return` at the very end is implied.
    if matches!(out.last(), Some(Stmt::Return(None))) {
        out.pop();
    }
    declare_locals(cx, info, &mut out);
    out
}

struct Body<'c, 'p> {
    cx: &'c ScriptCx<'p>,
    insts: &'p [Instruction],
    /// Lowercase names of locals, parameters and members to their type.
    types: FxHashMap<String, String>,
    /// Values of temporaries not read yet, in definition order.
    pending: Vec<(String, Expr)>,
}

/// `::tempN`; `::temporarilyCleared_var` is a property variable.
fn is_temp(name: &str) -> bool {
    name.len() > 6
        && name[..6].eq_ignore_ascii_case("::temp")
        && name[6..].bytes().all(|b| b.is_ascii_digit())
}

impl Body<'_, '_> {
    fn ident(&self, v: &VariableValue) -> Option<String> {
        match v {
            VariableValue::Identifier(id) => Some(self.cx.str(*id)),
            _ => None,
        }
    }

    /// The type of a value, lowercase.
    fn type_of(&self, v: &VariableValue) -> Option<String> {
        Some(match v {
            VariableValue::Null => "none".to_owned(),
            VariableValue::Identifier(_) => {
                let name = self.ident(v)?.to_ascii_lowercase();
                if name == "self" {
                    return Some(self.cx.script.to_ascii_lowercase());
                }
                self.types.get(&name)?.to_ascii_lowercase()
            }
            VariableValue::String(_) => "string".to_owned(),
            VariableValue::Integer(_) => "int".to_owned(),
            VariableValue::Float(_) => "float".to_owned(),
            VariableValue::Boolean(_) => "bool".to_owned(),
        })
    }

    /// The expression of a value; reading a temporary consumes it.
    fn take(&mut self, v: &VariableValue) -> Expr {
        match v {
            VariableValue::Null => Expr::None,
            VariableValue::Integer(i) => Expr::Int(*i),
            VariableValue::Float(f) => Expr::Float(*f),
            VariableValue::Boolean(b) => Expr::Bool(*b),
            VariableValue::String(id) => Expr::Str(self.cx.str(*id)),
            VariableValue::Identifier(_) => {
                let name = self.ident(v).unwrap_or_default();
                if is_temp(&name)
                    && let Some(i) =
                        self.pending.iter().position(|(n, _)| n.eq_ignore_ascii_case(&name))
                {
                    return self.pending.remove(i).1;
                }
                self.name(&name)
            }
        }
    }

    /// A name as written in the source.
    fn name(&self, name: &str) -> Expr {
        let lower = name.to_ascii_lowercase();
        if lower == "self" {
            return Expr::SelfRef;
        }
        if lower == "::nonevar" {
            return Expr::None;
        }
        if let Some(prop) = self.cx.auto_vars.get(&lower) {
            return Expr::Name(prop.clone());
        }
        // The hidden variable of an auto property of a parent script.
        if let Some(prop) = name.strip_prefix("::").and_then(|n| n.strip_suffix("_var")) {
            return Expr::Name(prop.to_owned());
        }
        // A temporary read without definition (code this decompiler does not
        // understand): keep a valid name.
        Expr::Name(name.trim_start_matches(':').to_owned())
    }

    /// Calls whose result is pending become statements, in order, so that
    /// they run before what follows.
    fn flush_calls(&mut self, out: &mut Vec<Stmt>) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].1.has_call() {
                out.push(Stmt::Expr(self.pending.remove(i).1));
            } else {
                i += 1;
            }
        }
    }

    fn emit(&mut self, stmt: Stmt, out: &mut Vec<Stmt>) {
        self.flush_calls(out);
        out.push(stmt);
    }

    /// Writes `value` to `dest`.
    fn define(&mut self, dest: &VariableValue, value: Expr, out: &mut Vec<Stmt>) {
        let Some(name) = self.ident(dest) else { return };
        if is_temp(&name) {
            if let Some(i) = self.pending.iter().position(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                // Overwritten before being read: only calls matter.
                let (_, old) = self.pending.remove(i);
                if old.has_call() {
                    self.emit(Stmt::Expr(old), out);
                }
            }
            self.pending.push((name, value));
        } else if name.eq_ignore_ascii_case("::NoneVar") {
            self.emit(Stmt::Expr(value), out);
        } else {
            let target = self.name(&name);
            self.emit(Stmt::Assign(target, value), out);
        }
    }

    fn target(&self, index: usize, inst: &Instruction) -> usize {
        let offset = match inst.args.last() {
            Some(VariableValue::Integer(o)) => *o as i64,
            _ => 1,
        };
        (index as i64 + offset).max(0) as usize
    }

    fn block(&mut self, start: usize, end: usize, out: &mut Vec<Stmt>, flush: bool) {
        let mut i = start;
        while i < end {
            i = match self.loop_end(i, end) {
                Some(j) => self.while_loop(i, j, out),
                None => self.step(i, end, out),
            };
        }
        if flush {
            // Values nobody reads come from expression statements, calls or
            // not (`x == 1` alone compiles).
            for (_, e) in std::mem::take(&mut self.pending) {
                out.push(Stmt::Expr(e));
            }
        }
    }

    /// The jump closing a loop starting at `i`: the last `JMP` back to it.
    fn loop_end(&self, i: usize, end: usize) -> Option<usize> {
        (i + 1..end).rev().find(|&j| {
            let inst = &self.insts[j];
            inst.opcode == OpCode::Jmp && self.target(j, inst) == i
        })
    }

    fn while_loop(&mut self, start: usize, back: usize, out: &mut Vec<Stmt>) -> usize {
        self.flush_calls(out);
        let mut k = start;
        let mut pre = Vec::new();
        while k < back {
            let inst = &self.insts[k];
            if matches!(inst.opcode, OpCode::JmpF | OpCode::JmpT) && self.target(k, inst) > back {
                break;
            }
            k = self.step(k, back, &mut pre);
        }
        out.extend(pre);
        if k >= back {
            // An endless loop: `While True`.
            let mut body = Vec::new();
            self.block(start, back, &mut body, true);
            out.push(Stmt::While(Expr::Bool(true), body));
            return back + 1;
        }
        let inst = &self.insts[k];
        let cond = self.take(&inst.args[0]);
        let cond = if inst.opcode == OpCode::JmpT { not(cond) } else { cond };
        let mut body = Vec::new();
        self.block(k + 1, back, &mut body, true);
        out.push(Stmt::While(cond, body));
        back + 1
    }

    /// Decompiles the instruction at `i`, or the construct starting there,
    /// and returns the index of the next one.
    fn step(&mut self, i: usize, end: usize, out: &mut Vec<Stmt>) -> usize {
        let inst = &self.insts[i];
        match inst.opcode {
            OpCode::Cast => match self.logical(i, end, out) {
                Some(next) => next,
                None => {
                    self.simple(inst, out);
                    i + 1
                }
            },
            OpCode::JmpF | OpCode::JmpT => self.if_stmt(i, end, out),
            // Jumps to the next instruction, or that no construct explains.
            OpCode::Jmp => i + 1,
            _ => {
                self.simple(inst, out);
                i + 1
            }
        }
    }

    /// `CAST r, a; JMPF r, end; ...; CAST r, b; end:` is `a && b` (`||` with
    /// `JMPT`).
    fn logical(&mut self, i: usize, end: usize, out: &mut Vec<Stmt>) -> Option<usize> {
        let cast = &self.insts[i];
        let jump = self.insts.get(i + 1)?;
        let r = self.ident(&cast.args[0])?;
        if !matches!(jump.opcode, OpCode::JmpF | OpCode::JmpT)
            || self.ident(&jump.args[0]).is_none_or(|c| !c.eq_ignore_ascii_case(&r))
        {
            return None;
        }
        let after = self.target(i + 1, jump);
        if after <= i + 2 || after > end {
            return None;
        }
        let last = &self.insts[after - 1];
        if last.opcode != OpCode::Cast
            || self.ident(&last.args[0]).is_none_or(|d| !d.eq_ignore_ascii_case(&r))
        {
            return None;
        }
        let a = self.take(&cast.args[1]);
        let mut inner = Vec::new();
        self.block(i + 2, after - 1, &mut inner, false);
        out.extend(inner);
        let b = self.take(&last.args[1]);
        let op = if jump.opcode == OpCode::JmpF { BinOp::And } else { BinOp::Or };
        self.define(&cast.args[0], Expr::Binary(op, Box::new(a), Box::new(b)), out);
        Some(after)
    }

    fn if_stmt(&mut self, i: usize, end: usize, out: &mut Vec<Stmt>) -> usize {
        let inst = &self.insts[i];
        let cond = self.take(&inst.args[0]);
        let cond = if inst.opcode == OpCode::JmpT { not(cond) } else { cond };
        self.flush_calls(out);
        let next = self.target(i, inst).clamp(i + 1, end);
        // `JMP end` closing the `then` part when there is an `Else`.
        let else_end = (next >= i + 2)
            .then(|| (next - 1, &self.insts[next - 1]))
            .filter(|(j, last)| last.opcode == OpCode::Jmp && self.target(*j, last) > next)
            .map(|(j, last)| self.target(j, last).min(end));
        let mut then = Vec::new();
        self.block(i + 1, if else_end.is_some() { next - 1 } else { next }, &mut then, true);
        let mut branches = vec![(cond, then)];
        let mut else_branch = None;
        if let Some(e) = else_end {
            let mut els = Vec::new();
            self.block(next, e, &mut els, true);
            match els.as_mut_slice() {
                [Stmt::If(inner, inner_else)] => {
                    branches.append(inner);
                    else_branch = inner_else.take();
                }
                [] => {}
                _ => else_branch = Some(els),
            }
        }
        out.push(Stmt::If(branches, else_branch));
        else_end.unwrap_or(next)
    }

    /// The instructions without control flow.
    fn simple(&mut self, inst: &Instruction, out: &mut Vec<Stmt>) {
        use OpCode::*;
        let a = &inst.args;
        match inst.opcode {
            Nop | Jmp | JmpT | JmpF => {}
            IAdd | FAdd | ISub | FSub | IMul | FMul | IDiv | FDiv | IMod | StrCat | CmpEq
            | CmpLt | CmpLe | CmpGt | CmpGe => {
                let op = match inst.opcode {
                    IAdd | FAdd | StrCat => BinOp::Add,
                    ISub | FSub => BinOp::Sub,
                    IMul | FMul => BinOp::Mul,
                    IDiv | FDiv => BinOp::Div,
                    IMod => BinOp::Rem,
                    CmpEq => BinOp::Eq,
                    CmpLt => BinOp::Lt,
                    CmpLe => BinOp::Le,
                    CmpGt => BinOp::Gt,
                    _ => BinOp::Ge,
                };
                let l = self.take(&a[1]);
                let r = self.take(&a[2]);
                // When both operands were converted, the conversion decides
                // the operation (`(i as Float) / j`): keep one explicit.
                let l = match (l, &r) {
                    (Expr::Implicit(e, ty), Expr::Implicit(..)) => Expr::Cast(e, ty),
                    (l, _) => l,
                };
                self.define(&a[0], Expr::Binary(op, Box::new(l), Box::new(r)), out);
            }
            Not => {
                let e = match self.take(&a[1]) {
                    Expr::Binary(BinOp::Eq, l, r) => Expr::Binary(BinOp::Ne, l, r),
                    e => not(e),
                };
                self.define(&a[0], e, out);
            }
            INeg | FNeg => {
                let e = match self.take(&a[1]) {
                    Expr::Int(i) => Expr::Int(i.wrapping_neg()),
                    Expr::Float(f) => Expr::Float(-f),
                    e => Expr::Neg(Box::new(e)),
                };
                self.define(&a[0], e, out);
            }
            Assign => {
                let v = self.take(&a[1]);
                self.define(&a[0], v, out);
            }
            Cast => {
                let to = self
                    .ident(&a[0])
                    .and_then(|d| self.types.get(&d.to_ascii_lowercase()).cloned());
                let from = self.type_of(&a[1]);
                let v = self.take(&a[1]);
                let e = match (to, from) {
                    (_, Some(f)) if f == "none" => v,
                    // `x as T` of an object already of type `T` is explicit.
                    (Some(t), Some(f)) if t.eq_ignore_ascii_case(&f) && is_primitive(&f) => v,
                    (Some(t), Some(f))
                        if t.eq_ignore_ascii_case("bool")
                            || t.eq_ignore_ascii_case("string")
                            || (t.eq_ignore_ascii_case("float") && f == "int") =>
                    {
                        Expr::Implicit(Box::new(v), t)
                    }
                    (Some(t), _) => Expr::Cast(Box::new(v), t),
                    (None, _) => v,
                };
                self.define(&a[0], e, out);
            }
            CallMethod => {
                let recv = match self.take(&a[1]) {
                    Expr::SelfRef => None,
                    r => Some(Box::new(r)),
                };
                let args = self.call_args(a, 4);
                let name = self.ident(&a[0]).unwrap_or_default();
                let name = if name.eq_ignore_ascii_case("GotoState") {
                    "GoToState".to_owned()
                } else {
                    name
                };
                self.define(&a[2], Expr::Call(recv, name, args), out);
            }
            CallParent => {
                let args = self.call_args(a, 3);
                let name = self.ident(&a[0]).unwrap_or_default();
                self.define(&a[1], Expr::Call(Some(Box::new(Expr::ParentRef)), name, args), out);
            }
            CallStatic => {
                let script = self.ident(&a[0]).unwrap_or_default();
                let args = self.call_args(a, 4);
                let name = self.ident(&a[1]).unwrap_or_default();
                let recv = (!script.eq_ignore_ascii_case(&self.cx.script))
                    .then(|| Box::new(Expr::Name(script)));
                self.define(&a[2], Expr::Call(recv, name, args), out);
            }
            Ret => {
                let v = match &a[0] {
                    VariableValue::Null => None,
                    v => Some(self.take(v)),
                };
                self.emit(Stmt::Return(v), out);
            }
            PropGet => {
                let recv = self.take(&a[1]);
                let name = self.ident(&a[0]).unwrap_or_default();
                self.define(&a[2], field(recv, name), out);
            }
            PropSet => {
                let recv = self.take(&a[1]);
                let v = self.take(&a[2]);
                let name = self.ident(&a[0]).unwrap_or_default();
                self.emit(Stmt::Assign(field(recv, name), v), out);
            }
            ArrayCreate => {
                let ty = self
                    .ident(&a[0])
                    .and_then(|d| self.types.get(&d.to_ascii_lowercase()).cloned());
                let elem = ty.as_deref().unwrap_or("Int").trim_end_matches("[]").to_owned();
                let size = self.take(&a[1]);
                self.define(&a[0], Expr::New(elem, Box::new(size)), out);
            }
            ArrayLength => {
                let arr = self.take(&a[1]);
                self.define(&a[0], Expr::Field(Box::new(arr), "Length".to_owned()), out);
            }
            ArrayGetElement => {
                let arr = self.take(&a[1]);
                let i = self.take(&a[2]);
                self.define(&a[0], Expr::Index(Box::new(arr), Box::new(i)), out);
            }
            ArraySetElement => {
                let arr = self.take(&a[0]);
                let i = self.take(&a[1]);
                let v = self.take(&a[2]);
                self.emit(Stmt::Assign(Expr::Index(Box::new(arr), Box::new(i)), v), out);
            }
            ArrayFindElement | ArrayRFindElement => {
                let arr = self.take(&a[0]);
                let value = self.take(&a[2]);
                let start = self.take(&a[3]);
                let (name, default) =
                    if inst.opcode == ArrayFindElement { ("Find", 0) } else { ("RFind", -1) };
                let mut args = vec![value];
                if start != Expr::Int(default) {
                    args.push(start);
                }
                self.define(&a[1], Expr::Call(Some(Box::new(arr)), name.to_owned(), args), out);
            }
        }
    }

    /// The variadic arguments of a call, after the count at `count`.
    fn call_args(&mut self, args: &[VariableValue], count: usize) -> Vec<Expr> {
        args.iter().skip(count).map(|v| self.take(v)).collect::<Vec<_>>()
    }
}

fn is_primitive(ty: &str) -> bool {
    ["bool", "int", "float", "string"].iter().any(|p| ty.eq_ignore_ascii_case(p))
}

fn not(e: Expr) -> Expr {
    match e {
        Expr::Not(inner) => *inner,
        Expr::Binary(BinOp::Eq, l, r) => Expr::Binary(BinOp::Ne, l, r),
        Expr::Binary(BinOp::Ne, l, r) => Expr::Binary(BinOp::Eq, l, r),
        e => Expr::Not(Box::new(e)),
    }
}

/// `recv.name`, or `name` on `Self`.
fn field(recv: Expr, name: String) -> Expr {
    match recv {
        Expr::SelfRef => Expr::Name(name),
        r => Expr::Field(Box::new(r), name),
    }
}

/// Declares the locals of the function: at their first assignment when it
/// starts the innermost block using them, else before their first use.
fn declare_locals(cx: &ScriptCx<'_>, info: &FunctionInfo, stmts: &mut Vec<Stmt>) {
    let mut unused = Vec::new();
    for local in &info.locals {
        let name = cx.str(local.name);
        if name.starts_with("::") {
            continue;
        }
        let ty = cx.str(local.ty);
        if !place(stmts, &name, &ty) {
            unused.push(Stmt::Local(ty, name, None));
        }
    }
    // Locals no code uses still keep their slot.
    stmts.splice(0..0, unused);
}

fn uses(stmt: &Stmt, name: &str) -> bool {
    let mut found = false;
    stmt.names(&mut |n| found |= n.eq_ignore_ascii_case(name));
    found
}

/// Declares `name` in `stmts`; `false` when no statement uses it.
fn place(stmts: &mut Vec<Stmt>, name: &str, ty: &str) -> bool {
    let users: Vec<usize> = (0..stmts.len()).filter(|&i| uses(&stmts[i], name)).collect();
    let Some(&first) = users.first() else { return false };
    if users.len() == 1 {
        // Used in one nested block only: declare it there.
        let placed = match &mut stmts[first] {
            Stmt::If(branches, else_branch) => {
                let cond_use = branches.iter().any(|(c, _)| {
                    let mut f = false;
                    c.names(&mut |n| f |= n.eq_ignore_ascii_case(name));
                    f
                });
                let mut blocks: Vec<&mut Vec<Stmt>> =
                    branches.iter_mut().map(|(_, b)| b).chain(else_branch.iter_mut()).collect();
                let using: Vec<usize> = (0..blocks.len())
                    .filter(|&k| blocks[k].iter().any(|s| uses(s, name)))
                    .collect();
                !cond_use && using.len() == 1 && place(blocks[using[0]], name, ty)
            }
            Stmt::While(cond, body) => {
                let mut f = false;
                cond.names(&mut |n| f |= n.eq_ignore_ascii_case(name));
                !f && place(body, name, ty)
            }
            _ => false,
        };
        if placed {
            return true;
        }
    }
    let init = match &stmts[first] {
        Stmt::Assign(Expr::Name(n), value) if n.eq_ignore_ascii_case(name) => {
            let mut self_ref = false;
            value.names(&mut |m| self_ref |= m.eq_ignore_ascii_case(name));
            (!self_ref).then(|| value.clone())
        }
        _ => None,
    };
    match init {
        Some(value) => stmts[first] = Stmt::Local(ty.to_owned(), name.to_owned(), Some(value)),
        None => stmts.insert(first, Stmt::Local(ty.to_owned(), name.to_owned(), None)),
    }
    true
}
