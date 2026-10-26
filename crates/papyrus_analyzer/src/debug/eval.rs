//! Expressions of the debugger (watch, hover, debug console): Papyrus
//! expressions over the locals of a frame and the fields of `Self`.
//!
//! Evaluating has no side effect: names, members (auto properties and
//! variables), `Length`, indexes, literals, operators and `As` casts between
//! primitives. Calls are refused, since they would run code.

use std::{cmp::Ordering, rc::Rc};

use papyrus_parser::{
    SyntaxKind,
    ast::{self, AstNode, Expr, LiteralKind},
};
use papyrus_vm::{StackFrame, Value, Vm};

/// The value of `text` in `frame` (none: only literals and operators).
pub(super) fn evaluate(vm: &Vm, frame: Option<&StackFrame>, text: &str) -> Result<Value, String> {
    let src =
        format!("ScriptName DebugWatch\nFunction DebugWatch()\n    Return {text}\nEndFunction\n");
    let parse = papyrus_parser::parse(&src);
    if !parse.errors().is_empty() {
        return Err(format!("`{text}` is not an expression"));
    }
    let expr = (parse.syntax_node().descendants())
        .find_map(ast::ReturnStmt::cast)
        .and_then(|r| r.expr())
        .ok_or_else(|| format!("`{text}` is not an expression"))?;
    let end = expr.syntax().text_range().end();
    // Nothing else on the line of `Return`.
    if src[usize::from(end)..].lines().next().is_some_and(|l| !l.trim().is_empty()) {
        return Err(format!("`{text}` is not one expression"));
    }
    Eval { vm, frame }.expr(expr)
}

struct Eval<'v> {
    vm: &'v Vm,
    frame: Option<&'v StackFrame>,
}

impl Eval<'_> {
    fn expr(&self, expr: Expr<'_>) -> Result<Value, String> {
        let missing = || "incomplete expression".to_owned();
        match expr {
            Expr::Paren(p) => self.expr(p.expr().ok_or_else(missing)?),
            Expr::Literal(l) => literal(l),
            Expr::NameRef(n) => self.name(n.text()),
            Expr::Field(f) => {
                let receiver = self.expr(f.receiver().ok_or_else(missing)?)?;
                let name = f.name_ref().ok_or_else(missing)?;
                self.member(&receiver, name.text())
            }
            Expr::Index(i) => {
                let base = self.expr(i.base().ok_or_else(missing)?)?;
                let index = self.expr(i.index().ok_or_else(missing)?)?.as_int();
                match base {
                    Value::Array(items) => {
                        let items = items.borrow();
                        usize::try_from(index).ok().and_then(|i| items.get(i).cloned()).ok_or_else(
                            || {
                                format!(
                                    "index {index} is out of range (0-{})",
                                    items.len() as i32 - 1
                                )
                            },
                        )
                    }
                    other => Err(format!("{} is not an array", show(&other))),
                }
            }
            Expr::Prefix(p) => {
                let value = self.expr(p.expr().ok_or_else(missing)?)?;
                match p.op().map(|t| t.kind()) {
                    Some(SyntaxKind::BANG) => Ok(Value::Bool(!value.truthy())),
                    _ => match value {
                        Value::Float(f) => Ok(Value::Float(-f)),
                        other => Ok(Value::Int(other.as_int().wrapping_neg())),
                    },
                }
            }
            Expr::Bin(b) => self.binary(b),
            Expr::Cast(c) => {
                let value = self.expr(c.expr().ok_or_else(missing)?)?;
                let ty = c.ty().ok_or_else(missing)?;
                cast(value, ty.base_text(), ty.is_array())
            }
            Expr::Call(_) => Err("calls are not evaluated: they would run code".to_owned()),
            Expr::New(_) => Err("`New` is not evaluated".to_owned()),
        }
    }

    /// A local, `Self`, or a field of `Self`.
    fn name(&self, name: &str) -> Result<Value, String> {
        let Some(frame) = self.frame else { return Err(format!("unknown name `{name}`")) };
        if name.eq_ignore_ascii_case("self") {
            return Ok(frame.this.map_or(Value::None, Value::Object));
        }
        if let Some((_, v)) = frame.locals.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            return Ok(v.clone());
        }
        match frame.this {
            Some(this) => self
                .member(&Value::Object(this), name)
                .map_err(|_| format!("unknown name `{name}`")),
            None => Err(format!("unknown name `{name}`")),
        }
    }

    /// `receiver.name`: an auto property or a variable, `Length` of arrays.
    fn member(&self, receiver: &Value, name: &str) -> Result<Value, String> {
        match receiver {
            Value::Array(items) if name.eq_ignore_ascii_case("length") => {
                Ok(Value::Int(items.borrow().len() as i32))
            }
            Value::Object(obj) => {
                let (script, fields) = self.vm.fields(*obj);
                fields
                    .into_iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v)
                    .ok_or_else(|| format!("`{script}` has no auto property or variable `{name}`"))
            }
            Value::None => Err(format!("cannot get `{name}` of None")),
            other => Err(format!("{} has no member `{name}`", show(other))),
        }
    }

    fn binary(&self, b: ast::BinExpr<'_>) -> Result<Value, String> {
        let missing = || "incomplete expression".to_owned();
        let op = b.op().map(|t| t.kind()).ok_or_else(missing)?;
        let lhs = self.expr(b.lhs().ok_or_else(missing)?)?;
        // `&&` and `||` short-circuit, like Papyrus.
        match op {
            SyntaxKind::AMP2 if !lhs.truthy() => return Ok(Value::Bool(false)),
            SyntaxKind::PIPE2 if lhs.truthy() => return Ok(Value::Bool(true)),
            _ => {}
        }
        let rhs = self.expr(b.rhs().ok_or_else(missing)?)?;
        let floats = matches!(lhs, Value::Float(_)) || matches!(rhs, Value::Float(_));
        let strings = matches!(lhs, Value::Str(_)) || matches!(rhs, Value::Str(_));
        Ok(match op {
            SyntaxKind::AMP2 | SyntaxKind::PIPE2 => Value::Bool(rhs.truthy()),
            SyntaxKind::EQ2 => Value::Bool(lhs.loose_eq(&rhs)),
            SyntaxKind::NEQ => Value::Bool(!lhs.loose_eq(&rhs)),
            SyntaxKind::LT => Value::Bool(lhs.compare(&rhs) == Ordering::Less),
            SyntaxKind::LTEQ => Value::Bool(lhs.compare(&rhs) != Ordering::Greater),
            SyntaxKind::GT => Value::Bool(lhs.compare(&rhs) == Ordering::Greater),
            SyntaxKind::GTEQ => Value::Bool(lhs.compare(&rhs) != Ordering::Less),
            SyntaxKind::PLUS if strings => Value::Str(Rc::from(format!("{lhs}{rhs}"))),
            SyntaxKind::PLUS | SyntaxKind::MINUS | SyntaxKind::STAR | SyntaxKind::SLASH
                if floats =>
            {
                let (x, y) = (lhs.as_float(), rhs.as_float());
                Value::Float(match op {
                    SyntaxKind::PLUS => x + y,
                    SyntaxKind::MINUS => x - y,
                    SyntaxKind::STAR => x * y,
                    _ => x / y,
                })
            }
            SyntaxKind::PLUS
            | SyntaxKind::MINUS
            | SyntaxKind::STAR
            | SyntaxKind::SLASH
            | SyntaxKind::PERCENT => {
                let (x, y) = (lhs.as_int(), rhs.as_int());
                Value::Int(match op {
                    SyntaxKind::PLUS => x.wrapping_add(y),
                    SyntaxKind::MINUS => x.wrapping_sub(y),
                    SyntaxKind::STAR => x.wrapping_mul(y),
                    _ if y == 0 => return Err("divide by zero".to_owned()),
                    SyntaxKind::SLASH => x.wrapping_div(y),
                    _ => x.wrapping_rem(y),
                })
            }
            _ => return Err("unknown operator".to_owned()),
        })
    }
}

fn literal(l: ast::Literal<'_>) -> Result<Value, String> {
    let token = l.token().ok_or("incomplete expression")?;
    let text = token.text();
    Ok(match l.kind().ok_or("incomplete expression")? {
        LiteralKind::Int => {
            let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                Some(hex) => u32::from_str_radix(hex, 16).map(|v| v as i32).ok(),
                None => text.parse().ok(),
            };
            Value::Int(parsed.ok_or_else(|| format!("invalid number `{text}`"))?)
        }
        LiteralKind::Float => {
            Value::Float(text.parse().map_err(|_| format!("invalid number `{text}`"))?)
        }
        LiteralKind::String => {
            let inner = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(text);
            let mut out = String::new();
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c != '\\' {
                    out.push(c);
                    continue;
                }
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => {}
                }
            }
            Value::Str(Rc::from(out))
        }
        LiteralKind::Bool => Value::Bool(text.eq_ignore_ascii_case("true")),
        LiteralKind::None => Value::None,
    })
}

/// `value As ty`, between primitives.
fn cast(value: Value, ty: &str, array: bool) -> Result<Value, String> {
    if array {
        return Err("casts to arrays are not evaluated".to_owned());
    }
    Ok(match ty.to_ascii_lowercase().as_str() {
        "int" => Value::Int(value.as_int()),
        "float" => Value::Float(value.as_float()),
        "bool" => Value::Bool(value.truthy()),
        "string" => match value {
            Value::Str(s) => Value::Str(s),
            other => Value::Str(Rc::from(other.to_string())),
        },
        _ => return Err(format!("casts to `{ty}` are not evaluated")),
    })
}

/// A value in a message.
fn show(value: &Value) -> String {
    match value {
        Value::Str(s) => format!("{s:?}"),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use papyrus_vm::{StackFrame, Value};

    #[test]
    fn evaluates_locals_operators_and_literals() {
        struct NoScripts;
        impl papyrus_vm::ScriptLoader for NoScripts {
            fn load(&mut self, _: &str) -> Option<papyrus_vm::Script> {
                None
            }
        }
        let vm = papyrus_vm::Vm::new(NoScripts);
        let items = Value::Array(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Int(10),
            Value::Int(20),
        ])));
        let frame = StackFrame {
            script: "T".to_owned(),
            function: "F".to_owned(),
            line: Some(1),
            this: None,
            locals: vec![
                ("aiCount".to_owned(), Value::Int(3)),
                ("afScale".to_owned(), Value::Float(0.5)),
                ("akItems".to_owned(), items),
            ],
        };
        let eval = |text: &str| match super::evaluate(&vm, Some(&frame), text) {
            Ok(v) => v.to_string(),
            Err(e) => format!("error: {e}"),
        };
        assert_eq!(eval("aicount + 1"), "4");
        assert_eq!(eval("aiCount * afScale"), "1.500000");
        assert_eq!(eval("akItems[1] - akItems.Length"), "18");
        assert_eq!(eval("\"n=\" + aiCount"), "n=3");
        assert_eq!(eval("aiCount > 2 && !(afScale == 0.5)"), "False");
        assert_eq!(eval("(aiCount As Float) / 2"), "1.500000");
        assert_eq!(eval("0x10 % 3"), "1");
        assert_eq!(eval("akItems[5]"), "error: index 5 is out of range (0-1)");
        assert_eq!(eval("Missing"), "error: unknown name `Missing`");
        assert_eq!(eval("Debug.Trace(1)"), "error: calls are not evaluated: they would run code");
        assert_eq!(eval("aiCount +"), "error: `aiCount +` is not an expression");
    }
}
