//! The reconstructed source: expressions and statements, and their printing.

use std::fmt::Write as _;

/// A binary operator, with its precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl BinOp {
    fn as_str(self) -> &'static str {
        match self {
            BinOp::Or => "||",
            BinOp::And => "&&",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
        }
    }

    fn precedence(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
            BinOp::Add | BinOp::Sub => 4,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 5,
        }
    }

    /// Operators that can be written as `x op= y`.
    fn compound(self) -> bool {
        matches!(self, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem)
    }
}

const UNARY: u8 = 6;
const CAST: u8 = 7;
const POSTFIX: u8 = 8;

/// An expression.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Expr {
    None,
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(String),
    /// A local, parameter, variable, property or script name.
    Name(String),
    SelfRef,
    ParentRef,
    Field(Box<Expr>, String),
    /// A call; without receiver for functions of `Self`.
    Call(Option<Box<Expr>>, String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, String),
    /// A conversion the compiler inserts by itself (to `Bool`, to `String`,
    /// `Int` to `Float`): printed as the inner expression.
    Implicit(Box<Expr>, String),
    New(String, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

impl Expr {
    fn precedence(&self) -> u8 {
        match self {
            Expr::Binary(op, ..) => op.precedence(),
            Expr::Not(_) | Expr::Neg(_) => UNARY,
            Expr::Cast(..) => CAST,
            Expr::Implicit(e, _) => e.precedence(),
            // Negative numbers print with their sign.
            Expr::Int(i) if *i < 0 => UNARY,
            Expr::Float(f) if f.is_sign_negative() => UNARY,
            _ => POSTFIX,
        }
    }

    /// Whether evaluating the expression may have effects (calls).
    pub(crate) fn has_call(&self) -> bool {
        match self {
            Expr::Call(..) => true,
            Expr::Field(e, _)
            | Expr::Cast(e, _)
            | Expr::Implicit(e, _)
            | Expr::New(_, e)
            | Expr::Not(e)
            | Expr::Neg(e) => e.has_call(),
            Expr::Index(a, b) | Expr::Binary(_, a, b) => a.has_call() || b.has_call(),
            _ => false,
        }
    }

    /// Calls `f` on every name the expression uses.
    pub(crate) fn names(&self, f: &mut dyn FnMut(&str)) {
        match self {
            Expr::Name(n) => f(n),
            Expr::Field(e, _)
            | Expr::Cast(e, _)
            | Expr::Implicit(e, _)
            | Expr::New(_, e)
            | Expr::Not(e)
            | Expr::Neg(e) => e.names(f),
            Expr::Index(a, b) | Expr::Binary(_, a, b) => {
                a.names(f);
                b.names(f);
            }
            Expr::Call(recv, _, args) => {
                if let Some(r) = recv {
                    r.names(f);
                }
                for a in args {
                    a.names(f);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn write(&self, out: &mut String) {
        match self {
            Expr::None => out.push_str("None"),
            Expr::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Expr::Float(f) => out.push_str(&float(*f)),
            Expr::Bool(b) => out.push_str(if *b { "True" } else { "False" }),
            Expr::Str(s) => string(out, s),
            Expr::Name(n) => out.push_str(n),
            Expr::SelfRef => out.push_str("Self"),
            Expr::ParentRef => out.push_str("Parent"),
            Expr::Field(e, name) => {
                self.child(out, e, POSTFIX);
                out.push('.');
                out.push_str(name);
            }
            Expr::Call(recv, name, args) => {
                if let Some(r) = recv {
                    self.child(out, r, POSTFIX);
                    out.push('.');
                }
                out.push_str(name);
                out.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    a.write(out);
                }
                out.push(')');
            }
            Expr::Index(a, i) => {
                self.child(out, a, POSTFIX);
                out.push('[');
                i.write(out);
                out.push(']');
            }
            Expr::Implicit(e, _) => e.write(out),
            Expr::Cast(e, ty) => {
                self.child(out, e, CAST);
                out.push_str(" as ");
                out.push_str(ty);
            }
            Expr::New(elem, size) => {
                out.push_str("new ");
                out.push_str(elem);
                out.push('[');
                size.write(out);
                out.push(']');
            }
            Expr::Not(e) => {
                out.push('!');
                self.child(out, e, UNARY);
            }
            Expr::Neg(e) => {
                out.push('-');
                self.child(out, e, UNARY);
            }
            Expr::Binary(op, l, r) => {
                let p = op.precedence();
                self.child(out, l, p);
                let _ = write!(out, " {} ", op.as_str());
                // Left associative: parenthesize a right operand of the same
                // precedence.
                self.child(out, r, p + 1);
            }
        }
    }

    fn child(&self, out: &mut String, e: &Expr, min: u8) {
        // `(i as Float) / 2` reads better than `i as Float / 2`.
        let cast_operand = matches!(self, Expr::Binary(..)) && matches!(e, Expr::Cast(..));
        if e.precedence() < min || cast_operand {
            out.push('(');
            e.write(out);
            out.push(')');
        } else {
            e.write(out);
        }
    }
}

fn float(f: f32) -> String {
    let mut s = format!("{f}");
    if !s.contains(['.', 'e', 'E', 'i', 'N']) {
        s.push_str(".0");
    }
    s
}

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A statement.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stmt {
    /// `Type name = value`
    Local(String, String, Option<Expr>),
    Assign(Expr, Expr),
    Expr(Expr),
    Return(Option<Expr>),
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
}

impl Stmt {
    /// Calls `f` on every name the statement uses, nested blocks included.
    pub(crate) fn names(&self, f: &mut dyn FnMut(&str)) {
        match self {
            Stmt::Local(_, name, init) => {
                f(name);
                if let Some(e) = init {
                    e.names(f);
                }
            }
            Stmt::Assign(t, v) => {
                t.names(f);
                v.names(f);
            }
            Stmt::Expr(e) | Stmt::Return(Some(e)) => e.names(f),
            Stmt::Return(None) => {}
            Stmt::If(branches, else_branch) => {
                for (c, b) in branches {
                    c.names(f);
                    for s in b {
                        s.names(f);
                    }
                }
                for s in else_branch.iter().flatten() {
                    s.names(f);
                }
            }
            Stmt::While(c, b) => {
                c.names(f);
                for s in b {
                    s.names(f);
                }
            }
        }
    }
}

/// Prints statements, indented by `depth` levels of four spaces.
pub(crate) fn write_block(out: &mut String, stmts: &[Stmt], depth: usize) {
    for s in stmts {
        write_stmt(out, s, depth);
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}

fn write_stmt(out: &mut String, stmt: &Stmt, depth: usize) {
    indent(out, depth);
    match stmt {
        Stmt::Local(ty, name, init) => {
            let _ = write!(out, "{ty} {name}");
            if let Some(e) = init {
                out.push_str(" = ");
                e.write(out);
            }
        }
        Stmt::Assign(target, value) => {
            target.write(out);
            match value {
                Expr::Binary(op, l, r) if op.compound() && **l == *target => {
                    let _ = write!(out, " {}= ", op.as_str());
                    r.write(out);
                }
                _ => {
                    out.push_str(" = ");
                    value.write(out);
                }
            }
        }
        Stmt::Expr(e) => e.write(out),
        Stmt::Return(value) => {
            out.push_str("Return");
            if let Some(e) = value {
                out.push(' ');
                e.write(out);
            }
        }
        Stmt::If(branches, else_branch) => {
            for (i, (cond, body)) in branches.iter().enumerate() {
                if i > 0 {
                    indent(out, depth);
                }
                out.push_str(if i == 0 { "If " } else { "ElseIf " });
                cond.write(out);
                out.push('\n');
                write_block(out, body, depth + 1);
            }
            if let Some(body) = else_branch {
                indent(out, depth);
                out.push_str("Else\n");
                write_block(out, body, depth + 1);
            }
            indent(out, depth);
            out.push_str("EndIf");
        }
        Stmt::While(cond, body) => {
            out.push_str("While ");
            cond.write(out);
            out.push('\n');
            write_block(out, body, depth + 1);
            indent(out, depth);
            out.push_str("EndWhile");
        }
    }
    out.push('\n');
}
