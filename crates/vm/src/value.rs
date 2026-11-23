//! Runtime values.
//!
//! Arrays, maps, tuples, strings and structs are immutable values shared by
//! reference count; an update builds a new value. Class instances are
//! references with mutable fields.

use reverse_syntax::ast::FuncDef;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum Value {
    Void,
    Int(i64),
    Float(f64),
    /// Numerator and denominator in lowest terms; the denominator is positive.
    Rational(i64, i64),
    Logic(bool),
    Char(char),
    Str(Rc<str>),
    Array(Rc<Vec<Value>>),
    /// Insertion-ordered key/value pairs.
    Map(Rc<Vec<(Value, Value)>>),
    Opt(Option<Rc<Value>>),
    Tuple(Rc<Vec<Value>>),
    Object(Rc<Object>),
    Struct(Rc<StructVal>),
    Enum {
        ty: Rc<str>,
        value: Rc<str>,
    },
    Func(Rc<FuncVal>),
    Type(Rc<str>),
    /// Handle returned by `spawn`.
    Task(Rc<TaskCell>),
    Event(Rc<EventCell>),
    /// Handle returned by `Subscribe`.
    Cancelable(Rc<CancelCell>),
}

#[derive(Debug)]
pub struct CancelCell {
    pub event: Rc<EventCell>,
    pub id: u64,
}

#[derive(Debug, Default)]
pub struct TaskCell {
    pub result: RefCell<Option<Value>>,
}

/// An `event(t)`. Each signal wakes exactly one awaiter, oldest first.
#[derive(Debug, Default)]
pub struct EventCell {
    pub waiting: RefCell<VecDeque<u64>>,
    pub delivered: RefCell<Vec<(u64, Value)>>,
    pub next_ticket: Cell<u64>,
    /// Handlers registered with `Subscribe`, in subscription order.
    pub subs: RefCell<Vec<(u64, Value)>>,
}

#[derive(Debug)]
pub struct Object {
    pub class: usize,
    pub class_name: Rc<str>,
    pub fields: RefCell<Vec<Value>>,
}

#[derive(Clone, Debug)]
pub struct StructVal {
    pub class: usize,
    pub class_name: Rc<str>,
    pub fields: Vec<Value>,
}

#[derive(Debug)]
pub enum FuncVal {
    User {
        def: Rc<FuncDef>,
        this: Option<Value>,
        class: Option<usize>,
    },
    Builtin(String),
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a as i64
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    /// A normalised rational. `den` must not be zero.
    pub fn rational(num: i64, den: i64) -> Value {
        // Work in 128 bits so the most negative int cannot overflow on negation.
        let g = gcd(num, den).max(1) as i128;
        let (mut n, mut d) = (num as i128 / g, den as i128 / g);
        if d < 0 {
            n = -n;
            d = -d;
        }
        Value::Rational(
            n.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            d.clamp(1, i64::MAX as i128) as i64,
        )
    }

    pub fn type_name(&self) -> String {
        match self {
            Value::Void => "void".into(),
            Value::Int(_) => "int".into(),
            Value::Float(_) => "float".into(),
            Value::Rational(..) => "rational".into(),
            Value::Logic(_) => "logic".into(),
            Value::Char(_) => "char".into(),
            Value::Str(_) => "string".into(),
            Value::Array(_) => "array".into(),
            Value::Map(_) => "map".into(),
            Value::Opt(_) => "option".into(),
            Value::Tuple(_) => "tuple".into(),
            Value::Object(o) => o.class_name.to_string(),
            Value::Struct(s) => s.class_name.to_string(),
            Value::Enum { ty, .. } => ty.to_string(),
            Value::Func(_) => "function".into(),
            Value::Type(_) => "type".into(),
            Value::Task(_) => "task".into(),
            Value::Event(_) => "event".into(),
            Value::Cancelable(_) => "cancelable".into(),
        }
    }

    /// Verse equality: values of different types are never equal; class
    /// instances compare by identity; everything else compares by value.
    pub fn equals(&self, other: &Value) -> bool {
        use Value::*;
        match (self, other) {
            (Void, Void) => true,
            (Int(a), Int(b)) => a == b,
            // The Book: every float equals itself, including NaN.
            (Float(a), Float(b)) => a == b || (a.is_nan() && b.is_nan()),
            (Rational(an, ad), Rational(bn, bd)) => an == bn && ad == bd,
            (Int(a), Rational(n, d)) | (Rational(n, d), Int(a)) => *d == 1 && a == n,
            (Logic(a), Logic(b)) => a == b,
            (Char(a), Char(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Str(s), Array(a)) | (Array(a), Str(s)) => {
                let chars: Vec<char> = s.chars().collect();
                chars.len() == a.len()
                    && chars
                        .iter()
                        .zip(a.iter())
                        .all(|(c, v)| matches!(v, Char(x) if x == c))
            }
            (Array(a), Array(b)) | (Tuple(a), Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Map(a), Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|((ak, av), (bk, bv))| ak.equals(bk) && av.equals(bv))
            }
            (Opt(a), Opt(b)) => match (a, b) {
                (None, None) => true,
                (Some(x), Some(y)) => x.equals(y),
                _ => false,
            },
            (Opt(None), Logic(false)) | (Logic(false), Opt(None)) => true,
            (Object(a), Object(b)) => Rc::ptr_eq(a, b),
            (Task(a), Task(b)) => Rc::ptr_eq(a, b),
            (Event(a), Event(b)) => Rc::ptr_eq(a, b),
            (Struct(a), Struct(b)) => {
                a.class == b.class && a.fields.iter().zip(&b.fields).all(|(x, y)| x.equals(y))
            }
            (Enum { ty: at, value: av }, Enum { ty: bt, value: bv }) => at == bt && av == bv,
            _ => false,
        }
    }

    /// Text as it appears in string interpolation and `Print`.
    pub fn display(&self) -> String {
        match self {
            Value::Str(s) => s.to_string(),
            Value::Char(c) => c.to_string(),
            other => other.repr(),
        }
    }

    /// Text for showing a result to a developer: strings are quoted.
    pub fn repr(&self) -> String {
        match self {
            Value::Void => "void".into(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                if f.is_nan() {
                    "NaN".into()
                } else if f.is_infinite() {
                    if *f > 0.0 {
                        "Inf".into()
                    } else {
                        "-Inf".into()
                    }
                } else {
                    format!("{:.6}", f)
                }
            }
            Value::Rational(n, d) => {
                if *d == 1 {
                    n.to_string()
                } else {
                    format!("{}/{}", n, d)
                }
            }
            Value::Logic(b) => b.to_string(),
            Value::Char(c) => format!("'{}'", c),
            Value::Str(s) => format!("\"{}\"", s),
            Value::Array(a) => format!(
                "array{{{}}}",
                a.iter().map(|v| v.repr()).collect::<Vec<_>>().join(", ")
            ),
            Value::Map(m) => format!(
                "map{{{}}}",
                m.iter()
                    .map(|(k, v)| format!("{} => {}", k.repr(), v.repr()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Opt(None) => "false".into(),
            Value::Opt(Some(v)) => format!("option{{{}}}", v.repr()),
            Value::Tuple(t) => format!(
                "({})",
                t.iter().map(|v| v.repr()).collect::<Vec<_>>().join(", ")
            ),
            Value::Object(o) => format!("{}{{...}}", o.class_name),
            Value::Struct(s) => format!(
                "{}{{{}}}",
                s.class_name,
                s.fields
                    .iter()
                    .map(|v| v.repr())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Enum { ty, value } => format!("{}.{}", ty, value),
            Value::Func(_) => "<function>".into(),
            Value::Task(_) => "<task>".into(),
            Value::Event(_) => "<event>".into(),
            Value::Cancelable(_) => "<cancelable>".into(),
            Value::Type(t) => t.to_string(),
        }
    }
}
