//! The built-in natives: what can be done without the game.
//!
//! - `Debug.Trace`, `Debug.Notification`, `Debug.MessageBox`: to
//!   [`Vm::output`];
//! - `Math.*`: computed (angles in degrees, like the game);
//! - `Utility.RandomInt`, `Utility.RandomFloat`: from [`Vm::seed`];
//! - `Utility.Wait`, `Utility.WaitGameTime`: advance [`Vm::clock`];
//!   `Utility.GetCurrentRealTime`, `Utility.GetCurrentGameTime` read it;
//! - `StringUtil.GetLength`, `StringUtil.Substring`, `StringUtil.Find`.

use std::rc::Rc;

use crate::{Value, Vm};

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or_default()
}

fn text(args: &[Value], i: usize) -> String {
    arg(args, i).to_string()
}

fn random(vm: &mut Vm) -> u64 {
    // xorshift64*
    let mut x = vm.seed;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    vm.seed = x;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

pub(crate) fn register_builtins(vm: &mut Vm) {
    for name in ["Trace", "Notification", "MessageBox", "TraceStack"] {
        vm.register_native("Debug", name, |vm, _, args| {
            vm.output.push(text(args, 0));
            Value::None
        });
    }

    let float = |f: fn(f32) -> f32| {
        move |_: &mut Vm, _, args: &[Value]| Value::Float(f(arg(args, 0).as_float()))
    };
    vm.register_native("Math", "Abs", float(f32::abs));
    vm.register_native("Math", "Sqrt", float(f32::sqrt));
    vm.register_native("Math", "Sin", float(|d| d.to_radians().sin()));
    vm.register_native("Math", "Cos", float(|d| d.to_radians().cos()));
    vm.register_native("Math", "Tan", float(|d| d.to_radians().tan()));
    vm.register_native("Math", "aSin", float(|x| x.asin().to_degrees()));
    vm.register_native("Math", "aCos", float(|x| x.acos().to_degrees()));
    vm.register_native("Math", "aTan", float(|x| x.atan().to_degrees()));
    vm.register_native("Math", "Floor", |_, _, args| {
        Value::Int(arg(args, 0).as_float().floor() as i32)
    });
    vm.register_native("Math", "Ceiling", |_, _, args| {
        Value::Int(arg(args, 0).as_float().ceil() as i32)
    });
    vm.register_native("Math", "Pow", |_, _, args| {
        Value::Float(arg(args, 0).as_float().powf(arg(args, 1).as_float()))
    });

    vm.register_native("Utility", "RandomInt", |vm, _, args| {
        let (lo, hi) = (arg(args, 0).as_int(), arg(args, 1).as_int());
        let span = (i64::from(hi) - i64::from(lo) + 1).max(1) as u64;
        Value::Int((i64::from(lo) + (random(vm) % span) as i64) as i32)
    });
    vm.register_native("Utility", "RandomFloat", |vm, _, args| {
        let (lo, hi) = (arg(args, 0).as_float(), arg(args, 1).as_float());
        let unit = (random(vm) >> 40) as f32 / (1u64 << 24) as f32;
        Value::Float(lo + (hi - lo) * unit)
    });
    vm.register_native("Utility", "Wait", |vm, _, args| {
        vm.clock += arg(args, 0).as_float().max(0.0);
        Value::None
    });
    vm.register_native("Utility", "WaitGameTime", |vm, _, args| {
        // Game hours to seconds, at the default time scale of 20.
        vm.clock += arg(args, 0).as_float().max(0.0) * 3600.0 / 20.0;
        Value::None
    });
    vm.register_native("Utility", "GetCurrentRealTime", |vm, _, _| Value::Float(vm.clock));
    vm.register_native("Utility", "GetCurrentGameTime", |vm, _, _| {
        Value::Float(vm.clock * 20.0 / 3600.0 / 24.0)
    });

    vm.register_native("StringUtil", "GetLength", |_, _, args| {
        Value::Int(text(args, 0).chars().count() as i32)
    });
    vm.register_native("StringUtil", "Substring", |_, _, args| {
        let s: Vec<char> = text(args, 0).chars().collect();
        let start = arg(args, 1).as_int().clamp(0, s.len() as i32) as usize;
        let len = arg(args, 2).as_int();
        let end = if len <= 0 { s.len() } else { (start + len as usize).min(s.len()) };
        Value::Str(Rc::from(s[start..end].iter().collect::<String>()))
    });
    vm.register_native("StringUtil", "Find", |_, _, args| {
        let (s, needle) = (text(args, 0).to_ascii_lowercase(), text(args, 1).to_ascii_lowercase());
        let start = arg(args, 2).as_int().max(0) as usize;
        let found = s.get(start..).and_then(|rest| rest.find(&needle)).map(|i| (i + start) as i32);
        Value::Int(found.unwrap_or(-1))
    });
}
