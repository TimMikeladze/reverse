//! Runtime implementations of the built-in functions and container methods.

use crate::{error, Ctrl, Value, Vm};
use reverse_syntax::Span;
use std::rc::Rc;

type R = Result<Value, Ctrl>;

const NAMES: &[&str] = &[
    "Print",
    "ToString",
    "Abs",
    "Min",
    "Max",
    "Floor",
    "Ceil",
    "Round",
    "Int",
    "Sqrt",
    "Pow",
    "Sin",
    "Cos",
    "Tan",
    "ArcSin",
    "ArcCos",
    "ArcTan",
    "Exp",
    "Ln",
    "Mod",
    "Quotient",
    "GetRandomInt",
    "GetRandomFloat",
    "Shuffle",
    "Sleep",
    "GetSimulationElapsedTime",
    "Join",
    "Concatenate",
    "ConcatenateMaps",
];

const METHODS: &[&str] = &[
    "Find",
    "Slice",
    "RemoveFirstElement",
    "RemoveAllElements",
    "RemoveElement",
    "Remove",
    "ReplaceElement",
    "ReplaceFirstElement",
    "ReplaceAllElements",
    "Insert",
];

/// Is this the name of a built-in container method?
pub fn is_method(name: &str) -> bool {
    METHODS.contains(&name)
}

pub fn is_builtin(name: &str) -> bool {
    NAMES.contains(&name)
}

fn bad(name: &str, args: &[Value], span: Span) -> Ctrl {
    let types: Vec<String> = args.iter().map(|a| a.type_name()).collect();
    error(
        "runtime_error",
        format!("`{}` cannot be called with ({})", name, types.join(", ")),
        span,
    )
}

fn float_to_int(f: f64) -> R {
    if !f.is_finite() || !(-9.3e18..=9.3e18).contains(&f) {
        return Err(Ctrl::Fail);
    }
    Ok(Value::Int(f as i64))
}

pub fn call(vm: &mut Vm, name: &str, args: Vec<Value>, _bracket: bool, span: Span) -> R {
    use Value::*;
    match (name, args.as_slice()) {
        ("Print", [msg, ..]) => {
            vm.print(msg.display(), span)?;
            Ok(Void)
        }
        ("ToString", [v]) => Ok(Value::str(&v.display())),
        ("Abs", [Int(i)]) => i
            .checked_abs()
            .map(Int)
            .ok_or_else(|| error("overflow", "integer overflow", span)),
        ("Abs", [Float(f)]) => Ok(Float(f.abs())),
        ("Min", [Int(a), Int(b)]) => Ok(Int(*a.min(b))),
        ("Max", [Int(a), Int(b)]) => Ok(Int(*a.max(b))),
        // The Book: Min and Max propagate NaN.
        ("Min", [Float(a), Float(b)]) => Ok(Float(if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.min(*b)
        })),
        ("Max", [Float(a), Float(b)]) => Ok(Float(if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.max(*b)
        })),
        ("Floor", [Int(i)]) | ("Ceil", [Int(i)]) => Ok(Int(*i)),
        ("Floor", [Rational(n, d)]) => Ok(Int(n.div_euclid(*d))),
        ("Ceil", [Rational(n, d)]) => Ok(Int((-((-(*n as i128)).div_euclid(*d as i128))) as i64)),
        ("Floor", [Float(f)]) => float_to_int(f.floor()),
        ("Ceil", [Float(f)]) => float_to_int(f.ceil()),
        ("Round", [Float(f)]) => float_to_int(f.round_ties_even()),
        ("Int", [Float(f)]) => float_to_int(f.trunc()),
        ("Sqrt", [Float(f)]) => Ok(Float(f.sqrt())),
        ("Pow", [Float(a), Float(b)]) => Ok(Float(a.powf(*b))),
        ("Sin", [Float(f)]) => Ok(Float(f.sin())),
        ("Cos", [Float(f)]) => Ok(Float(f.cos())),
        ("Tan", [Float(f)]) => Ok(Float(f.tan())),
        ("ArcSin", [Float(f)]) => Ok(Float(f.asin())),
        ("ArcCos", [Float(f)]) => Ok(Float(f.acos())),
        ("ArcTan", [Float(f)]) => Ok(Float(f.atan())),
        ("Exp", [Float(f)]) => Ok(Float(f.exp())),
        ("Ln", [Float(f)]) => Ok(Float(f.ln())),
        ("Mod", [Int(a), Int(b)]) => {
            if *b == 0 {
                return Err(Ctrl::Fail);
            }
            // Floored modulo: the result takes the sign of the divisor.
            let r = (*a as i128).rem_euclid(*b as i128);
            Ok(Int(
                (if *b < 0 && r != 0 { r + *b as i128 } else { r }) as i64
            ))
        }
        ("Quotient", [Int(a), Int(b)]) => {
            if *b == 0 {
                return Err(Ctrl::Fail);
            }
            // Floored division, so Quotient * B + Mod = A.
            let q = a
                .checked_div(*b)
                .ok_or_else(|| error("overflow", "integer overflow", span))?;
            Ok(Int(if (a % b != 0) && ((*a < 0) != (*b < 0)) {
                q - 1
            } else {
                q
            }))
        }
        ("GetRandomInt", [Int(lo), Int(hi)]) => {
            let (lo, hi) = (*lo.min(hi), *lo.max(hi));
            let span_len = (hi as i128 - lo as i128 + 1) as u128;
            let r = (vm.next_random() as u128 % span_len) as i128;
            Ok(Int((lo as i128 + r) as i64))
        }
        ("GetRandomFloat", [Float(lo), Float(hi)]) => {
            let unit = (vm.next_random() >> 11) as f64 / (1u64 << 53) as f64;
            Ok(Float(lo + (hi - lo) * unit))
        }
        ("Shuffle", [Array(a)]) => {
            let mut v = (**a).clone();
            for i in (1..v.len()).rev() {
                let j = (vm.next_random() % (i as u64 + 1)) as usize;
                v.swap(i, j);
            }
            Ok(Array(Rc::new(v)))
        }
        ("Sleep", _) => Err(error(
            "runtime_error",
            "`Sleep` suspends, so it cannot run inside a failure context",
            span,
        )),
        ("GetSimulationElapsedTime", []) => Ok(Float(vm.now)),
        ("Join", [Array(parts), Str(sep)]) => {
            let strs: Vec<String> = parts.iter().map(|p| p.display()).collect();
            Ok(Value::str(&strs.join(sep)))
        }
        ("Concatenate", items) => {
            if !items.is_empty() && items.iter().all(|i| matches!(i, Str(_))) {
                let s: String = items.iter().map(|i| i.display()).collect();
                return Ok(Value::str(&s));
            }
            let mut out = Vec::new();
            for it in items {
                match it {
                    Array(a) => out.extend(a.iter().cloned()),
                    Str(s) => out.extend(s.chars().map(Char)),
                    _ => return Err(bad(name, &args, span)),
                }
            }
            Ok(Array(Rc::new(out)))
        }
        ("ConcatenateMaps", items) => {
            let mut out: Vec<(Value, Value)> = Vec::new();
            for it in items {
                let Map(m) = it else {
                    return Err(bad(name, &args, span));
                };
                for (k, v) in m.iter() {
                    match out.iter_mut().find(|(ek, _)| ek.equals(k)) {
                        Some(entry) => entry.1 = v.clone(),
                        None => out.push((k.clone(), v.clone())),
                    }
                }
            }
            Ok(Map(Rc::new(out)))
        }
        _ => Err(bad(name, &args, span)),
    }
}

/// Methods on arrays. Strings are arrays of characters, so they share them.
pub fn method(vm: &mut Vm, recv: Value, name: &str, args: Vec<Value>, span: Span) -> R {
    let _ = vm;
    let is_str = matches!(recv, Value::Str(_));
    let items: Vec<Value> = match &recv {
        Value::Array(a) => (**a).clone(),
        Value::Str(s) => s.chars().map(Value::Char).collect(),
        _ => return Err(bad(name, &args, span)),
    };
    let wrap = |v: Vec<Value>| -> Value {
        if is_str {
            Value::str(&v.iter().map(|c| c.display()).collect::<String>())
        } else {
            Value::Array(Rc::new(v))
        }
    };
    // A string argument where an array is expected.
    let as_items = |v: &Value| -> Option<Vec<Value>> {
        match v {
            Value::Array(a) => Some((**a).clone()),
            Value::Str(s) => Some(s.chars().map(Value::Char).collect()),
            _ => None,
        }
    };
    let index = |v: &Value, max: usize| -> Option<usize> {
        match v {
            Value::Int(i) if *i >= 0 && (*i as usize) <= max => Some(*i as usize),
            _ => None,
        }
    };
    let n = items.len();
    match (name, args.as_slice()) {
        ("Find", [x]) => match items.iter().position(|v| v.equals(x)) {
            Some(i) => Ok(Value::Int(i as i64)),
            None => Err(Ctrl::Fail),
        },
        ("Slice", [s]) => match index(s, n) {
            Some(s) => Ok(wrap(items[s..].to_vec())),
            None => Err(Ctrl::Fail),
        },
        ("Slice", [s, e]) => match (index(s, n), index(e, n)) {
            (Some(s), Some(e)) if s <= e => Ok(wrap(items[s..e].to_vec())),
            _ => Err(Ctrl::Fail),
        },
        ("RemoveFirstElement", [x]) => match items.iter().position(|v| v.equals(x)) {
            Some(i) => {
                let mut v = items;
                v.remove(i);
                Ok(wrap(v))
            }
            None => Err(Ctrl::Fail),
        },
        ("RemoveAllElements", [x]) => {
            Ok(wrap(items.into_iter().filter(|v| !v.equals(x)).collect()))
        }
        ("RemoveElement", [i]) => match index(i, n) {
            Some(i) if i < n => {
                let mut v = items;
                v.remove(i);
                Ok(wrap(v))
            }
            _ => Err(Ctrl::Fail),
        },
        ("Remove", [s, e]) => match (index(s, n), index(e, n)) {
            (Some(s), Some(e)) if s <= e => {
                let mut v = items;
                v.drain(s..e);
                Ok(wrap(v))
            }
            _ => Err(Ctrl::Fail),
        },
        ("ReplaceElement", [i, x]) => match index(i, n) {
            Some(i) if i < n => {
                let mut v = items;
                v[i] = x.clone();
                Ok(wrap(v))
            }
            _ => Err(Ctrl::Fail),
        },
        ("ReplaceFirstElement", [old, new]) => match items.iter().position(|v| v.equals(old)) {
            Some(i) => {
                let mut v = items;
                v[i] = new.clone();
                Ok(wrap(v))
            }
            None => Err(Ctrl::Fail),
        },
        ("ReplaceAllElements", [old, new]) => Ok(wrap(
            items
                .into_iter()
                .map(|v| if v.equals(old) { new.clone() } else { v })
                .collect(),
        )),
        ("Insert", [i, extra]) => match (index(i, n), as_items(extra)) {
            (Some(i), Some(extra)) => {
                let mut v = items;
                v.splice(i..i, extra);
                Ok(wrap(v))
            }
            _ => Err(Ctrl::Fail),
        },
        _ => Err(error(
            "runtime_error",
            format!("no method `{}` taking {} argument(s)", name, args.len()),
            span,
        )),
    }
}
