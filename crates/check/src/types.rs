//! The checker's view of types and effects.

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Rational,
    Logic,
    Char,
    Str,
    Void,
    Any,
    /// Bottom: the type of `break`, `return` and empty literals.
    Never,
    /// Something this checker cannot type (yet). Compatible with everything
    /// so one gap never produces a cascade of errors.
    Unknown,
    Array(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Opt(Box<Ty>),
    Tuple(Vec<Ty>),
    Class(usize),
    Enum(usize),
    Func(Box<FuncSig>),
    /// A type used as a value, such as a class name before `{`.
    TypeOf(Box<Ty>),
    Range,
    /// `event(t)`: tasks await it, code signals it with a `t`.
    Event(Box<Ty>),
    /// `listenable(t)`: an event that outside code may await or subscribe to
    /// but not signal. Device events have this type.
    Listenable(Box<Ty>),
    /// What `Subscribe` returns; `Cancel()` removes the handler.
    Cancelable,
    /// `task(t)`: the handle `spawn` returns.
    Task(Box<Ty>),
    /// A type variable bound by a `where` clause.
    Param(String),
}

/// Effects a function may have. The default (no specifier) is `<transacts>`:
/// reads, writes and allocates, but neither fails nor suspends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effects {
    pub decides: bool,
    pub suspends: bool,
    pub reads: bool,
    pub writes: bool,
    pub allocates: bool,
}

impl Effects {
    pub const TRANSACTS: Effects = Effects {
        decides: false,
        suspends: false,
        reads: true,
        writes: true,
        allocates: true,
    };
    pub const COMPUTES: Effects = Effects {
        decides: false,
        suspends: false,
        reads: false,
        writes: false,
        allocates: false,
    };

    pub fn decides(mut self) -> Effects {
        self.decides = true;
        self
    }
    pub fn suspends(mut self) -> Effects {
        self.suspends = true;
        self
    }

    /// Specifier text, for messages and summaries.
    pub fn describe(&self) -> String {
        let mut s = String::new();
        if self.suspends {
            s.push_str("<suspends>");
        }
        if self.decides {
            s.push_str("<decides>");
        }
        match (self.reads, self.writes, self.allocates) {
            (true, true, true) => s.push_str("<transacts>"),
            (false, false, false) => s.push_str("<computes>"),
            (r, w, a) => {
                if r {
                    s.push_str("<reads>");
                }
                if w {
                    s.push_str("<writes>");
                }
                if a {
                    s.push_str("<allocates>");
                }
            }
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamSig {
    pub name: String,
    pub ty: Ty,
    pub named: bool,
    pub has_default: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncSig {
    pub name: String,
    pub params: Vec<ParamSig>,
    pub ret: Ty,
    pub effects: Effects,
}

impl FuncSig {
    pub fn simple(name: &str, params: &[Ty], ret: Ty, effects: Effects) -> FuncSig {
        FuncSig {
            name: name.to_string(),
            params: params
                .iter()
                .enumerate()
                .map(|(i, t)| ParamSig {
                    name: format!("Arg{}", i + 1),
                    ty: t.clone(),
                    named: false,
                    has_default: false,
                })
                .collect(),
            ret,
            effects,
        }
    }
}

/// Levenshtein distance, for "did you mean" suggestions.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1].eq_ignore_ascii_case(&b[j - 1]) {
                0
            } else {
                1
            };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The closest candidate within a small edit distance, if any.
pub fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let limit = (name.len() / 3).max(1);
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        if c == name {
            continue;
        }
        let d = edit_distance(name, c);
        if d <= limit && best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c.to_string())
}
