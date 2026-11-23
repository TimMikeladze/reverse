//! Static checker for Verse: names, types, failure contexts and effects.
//!
//! Rules follow the Book of Verse (https://verselang.github.io/book/). Where
//! the Book is silent the checker stays permissive and types the expression
//! as `Unknown`, so a gap never turns into a false error.

pub mod builtins;
pub mod explain;
pub mod stdlib;
pub mod types;

use reverse_syntax::ast::*;
use reverse_syntax::{Diagnostic, Severity, Span};
use std::collections::{HashMap, HashSet};
use types::*;

pub use explain::{explain, CODES};

#[derive(Clone, Debug)]
pub struct FnSummary {
    pub name: String,
    pub required_params: usize,
    pub decides: bool,
    pub suspends: bool,
    pub signature: String,
}

#[derive(Clone, Debug, Default)]
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub functions: Vec<FnSummary>,
}

impl CheckResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// Parse and check a source file. Diagnostics from both stages are merged
/// and sorted by position.
pub fn analyze(src: &str) -> (Module, CheckResult) {
    let (module, mut diags) = reverse_syntax::parse(src);
    let syntax_ok = !diags.iter().any(|d| d.severity == Severity::Error);
    if syntax_ok && stdlib::wanted(src) {
        return analyze_with_library(src);
    }
    let mut result = check(src, &module);
    diags.append(&mut result.diagnostics);
    result.diagnostics = tidy(diags);
    (module, result)
}

fn tidy(mut diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diags.sort_by_key(|d| (d.span.start, d.code));
    diags.dedup_by(|a, b| a.span == b.span && a.code == b.code && a.message == b.message);
    diags
}

/// Check a file that imports Fortnite modules together with the built-in
/// stand-ins. The stand-ins are appended after the file, so every position
/// in the file itself is unchanged; diagnostics inside them are dropped.
fn analyze_with_library(src: &str) -> (Module, CheckResult) {
    let lib = stdlib::library_for(src);
    let lib_start = src.len() as u32 + 1;
    let full = format!("{}\n{}", src, lib.text);
    let (module, mut diags) = reverse_syntax::parse(&full);
    let mut result = check_at(&full, &module, lib_start);
    diags.append(&mut result.diagnostics);
    diags.retain(|d| d.span.start < lib_start);
    result
        .functions
        .retain(|f| !lib.functions.contains(f.name.as_str()));

    // Names used without their module, and simulation names outside tests.
    let imported: Vec<String> = module
        .items
        .iter()
        .filter_map(|it| match &it.kind {
            ExprKind::Using(p) => Some(p.clone()),
            _ => None,
        })
        .collect();
    let own = stdlib::top_level_names(src);
    // Spans of the user's functions that may use simulation names.
    let test_spans: Vec<Span> = module
        .items
        .iter()
        .filter(|it| it.span.start < lib_start)
        .filter_map(|it| match &it.kind {
            ExprKind::Func(f)
                if f.name.starts_with("Test")
                    || f.name == "Main"
                    || stdlib::is_sim_name(&f.name) =>
            {
                Some(it.span)
            }
            _ => None,
        })
        .collect();
    let (tokens, _) = reverse_syntax::lexer::lex(src);
    let mut reported: HashSet<String> = HashSet::new();
    for t in &tokens {
        let reverse_syntax::lexer::Tok::Ident(name) = &t.tok else {
            continue;
        };
        if own.contains(name.as_str()) {
            continue;
        }
        if stdlib::is_sim_name(name) {
            let in_test = test_spans
                .iter()
                .any(|s| s.start <= t.span.start && t.span.end <= s.end);
            if !in_test {
                let mut d = Diagnostic::warning(
                    "V6007",
                    t.span,
                    format!(
                        "`{}` exists only in this toolchain's simulation, not in UEFN",
                        name
                    ),
                );
                d.help = Some(
                    "use `Sim...` names only inside `Main` or functions named `Test...` or \
                     `Sim...`, and keep those out of the code you paste into UEFN"
                        .into(),
                );
                diags.push(d);
            }
            continue;
        }
        let Some(module_path) = lib.modules.get(name.as_str()) else {
            continue;
        };
        if *module_path == "sim" || imported.iter().any(|p| p == module_path) {
            continue;
        }
        if !reported.insert(name.clone()) {
            continue;
        }
        let mut d = Diagnostic::warning(
            "V6006",
            t.span,
            format!(
                "`{}` comes from `{}`, which this file does not import",
                name, module_path
            ),
        );
        d.help = Some(format!(
            "add `using {{ {} }}` at the top of the file",
            module_path
        ));
        d.fix = Some(reverse_syntax::Fix {
            span: Span::new(0, 0),
            replacement: format!("using {{ {} }}\n", module_path),
        });
        diags.push(d);
    }
    result.diagnostics = tidy(diags);
    (module, result)
}

pub fn check(src: &str, module: &Module) -> CheckResult {
    check_at(src, module, u32::MAX)
}

/// Diagnostics inside the built-in stand-ins themselves; a test keeps this
/// empty so a mistake in the library cannot hide behind the filter.
pub fn library_diagnostics() -> Vec<String> {
    let src = "using { /Fortnite.com/Devices }\n";
    let lib = stdlib::library_for(src);
    let lib_start = src.len() as u32 + 1;
    let full = format!("{}\n{}", src, lib.text);
    let (module, mut diags) = reverse_syntax::parse(&full);
    diags.append(&mut check_at(&full, &module, lib_start).diagnostics);
    let index = reverse_syntax::LineIndex::new(&full);
    diags
        .iter()
        .filter(|d| d.span.start >= lib_start)
        .map(|d| {
            let (line, _) = index.line_col(d.span.start);
            format!(
                "{} {} at library line {}: {}",
                d.severity.as_str(),
                d.code,
                line - 2,
                d.message
            )
        })
        .collect()
}

fn check_at(src: &str, module: &Module, lib_start: u32) -> CheckResult {
    let mut c = Checker::new(src);
    c.lib_start = lib_start;
    c.run(module);
    let mut functions: Vec<FnSummary> = Vec::new();
    for sig in &c.fn_order {
        functions.push(FnSummary {
            name: sig.name.clone(),
            required_params: sig.params.iter().filter(|p| !p.has_default).count(),
            decides: sig.effects.decides,
            suspends: sig.effects.suspends,
            signature: c.show_sig(sig),
        });
    }
    CheckResult {
        diagnostics: c.diags,
        functions,
    }
}

#[derive(Clone)]
struct VarInfo {
    ty: Ty,
    mutable: bool,
}

#[derive(Clone)]
struct FieldInfo {
    name: String,
    ty: Ty,
    mutable: bool,
    has_default: bool,
    access: Access,
    owner: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Public,
    Internal,
    Protected,
    Private,
}

fn access_of(specs: &[Spec]) -> Access {
    for s in specs {
        match s.name.as_str() {
            "public" => return Access::Public,
            "protected" => return Access::Protected,
            "private" => return Access::Private,
            _ => {}
        }
    }
    Access::Internal
}

struct ClassInfo {
    name: String,
    kind: ClassKind,
    parent: Option<usize>,
    interfaces: Vec<usize>,
    fields: Vec<FieldInfo>,
    methods: Vec<FuncSig>,
    /// Names of methods declared without a body.
    bodiless: Vec<String>,
    is_abstract: bool,
    /// Access level of each method, parallel to `methods`.
    method_access: Vec<Access>,
    unique: bool,
    /// A parent comes from an API digest that is not loaded.
    unknown_parent: bool,
}

struct EnumInfo {
    name: String,
    values: Vec<String>,
    open: bool,
}

#[derive(Clone)]
struct FnCtx {
    effects: Effects,
    ret: Ty,
}

const ACCESS_SPECS: &[&str] = &[
    "public",
    "private",
    "protected",
    "internal",
    "override",
    "final",
    "native",
    "native_callable",
    "abstract",
    "constructor",
    "localizes",
    "epic_internal",
    "scoped",
    "concrete",
    "unique",
    "castable",
    "persistable",
    "final_super",
    "final_super_base",
    "open",
    "closed",
    "experimental",
    "deprecated",
    "getter",
    "setter",
    "module_scoped_var_weak_map_key",
];
const EFFECT_SPECS: &[&str] = &[
    "decides",
    "suspends",
    "reads",
    "writes",
    "allocates",
    "transacts",
    "computes",
    "converges",
    "no_rollback",
    "predicts",
    "varies",
];

struct Checker<'s> {
    src: &'s str,
    diags: Vec<Diagnostic>,
    classes: Vec<ClassInfo>,
    class_ids: HashMap<String, usize>,
    enums: Vec<EnumInfo>,
    enum_ids: HashMap<String, usize>,
    funcs: HashMap<String, FuncSig>,
    /// Every free function in source order, overloads included.
    fn_order: Vec<FuncSig>,
    /// Names defined more than once, with every signature.
    overloads: HashMap<String, Vec<FuncSig>>,
    /// Extension methods by name; the receiver is the first parameter.
    ext: HashMap<String, Vec<FuncSig>>,
    /// Signature of each function definition, keyed by where its name starts.
    sig_at: HashMap<u32, FuncSig>,
    globals: HashMap<String, VarInfo>,
    modules: HashSet<String>,
    scopes: Vec<HashMap<String, VarInfo>>,
    cur_fn: Option<FnCtx>,
    cur_class: Option<usize>,
    failure_ctx: bool,
    loop_depth: u32,
    in_spawn: bool,
    failable_seen: u32,
    type_vars: Vec<String>,
    /// The file imports modules whose digests are not loaded.
    external: Option<String>,
    /// Offset where the built-in device stand-ins begin, or `u32::MAX`.
    lib_start: u32,
}

fn flatten<'a>(items: &'a [Expr], out: &mut Vec<&'a Expr>, modules: &mut HashSet<String>) {
    for it in items {
        if let ExprKind::Module { name, items } = &it.kind {
            modules.insert(name.clone());
            flatten(items, out, modules);
        } else {
            out.push(it);
        }
    }
}

impl<'s> Checker<'s> {
    fn new(src: &'s str) -> Checker<'s> {
        Checker {
            src,
            diags: Vec::new(),
            classes: Vec::new(),
            class_ids: HashMap::new(),
            enums: Vec::new(),
            enum_ids: HashMap::new(),
            funcs: HashMap::new(),
            fn_order: Vec::new(),
            overloads: HashMap::new(),
            ext: HashMap::new(),
            sig_at: HashMap::new(),
            globals: HashMap::new(),
            modules: HashSet::new(),
            scopes: Vec::new(),
            cur_fn: None,
            cur_class: None,
            failure_ctx: false,
            loop_depth: 0,
            in_spawn: false,
            failable_seen: 0,
            type_vars: Vec::new(),
            external: None,
            lib_start: u32::MAX,
        }
    }

    fn err(&mut self, code: &'static str, span: Span, msg: impl Into<String>) -> &mut Diagnostic {
        self.diags.push(Diagnostic::error(code, span, msg));
        self.diags.last_mut().unwrap()
    }
    fn warn(&mut self, code: &'static str, span: Span, msg: impl Into<String>) -> &mut Diagnostic {
        self.diags.push(Diagnostic::warning(code, span, msg));
        self.diags.last_mut().unwrap()
    }

    // ------------------------------------------------------------ type helpers

    fn show(&self, t: &Ty) -> String {
        match t {
            Ty::Int => "int".into(),
            Ty::Float => "float".into(),
            Ty::Rational => "rational".into(),
            Ty::Logic => "logic".into(),
            Ty::Char => "char".into(),
            Ty::Str => "string".into(),
            Ty::Void => "void".into(),
            Ty::Any => "any".into(),
            Ty::Never => "never".into(),
            Ty::Unknown => "unknown".into(),
            Ty::Array(t) if **t == Ty::Never => "[]any".into(),
            Ty::Array(t) => format!("[]{}", self.show(t)),
            Ty::Map(k, v) => format!("[{}]{}", self.show(k), self.show(v)),
            Ty::Opt(t) if **t == Ty::Never => "?any".into(),
            Ty::Opt(t) => format!("?{}", self.show(t)),
            Ty::Tuple(ts) => format!(
                "tuple({})",
                ts.iter()
                    .map(|t| self.show(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Ty::Class(i) => self.classes[*i].name.clone(),
            Ty::Enum(i) => self.enums[*i].name.clone(),
            Ty::Func(sig) => self.show_sig(sig),
            Ty::TypeOf(t) => format!("type {}", self.show(t)),
            Ty::Range => "range".into(),
            Ty::Event(t) => format!("event({})", self.show(t)),
            Ty::Listenable(t) => format!("listenable({})", self.show(t)),
            Ty::Cancelable => "cancelable".into(),
            Ty::Task(t) => format!("task({})", self.show(t)),
            Ty::Param(n) => n.clone(),
        }
    }

    fn show_sig(&self, sig: &FuncSig) -> String {
        let params: Vec<String> = sig
            .params
            .iter()
            .map(|p| {
                format!(
                    "{}{} : {}",
                    if p.named { "?" } else { "" },
                    p.name,
                    self.show(&p.ty)
                )
            })
            .collect();
        let eff = if sig.effects == Effects::TRANSACTS {
            String::new()
        } else {
            sig.effects.describe()
        };
        format!(
            "{}({}){} : {}",
            sig.name,
            params.join(", "),
            eff,
            self.show(&sig.ret)
        )
    }

    fn is_subclass(&self, a: usize, b: usize) -> bool {
        let mut stack = vec![a];
        let mut guard = 0;
        while let Some(c) = stack.pop() {
            if c == b {
                return true;
            }
            guard += 1;
            if guard > 256 {
                return false;
            }
            if let Some(p) = self.classes[c].parent {
                stack.push(p);
            }
            stack.extend(self.classes[c].interfaces.iter().copied());
        }
        false
    }

    fn ancestors(&self, cid: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![cid];
        while let Some(c) = stack.pop() {
            if out.contains(&c) || out.len() > 256 {
                continue;
            }
            out.push(c);
            stack.extend(self.classes[c].interfaces.iter().copied());
            if let Some(p) = self.classes[c].parent {
                stack.push(p);
            }
        }
        out
    }

    fn find_field(&self, cid: usize, name: &str) -> Option<FieldInfo> {
        for c in self.ancestors(cid) {
            if let Some(f) = self.classes[c].fields.iter().find(|f| f.name == name) {
                return Some(f.clone());
            }
        }
        None
    }

    fn find_method(&self, cid: usize, name: &str) -> Option<FuncSig> {
        for c in self.ancestors(cid) {
            if let Some(m) = self.classes[c].methods.iter().find(|m| m.name == name) {
                return Some(m.clone());
            }
        }
        None
    }

    fn same_shape(&self, a: &FuncSig, b: &FuncSig) -> bool {
        let x: Vec<&ParamSig> = a.params.iter().filter(|p| !p.named).collect();
        let y: Vec<&ParamSig> = b.params.iter().filter(|p| !p.named).collect();
        x.len() == y.len()
            && x.iter()
                .zip(&y)
                .all(|(p, q)| self.assignable(&p.ty, &q.ty) && self.assignable(&q.ty, &p.ty))
    }

    /// The method `name` of class `cid` that suits these arguments. With one
    /// definition this is plain lookup; with overloads the arguments decide.
    fn pick_method(&mut self, cid: usize, name: &str, args: &[Arg]) -> Option<FuncSig> {
        let mut all: Vec<FuncSig> = Vec::new();
        for c in self.ancestors(cid) {
            for m in self.classes[c].methods.iter().filter(|m| m.name == name) {
                if !all.iter().any(|o| self.same_shape(o, m)) {
                    all.push(m.clone());
                }
            }
        }
        if all.len() <= 1 {
            return all.pop();
        }
        let ats = self.quiet_arg_types(args);
        let n = ats.len();
        let fits = |s: &FuncSig, typed: bool| {
            let pos: Vec<&ParamSig> = s.params.iter().filter(|p| !p.named).collect();
            let required = pos.iter().filter(|p| !p.has_default).count();
            n >= required
                && n <= pos.len()
                && (!typed
                    || ats
                        .iter()
                        .zip(&pos)
                        .all(|(a, p)| self.assignable(a, &p.ty) || matches!(p.ty, Ty::Param(_))))
        };
        let pick = all
            .iter()
            .find(|s| fits(s, true))
            .or_else(|| all.iter().find(|s| fits(s, false)))
            .cloned();
        // No overload fits: report against the first so the user sees a signature.
        pick.or_else(|| all.into_iter().next())
    }

    fn all_fields(&self, cid: usize) -> Vec<FieldInfo> {
        let mut out: Vec<FieldInfo> = Vec::new();
        for c in self.ancestors(cid).into_iter().rev() {
            for f in &self.classes[c].fields {
                if let Some(existing) = out.iter_mut().find(|x| x.name == f.name) {
                    *existing = f.clone();
                } else {
                    out.push(f.clone());
                }
            }
        }
        out
    }

    fn assignable(&self, from: &Ty, to: &Ty) -> bool {
        match (from, to) {
            (Ty::Unknown, _) | (_, Ty::Unknown) | (_, Ty::Any) | (Ty::Never, _) | (_, Ty::Void) => {
                true
            }
            (a, b) if a == b => true,
            (Ty::Int, Ty::Rational) => true,
            (Ty::Str, Ty::Array(t)) | (Ty::Array(t), Ty::Str) => {
                self.assignable(&Ty::Char, t) && self.assignable(t, &Ty::Char)
            }
            (Ty::Array(a), Ty::Array(b)) => self.assignable(a, b),
            (Ty::Opt(a), Ty::Opt(b)) => self.assignable(a, b),
            (Ty::Map(ak, av), Ty::Map(bk, bv)) => {
                self.assignable(ak, bk) && self.assignable(av, bv)
            }
            (Ty::Tuple(a), Ty::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| self.assignable(x, y))
            }
            (Ty::Tuple(a), Ty::Array(b)) => a.iter().all(|x| self.assignable(x, b)),
            (Ty::Class(a), Ty::Class(b)) => self.is_subclass(*a, *b),
            (Ty::Event(a), Ty::Event(b)) | (Ty::Task(a), Ty::Task(b)) => self.assignable(a, b),
            (Ty::Event(a) | Ty::Listenable(a), Ty::Listenable(b)) => self.assignable(a, b),
            (Ty::Func(a), Ty::Func(b)) => {
                a.params.len() == b.params.len()
                    && a.params
                        .iter()
                        .zip(&b.params)
                        .all(|(x, y)| self.assignable(&y.ty, &x.ty))
                    && self.assignable(&a.ret, &b.ret)
                    && (!a.effects.suspends || b.effects.suspends)
                    && (!a.effects.decides || b.effects.decides)
            }
            _ => false,
        }
    }

    fn join(&self, a: &Ty, b: &Ty) -> Ty {
        if *a == Ty::Never {
            return b.clone();
        }
        if *b == Ty::Never {
            return a.clone();
        }
        if *a == Ty::Unknown || *b == Ty::Unknown {
            return Ty::Unknown;
        }
        if self.assignable(a, b) {
            return b.clone();
        }
        if self.assignable(b, a) {
            return a.clone();
        }
        if let (Ty::Class(x), Ty::Class(y)) = (a, b) {
            for anc in self.ancestors(*x) {
                if self.is_subclass(*y, anc) {
                    return Ty::Class(anc);
                }
            }
        }
        Ty::Any
    }

    fn stringable(&self, t: &Ty) -> bool {
        if matches!(
            t,
            Ty::Int | Ty::Float | Ty::Str | Ty::Char | Ty::Rational | Ty::Unknown | Ty::Never
        ) {
            return true;
        }
        if let Ty::Array(inner) = t {
            if **inner == Ty::Char {
                return true;
            }
        }
        // A user-defined `ToString(X : my_type) : string` makes a type printable.
        if let Some(sig) = self.funcs.get("ToString") {
            if let Some(p) = sig.params.first() {
                return self.assignable(t, &p.ty);
            }
        }
        false
    }

    fn resolve_type(&mut self, t: &TypeExpr) -> Ty {
        match t {
            TypeExpr::Array(inner, _) => Ty::Array(Box::new(self.resolve_type(inner))),
            TypeExpr::Map(k, v, _) => Ty::Map(
                Box::new(self.resolve_type(k)),
                Box::new(self.resolve_type(v)),
            ),
            TypeExpr::Option(inner, _) => Ty::Opt(Box::new(self.resolve_type(inner))),
            TypeExpr::Tuple(items, _) => {
                Ty::Tuple(items.iter().map(|i| self.resolve_type(i)).collect())
            }
            TypeExpr::Opaque(_) => Ty::Unknown,
            TypeExpr::Named { name, args, span } => {
                let name: &str = match name.as_str() {
                    "signalable" => "event",
                    "awaitable" | "subscribable" => "listenable",
                    n => n,
                };
                if name == "event" || name == "task" || name == "listenable" {
                    let inner = match args.first() {
                        Some(a) => self.resolve_type(a),
                        None => Ty::Void,
                    };
                    return if name == "event" {
                        Ty::Event(Box::new(inner))
                    } else if name == "listenable" {
                        Ty::Listenable(Box::new(inner))
                    } else {
                        Ty::Task(Box::new(inner))
                    };
                }
                if !args.is_empty() {
                    for a in args {
                        self.resolve_type(a);
                    }
                    return Ty::Unknown;
                }
                match name {
                    "int" => Ty::Int,
                    "float" => Ty::Float,
                    "rational" => Ty::Rational,
                    "logic" => Ty::Logic,
                    "char" | "char32" => Ty::Char,
                    "string" => Ty::Str,
                    "void" => Ty::Void,
                    "message" => Ty::Str,
                    "cancelable" => Ty::Cancelable,
                    "any" | "comparable" => Ty::Any,
                    "type" => Ty::Unknown,
                    n => {
                        if let Some(id) = self.class_ids.get(n) {
                            return Ty::Class(*id);
                        }
                        if let Some(id) = self.enum_ids.get(n) {
                            return Ty::Enum(*id);
                        }
                        if self.type_vars.iter().any(|v| v == n) {
                            return Ty::Param(n.to_string());
                        }
                        self.unknown_type(n, *span);
                        Ty::Unknown
                    }
                }
            }
        }
    }

    fn unknown_type(&mut self, n: &str, span: Span) {
        let alias = match n {
            "bool" | "boolean" | "Bool" | "Boolean" => Some("logic"),
            "String" | "str" | "FString" => Some("string"),
            "Int" | "integer" | "Integer" | "int32" | "int64" | "long" | "i32" | "i64" => {
                Some("int")
            }
            "Float" | "double" | "Double" | "number" | "Number" | "f32" | "f64" => Some("float"),
            "Void" | "None" | "unit" => Some("void"),
            "Any" | "object" | "Object" => Some("any"),
            _ => None,
        };
        if let Some(a) = alias {
            let d = self.err("V1002", span, format!("unknown type `{}`", n));
            d.help = Some(format!("the Verse type is `{}`", a));
            d.fix = Some(reverse_syntax::Fix {
                span,
                replacement: a.to_string(),
            });
            return;
        }
        if let Some(ext) = self.external.clone() {
            self.warn(
                "V6002",
                span,
                format!("type `{}` is not defined in this file", n),
            )
            .help = Some(format!(
                "it may come from `{}`; API digests are not loaded, so uses of it are not checked",
                ext
            ));
            return;
        }
        let candidates: Vec<String> = self
            .class_ids
            .keys()
            .chain(self.enum_ids.keys())
            .cloned()
            .collect();
        let builtin = [
            "int", "float", "string", "logic", "void", "char", "any", "rational",
        ];
        let suggestion = closest(
            n,
            candidates
                .iter()
                .map(|s| s.as_str())
                .chain(builtin.iter().copied()),
        );
        let d = self.err("V1002", span, format!("unknown type `{}`", n));
        if let Some(s) = suggestion {
            d.help = Some(format!("did you mean `{}`?", s));
            d.fix = Some(reverse_syntax::Fix {
                span,
                replacement: s,
            });
        }
    }

    fn effects_of(&mut self, f: &FuncDef) -> Effects {
        for s in &f.pre_specs {
            if EFFECT_SPECS.contains(&s.name.as_str()) {
                self.err(
                    "V4005",
                    s.span,
                    format!("effect specifier `<{}>` is in the wrong place", s.name),
                )
                .help = Some(format!(
                    "effects go after the parameter list: `{}(...)<{}> : type = ...`",
                    f.name, s.name
                ));
            } else if !ACCESS_SPECS.contains(&s.name.as_str()) {
                let sug = closest(
                    &s.name,
                    ACCESS_SPECS.iter().chain(EFFECT_SPECS.iter()).copied(),
                );
                let d = self.err("V4004", s.span, format!("unknown specifier `<{}>`", s.name));
                if let Some(sug) = sug {
                    d.help = Some(format!("did you mean `<{}>`?", sug));
                }
            }
        }
        // A misplaced or misspelled effect still counts, so one mistake
        // does not cascade into errors about the body.
        let misplaced: Vec<Spec> = f
            .pre_specs
            .iter()
            .filter(|s| EFFECT_SPECS.contains(&s.name.as_str()))
            .cloned()
            .collect();
        let mut seen: Vec<&str> = Vec::new();
        let mut exclusive: Option<&str> = None;
        let (mut decides, mut suspends) = (false, false);
        let (mut r, mut w, mut a) = (false, false, false);
        let mut additive = false;
        for s in f.effects.iter().chain(misplaced.iter()) {
            let mut n = s.name.as_str();
            let corrected = if EFFECT_SPECS.contains(&n) || ACCESS_SPECS.contains(&n) {
                None
            } else {
                closest(n, EFFECT_SPECS.iter().copied())
            };
            if let Some(c) = &corrected {
                let d = self.err(
                    "V4004",
                    s.span,
                    format!("unknown effect specifier `<{}>`", n),
                );
                d.help = Some(format!("did you mean `<{}>`?", c));
                d.fix = Some(reverse_syntax::Fix {
                    span: s.span,
                    replacement: format!("<{}>", c),
                });
                n = EFFECT_SPECS
                    .iter()
                    .copied()
                    .find(|e| *e == c.as_str())
                    .unwrap_or(n);
            }
            if seen.contains(&n) {
                self.err(
                    "V4003",
                    s.span,
                    format!("specifier `<{}>` is written twice", n),
                );
                continue;
            }
            seen.push(n);
            match n {
                "decides" => decides = true,
                "suspends" => suspends = true,
                "reads" => {
                    r = true;
                    additive = true;
                }
                "writes" => {
                    w = true;
                    additive = true;
                }
                "allocates" => {
                    a = true;
                    additive = true;
                }
                "transacts" | "computes" | "converges" => {
                    if let Some(prev) = exclusive {
                        self.err(
                            "V4003",
                            s.span,
                            format!("`<{}>` cannot be combined with `<{}>`", n, prev),
                        )
                        .help = Some(
                            "use at most one of `<transacts>`, `<computes>` and `<converges>`"
                                .into(),
                        );
                    }
                    exclusive = Some(n);
                }
                "no_rollback" | "predicts" | "varies" => {}
                _ => {
                    if ACCESS_SPECS.contains(&n) {
                        self.err(
                            "V4005",
                            s.span,
                            format!("specifier `<{}>` is in the wrong place", n),
                        )
                        .help = Some(format!(
                            "write it before the parameter list: `{}<{}>(...)`",
                            f.name, n
                        ));
                    } else {
                        let sug = closest(n, EFFECT_SPECS.iter().copied());
                        let d = self.err(
                            "V4004",
                            s.span,
                            format!("unknown effect specifier `<{}>`", n),
                        );
                        d.help = Some(match sug {
                            Some(s) => format!("did you mean `<{}>`?", s),
                            None => "effects are `<decides>`, `<suspends>`, `<transacts>`, `<computes>`, `<reads>`, `<writes>` and `<allocates>`".into(),
                        });
                    }
                }
            }
        }
        if decides && suspends {
            let sp = f
                .effects
                .iter()
                .find(|s| s.name == "suspends")
                .map(|s| s.span)
                .unwrap_or(f.name_span);
            self.err(
                "V4003",
                sp,
                "a function cannot be both `<decides>` and `<suspends>`",
            )
            .help =
                Some("keep `<suspends>` and return an option (`?type`) instead of failing".into());
        }
        let heap = match exclusive {
            Some("computes") | Some("converges") => (r, w, a),
            Some(_) => (true, true, true),
            None if additive => (r, w, a),
            None => (true, true, true),
        };
        Effects {
            decides,
            suspends,
            reads: heap.0,
            writes: heap.1,
            allocates: heap.2,
        }
    }

    fn sig_of(&mut self, f: &FuncDef) -> FuncSig {
        let saved = std::mem::replace(&mut self.type_vars, f.type_vars.clone());
        let mut params = Vec::new();
        let mut seen_named = false;
        for p in &f.params {
            if p.named {
                seen_named = true;
            } else if seen_named {
                self.err(
                    "V2002",
                    p.span,
                    format!(
                        "positional parameter `{}` comes after a named parameter",
                        p.name
                    ),
                )
                .help = Some("put all `?Named` parameters last".into());
            }
            let ty = self.resolve_type(&p.ty);
            params.push(ParamSig {
                name: p.name.clone(),
                ty,
                named: p.named,
                has_default: p.default.is_some(),
            });
        }
        let ret = match &f.ret {
            Some(t) => self.resolve_type(t),
            None => Ty::Unknown,
        };
        let effects = self.effects_of(f);
        self.type_vars = saved;
        FuncSig {
            name: f.name.clone(),
            params,
            ret,
            effects,
        }
    }

    // ------------------------------------------------------------ driver

    fn run(&mut self, module: &Module) {
        let mut items: Vec<&Expr> = Vec::new();
        let mut modules = HashSet::new();
        flatten(&module.items, &mut items, &mut modules);
        self.modules = modules;

        for it in &items {
            if let ExprKind::Using(path) = &it.kind {
                if !path.starts_with("/Verse.org") && self.external.is_none() {
                    self.external = Some(path.clone());
                }
            }
        }

        // 1. type names
        for it in &items {
            match &it.kind {
                ExprKind::Class(c) => {
                    if self.class_ids.contains_key(&c.name) || self.enum_ids.contains_key(&c.name) {
                        self.err(
                            "V1004",
                            c.name_span,
                            format!("`{}` is defined more than once", c.name),
                        );
                        continue;
                    }
                    self.class_ids.insert(c.name.clone(), self.classes.len());
                    self.classes.push(ClassInfo {
                        name: c.name.clone(),
                        kind: c.kind,
                        parent: None,
                        interfaces: Vec::new(),
                        fields: Vec::new(),
                        methods: Vec::new(),
                        bodiless: Vec::new(),
                        method_access: Vec::new(),
                        unique: c.specs.iter().any(|s| s.name == "unique"),
                        unknown_parent: false,
                        is_abstract: c.kind == ClassKind::Interface
                            || c.specs.iter().any(|s| s.name == "abstract"),
                    });
                }
                ExprKind::Enum(e) => {
                    if self.class_ids.contains_key(&e.name) || self.enum_ids.contains_key(&e.name) {
                        self.err(
                            "V1004",
                            e.name_span,
                            format!("`{}` is defined more than once", e.name),
                        );
                        continue;
                    }
                    let mut values: Vec<String> = Vec::new();
                    for (v, sp) in &e.values {
                        if values.contains(v) {
                            self.err("V1004", *sp, format!("enum value `{}` is listed twice", v));
                        } else {
                            values.push(v.clone());
                        }
                    }
                    self.enum_ids.insert(e.name.clone(), self.enums.len());
                    self.enums.push(EnumInfo {
                        name: e.name.clone(),
                        values,
                        open: e.specs.iter().any(|s| s.name == "open"),
                    });
                }
                _ => {}
            }
        }

        // 2. class headers and member signatures
        for it in &items {
            let ExprKind::Class(c) = &it.kind else {
                continue;
            };
            let Some(&cid) = self.class_ids.get(&c.name) else {
                continue;
            };
            for (pname, psp) in &c.parents {
                match self.class_ids.get(pname).copied() {
                    Some(pid) if pid == cid => {
                        self.err(
                            "V1005",
                            *psp,
                            format!("`{}` cannot inherit from itself", c.name),
                        );
                    }
                    Some(pid) => {
                        let pkind = self.classes[pid].kind;
                        if pkind == ClassKind::Interface {
                            self.classes[cid].interfaces.push(pid);
                        } else if c.kind == ClassKind::Struct || pkind == ClassKind::Struct {
                            self.err("V1005", *psp, "structs do not support inheritance");
                        } else if self.classes[cid].parent.is_some() {
                            self.err(
                                "V1005",
                                *psp,
                                format!(
                                    "`{}` already has a parent class; only one is allowed",
                                    c.name
                                ),
                            )
                            .help = Some(
                                "a class can inherit one class and any number of interfaces".into(),
                            );
                        } else {
                            self.classes[cid].parent = Some(pid);
                        }
                    }
                    None => {
                        self.classes[cid].unknown_parent = true;
                        if let Some(ext) = self.external.clone() {
                            self.warn("V6002", *psp, format!("parent type `{}` is not defined in this file", pname)).help =
                                Some(format!("it may come from `{}`; API digests are not loaded, so inherited members are not checked", ext));
                        } else {
                            let cands: Vec<String> = self.class_ids.keys().cloned().collect();
                            let sug = closest(pname, cands.iter().map(|s| s.as_str()));
                            let d =
                                self.err("V1002", *psp, format!("unknown parent type `{}`", pname));
                            if let Some(s) = sug {
                                d.help = Some(format!("did you mean `{}`?", s));
                            }
                        }
                    }
                }
            }
            let mut names: HashSet<String> = HashSet::new();
            for m in &c.members {
                match &m.kind {
                    ExprKind::Def(d) => {
                        if !names.insert(d.name.clone()) {
                            self.err(
                                "V1004",
                                d.name_span,
                                format!("member `{}` is defined more than once", d.name),
                            );
                            continue;
                        }
                        let ty = match &d.ty {
                            Some(t) => self.resolve_type(t),
                            None => Ty::Unknown,
                        };
                        if d.is_var && c.kind == ClassKind::Struct {
                            self.err("V5004", d.name_span, "struct fields cannot be `var`").help =
                                Some("declare the struct variable itself with `var` and update it with `set Value.Field = ...`".into());
                        }
                        self.classes[cid].fields.push(FieldInfo {
                            name: d.name.clone(),
                            ty,
                            mutable: d.is_var,
                            has_default: d.value.is_some(),
                            access: access_of(&d.specs),
                            owner: cid,
                        });
                    }
                    ExprKind::Func(f) => {
                        let repeated = !names.insert(f.name.clone());
                        if c.kind == ClassKind::Struct {
                            self.err("V1005", f.name_span, "structs cannot have methods")
                                .help = Some(
                                "use a `class`, or write a free function that takes the struct"
                                    .into(),
                            );
                        }
                        let sig = self.sig_of(f);
                        self.sig_at.insert(f.name_span.start, sig.clone());
                        if repeated {
                            let is_field =
                                self.classes[cid].fields.iter().any(|x| x.name == f.name);
                            let clash = is_field
                                || self.classes[cid]
                                    .methods
                                    .iter()
                                    .any(|o| o.name == f.name && self.same_shape(o, &sig));
                            if clash {
                                self.err(
                                    "V1004",
                                    f.name_span,
                                    format!("member `{}` is defined more than once", f.name),
                                )
                                .help = Some(
                                    "overloads must differ in the number or the types of their \
                                     parameters"
                                        .into(),
                                );
                                continue;
                            }
                        }
                        self.classes[cid].methods.push(sig);
                        self.classes[cid]
                            .method_access
                            .push(access_of(&f.pre_specs));
                        if f.body.is_none() {
                            self.classes[cid].bodiless.push(f.name.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        // 3. free functions, overloads and extension methods
        for it in &items {
            if let ExprKind::Func(f) = &it.kind {
                if self.class_ids.contains_key(&f.name) || self.enum_ids.contains_key(&f.name) {
                    self.err(
                        "V1004",
                        f.name_span,
                        format!("`{}` is already the name of a type", f.name),
                    );
                    continue;
                }
                let sig = self.sig_of(f);
                let table = if f.extension {
                    &self.ext
                } else {
                    &self.overloads
                };
                let mut existing: Vec<FuncSig> = table.get(&f.name).cloned().unwrap_or_default();
                if !f.extension && existing.is_empty() {
                    existing.extend(self.funcs.get(&f.name).cloned());
                }
                let clash = existing.iter().any(|o| {
                    let a: Vec<&ParamSig> = o.params.iter().filter(|p| !p.named).collect();
                    let b: Vec<&ParamSig> = sig.params.iter().filter(|p| !p.named).collect();
                    a.len() == b.len()
                        && a.iter().zip(&b).all(|(x, y)| {
                            self.assignable(&x.ty, &y.ty) && self.assignable(&y.ty, &x.ty)
                        })
                });
                if clash {
                    self.err(
                        "V1004",
                        f.name_span,
                        format!(
                            "`{}` is already defined with the same parameter types",
                            f.name
                        ),
                    )
                    .help = Some(
                        "overloads must differ in the number or the types of their parameters"
                            .into(),
                    );
                    continue;
                }
                self.sig_at.insert(f.name_span.start, sig.clone());
                if f.extension {
                    self.ext.entry(f.name.clone()).or_default().push(sig);
                    continue;
                }
                if !existing.is_empty() {
                    existing.push(sig.clone());
                    self.overloads.insert(f.name.clone(), existing);
                } else {
                    self.funcs.insert(f.name.clone(), sig.clone());
                }
                self.fn_order.push(sig);
            }
        }

        // 4. module-level values, in order
        for it in &items {
            match &it.kind {
                ExprKind::Def(d) => {
                    self.scopes.clear();
                    self.cur_fn = None;
                    self.failure_ctx = false;
                    let ann = d.ty.as_ref().map(|t| self.resolve_type(t));
                    let vt = match &d.value {
                        Some(v) => {
                            let t = self.expr(v, ann.as_ref());
                            if let Some(a) = &ann {
                                self.require(v, &t, a, "initial value");
                            }
                            t
                        }
                        None => {
                            self.err(
                                "V2012",
                                d.name_span,
                                format!("`{}` needs an initial value", d.name),
                            );
                            Ty::Unknown
                        }
                    };
                    if d.is_var {
                        self.warn("V6004", d.name_span, format!("module-level `var {}` is accepted here for testing", d.name)).help =
                            Some("UEFN only allows module-scoped variables of `weak_map` type; put mutable state in a class field for portable code".into());
                    }
                    if self.globals.contains_key(&d.name) || self.funcs.contains_key(&d.name) {
                        self.err(
                            "V1004",
                            d.name_span,
                            format!("`{}` is defined more than once", d.name),
                        );
                    }
                    self.globals.insert(
                        d.name.clone(),
                        VarInfo {
                            ty: ann.unwrap_or(vt),
                            mutable: d.is_var,
                        },
                    );
                }
                ExprKind::Class(_)
                | ExprKind::Enum(_)
                | ExprKind::Func(_)
                | ExprKind::Using(_)
                | ExprKind::Error => {}
                _ => {
                    self.err("V9001", it.span, "only definitions are allowed at module level").help =
                        Some("put statements inside a function, for example `Main() : void =` followed by an indented body".into());
                }
            }
        }

        // 5. bodies
        for it in &items {
            match &it.kind {
                ExprKind::Func(f) => {
                    if let Some(sig) = self.sig_at.get(&f.name_span.start).cloned() {
                        self.func_body(f, &sig, None);
                    }
                }
                ExprKind::Class(c) => self.class_body(c),
                _ => {}
            }
        }
    }

    fn class_body(&mut self, c: &ClassDef) {
        let Some(&cid) = self.class_ids.get(&c.name) else {
            return;
        };
        let parents: Vec<usize> = self
            .ancestors(cid)
            .into_iter()
            .filter(|a| *a != cid)
            .collect();
        for m in &c.members {
            match &m.kind {
                ExprKind::Def(d) => {
                    for p in &parents {
                        if self.classes[*p].fields.iter().any(|f| f.name == d.name)
                            && !d.specs.iter().any(|s| s.name == "override")
                        {
                            let pn = self.classes[*p].name.clone();
                            self.err("V7001", d.name_span, format!("field `{}` is already declared in `{}`", d.name, pn)).help =
                                Some("a subclass cannot redeclare a parent's field; give it a different name, or set it in the archetype".into());
                            break;
                        }
                    }
                    if let Some(v) = &d.value {
                        self.scopes.clear();
                        self.cur_fn = None;
                        self.cur_class = Some(cid);
                        self.failure_ctx = false;
                        let ann = d.ty.as_ref().map(|t| self.resolve_type(t));
                        let t = self.expr(v, ann.as_ref());
                        if let Some(a) = &ann {
                            self.require(v, &t, a, "default value");
                        }
                        self.cur_class = None;
                    }
                }
                ExprKind::Func(f) => {
                    let own = self.sig_at.get(&f.name_span.start).cloned();
                    // With overloads, the parent method of the same shape is the
                    // one overridden; otherwise any method of that name.
                    let candidates: Vec<FuncSig> = parents
                        .iter()
                        .flat_map(|p| self.classes[*p].methods.iter())
                        .filter(|m| m.name == f.name)
                        .cloned()
                        .collect();
                    let inherited = candidates
                        .iter()
                        .find(|m| own.as_ref().is_some_and(|o| self.same_shape(o, m)))
                        .or(candidates.first())
                        .cloned();
                    let has_override = f.pre_specs.iter().any(|s| s.name == "override");
                    match (&inherited, has_override) {
                        (Some(_), false) => {
                            let d = self.err("V7001", f.name_span, format!("`{}` overrides an inherited method but is not marked `<override>`", f.name));
                            d.help = Some(format!("write `{}<override>(...)`", f.name));
                            d.fix = Some(reverse_syntax::Fix {
                                span: Span::new(f.name_span.end, f.name_span.end),
                                replacement: "<override>".into(),
                            });
                        }
                        (None, true)
                            if self.external.is_none()
                                || c.parents
                                    .iter()
                                    .all(|(p, _)| self.class_ids.contains_key(p)) =>
                        {
                            self.err("V7002", f.name_span, format!("`{}` is marked `<override>` but no parent declares it", f.name)).help =
                                Some("remove `<override>`, or check the method name against the parent class".into());
                        }
                        (Some(parent_sig), true) => {
                            if let Some(mine) = own.clone() {
                                if mine.params.len() != parent_sig.params.len() {
                                    self.err("V7003", f.name_span, format!(
                                        "`{}` takes {} parameter(s) but the method it overrides takes {}",
                                        f.name, mine.params.len(), parent_sig.params.len()
                                    ));
                                }
                                if (mine.effects.decides && !parent_sig.effects.decides)
                                    || (mine.effects.suspends && !parent_sig.effects.suspends)
                                {
                                    self.err("V7003", f.name_span, format!("`{}` adds an effect the overridden method does not have", f.name)).help =
                                        Some(format!("the parent declares `{}`; an override may have fewer effects but not more", self.show_sig(parent_sig)));
                                }
                            }
                        }
                        _ => {}
                    }
                    if let Some(sig) = own {
                        if f.body.is_none()
                            && !self.classes[cid].is_abstract
                            && !f
                                .pre_specs
                                .iter()
                                .any(|s| s.name == "native" || s.name == "abstract")
                        {
                            self.err("V2012", f.name_span, format!("method `{}` has no body", f.name)).help =
                                Some("add `= Body`, or mark the class `<abstract>` if subclasses must implement it".into());
                        }
                        self.func_body(f, &sig, Some(cid));
                    }
                }
                ExprKind::Block(_) => {
                    self.scopes.clear();
                    self.scopes.push(HashMap::new());
                    self.cur_fn = Some(FnCtx {
                        effects: Effects::TRANSACTS,
                        ret: Ty::Void,
                    });
                    self.cur_class = Some(cid);
                    self.failure_ctx = false;
                    self.expr(m, None);
                    self.cur_class = None;
                    self.cur_fn = None;
                    self.scopes.clear();
                }
                ExprKind::Error => {}
                _ => {
                    self.err(
                        "V9001",
                        m.span,
                        format!(
                            "only fields, methods and `block` clauses are allowed in a {} body",
                            c.kind.as_str()
                        ),
                    );
                }
            }
        }
        // A concrete class must implement every abstract method it inherits.
        if !self.classes[cid].is_abstract && c.kind == ClassKind::Class {
            let mut missing: Vec<String> = Vec::new();
            for it in self.unimplemented(cid) {
                missing.push(it);
            }
            if !missing.is_empty() {
                self.err(
                    "V7004",
                    c.name_span,
                    format!(
                        "class `{}` does not implement {}",
                        c.name,
                        missing
                            .iter()
                            .map(|m| format!("`{}`", m))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
                .help = Some(
                    "add each method with `<override>`, or mark the class `<abstract>`".into(),
                );
            }
        }
    }

    /// Inherited methods that no class in the hierarchy gives a body.
    fn unimplemented(&self, cid: usize) -> Vec<String> {
        let chain = self.ancestors(cid);
        let mut out: Vec<String> = Vec::new();
        for a in &chain {
            for name in &self.classes[*a].bodiless {
                let implemented = chain.iter().any(|c| {
                    self.classes[*c].methods.iter().any(|m| &m.name == name)
                        && !self.classes[*c].bodiless.contains(name)
                });
                if !implemented && !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
        out
    }

    fn func_body(&mut self, f: &FuncDef, sig: &FuncSig, class: Option<usize>) {
        let Some(body) = &f.body else {
            if class.is_none() && !f.pre_specs.iter().any(|s| s.name == "native") {
                self.err(
                    "V2012",
                    f.name_span,
                    format!("function `{}` has no body", f.name),
                )
                .help = Some(format!(
                    "add `= Body` after the return type, for example `{}() : void = Print(\"hi\")`",
                    f.name
                ));
            }
            return;
        };
        if f.ret.is_none() {
            self.err(
                "V2013",
                f.name_span,
                format!("function `{}` has no return type", f.name),
            )
            .help = Some(format!(
                "add one after the parameter list, for example `{}(...) : void = ...`",
                f.name
            ));
        }
        self.scopes.clear();
        self.scopes.push(HashMap::new());
        self.type_vars = f.type_vars.clone();
        for (p, ps) in f.params.iter().zip(&sig.params) {
            if let Some(d) = &p.default {
                self.cur_fn = None;
                let t = self.expr(d, Some(&ps.ty));
                self.require(d, &t, &ps.ty, "default value");
            }
            self.declare(&p.name, p.span, ps.ty.clone(), false);
        }
        self.cur_fn = Some(FnCtx {
            effects: sig.effects,
            ret: sig.ret.clone(),
        });
        self.cur_class = class;
        self.failure_ctx = sig.effects.decides;
        self.loop_depth = 0;
        self.in_spawn = false;
        self.failable_seen = 0;
        let t = self.expr(body, Some(&sig.ret));
        if sig.ret != Ty::Void
            && sig.ret != Ty::Unknown
            && t != Ty::Never
            && !self.assignable(&t, &sig.ret)
        {
            let last = last_expr(body);
            let d_help = self.conversion_help(last, &t, &sig.ret);
            let from = self.show(&t);
            let to = self.show(&sig.ret);
            let d = self.err(
                "V2006",
                last.span,
                format!(
                    "`{}` must return `{}`, but its last expression has type `{}`",
                    f.name, to, from
                ),
            );
            d.help = d_help.0.or(Some(
                "a function returns the value of its last expression".into(),
            ));
            d.fix = d_help.1;
        }
        self.cur_fn = None;
        self.cur_class = None;
        self.failure_ctx = false;
        self.type_vars.clear();
        self.scopes.clear();
    }

    // ------------------------------------------------------------ scopes

    fn lookup_var(&self, name: &str) -> Option<VarInfo> {
        for s in self.scopes.iter().rev() {
            if let Some(v) = s.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    fn declare(&mut self, name: &str, span: Span, ty: Ty, mutable: bool) {
        if name == "_" {
            return;
        }
        if self.lookup_var(name).is_some() {
            self.err("V1004", span, format!("`{}` is already defined in this function", name)).help =
                Some("Verse does not allow a name to be redefined or shadowed; pick a new name, or use `set` to change a `var`".into());
            return;
        }
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), VarInfo { ty, mutable });
        }
    }

    fn failable(&mut self, span: Span, what: &str) {
        self.failable_seen += 1;
        if !self.failure_ctx {
            self.err("V3001", span, format!("{} can fail, but this is not a failure context", what)).help = Some(
                "handle the failure: `if (Value := <expr>) { ... }`, add a fallback with `<expr> or Default`, wrap it in `option{...}`, or mark the enclosing function `<decides>`".into(),
            );
        }
    }

    /// Check `e` inside a failure context whose failure is handled locally
    /// (the left side of `or`, `option{}`, `logic{}`). Returns the type and
    /// whether anything inside could fail.
    fn in_hidden_failure(&mut self, e: &Expr, expect: Option<&Ty>) -> (Ty, bool) {
        let before = self.failable_seen;
        let saved = std::mem::replace(&mut self.failure_ctx, true);
        let t = self.expr(e, expect);
        self.failure_ctx = saved;
        let failed = self.failable_seen != before;
        self.failable_seen = before;
        (t, failed)
    }

    fn need_effect(&mut self, span: Span, what: &str, reads: bool, writes: bool, allocates: bool) {
        let Some(ctx) = &self.cur_fn else { return };
        let e = ctx.effects;
        let mut missing = Vec::new();
        if reads && !e.reads {
            missing.push("<reads>");
        }
        if writes && !e.writes {
            missing.push("<writes>");
        }
        if allocates && !e.allocates {
            missing.push("<allocates>");
        }
        if missing.is_empty() {
            return;
        }
        let declared = e.describe();
        self.err("V4002", span, format!("{} needs the {} effect, but the enclosing function is declared `{}`", what, missing.join(" and "), declared)).help =
            Some("declare the function `<transacts>` (the default when no effect is written), or remove the mutable-state access".into());
    }

    // ------------------------------------------------------------ mismatch reporting

    fn conversion_help(
        &self,
        e: &Expr,
        from: &Ty,
        to: &Ty,
    ) -> (Option<String>, Option<reverse_syntax::Fix>) {
        let text = self
            .src
            .get(e.span.start as usize..e.span.end as usize)
            .unwrap_or("");
        match (from, to) {
            (Ty::Int, Ty::Float) => {
                if let ExprKind::Int(v) = &e.kind {
                    return (
                        Some(format!("float literals need a decimal point: write `{}.0`", v)),
                        Some(reverse_syntax::Fix { span: e.span, replacement: format!("{}.0", v) }),
                    );
                }
                (Some(format!("convert an int to a float by multiplying: `{} * 1.0`", text)), None)
            }
            (Ty::Float, Ty::Int) => (
                Some("convert a float to an int with `Floor[X]`, `Ceil[X]`, `Round[X]` or `Int[X]`; these can fail, so call them in a failure context".into()),
                None,
            ),
            (Ty::Rational, Ty::Int) => (
                Some(format!("`int / int` produces a `rational`; convert it with `Floor({})` or `Ceil({})`", text, text)),
                None,
            ),
            (Ty::Opt(inner), other) if self.assignable(inner, other) => (
                Some(format!("this is an option; unwrap it with `?` in a failure context, for example `if (Value := {}?)`, or give a fallback: `{}? or Default`", text, text)),
                None,
            ),
            (other, Ty::Opt(inner)) if self.assignable(other, inner) => (
                Some(format!("wrap the value: `option{{{}}}`", text)),
                Some(reverse_syntax::Fix { span: e.span, replacement: format!("option{{{}}}", text) }),
            ),
            (f, Ty::Str) if self.stringable(f) => (
                Some(format!("turn it into a string with interpolation: `\"{{{}}}\"`", text)),
                Some(reverse_syntax::Fix { span: e.span, replacement: format!("\"{{{}}}\"", text) }),
            ),
            (Ty::Logic, Ty::Str) => (
                Some("`logic` has no `ToString`; write `if (Flag?) then \"true\" else \"false\"`".into()),
                None,
            ),
            (Ty::Void, _) => (Some("this expression does not produce a value".into()), None),
            _ => (None, None),
        }
    }

    fn require(&mut self, e: &Expr, from: &Ty, to: &Ty, what: &str) {
        if self.assignable(from, to) {
            return;
        }
        let (help, fix) = self.conversion_help(e, from, to);
        let msg = format!(
            "{}: expected `{}`, found `{}`",
            what,
            self.show(to),
            self.show(from)
        );
        let d = self.err("V2001", e.span, msg);
        d.help = help;
        d.fix = fix;
    }

    // ------------------------------------------------------------ expressions

    fn block(&mut self, stmts: &[Expr], expect: Option<&Ty>) -> Ty {
        self.scopes.push(HashMap::new());
        let mut last = Ty::Void;
        for (i, s) in stmts.iter().enumerate() {
            let is_last = i + 1 == stmts.len();
            if !is_last && !self.failure_ctx {
                // `X = 5` as a statement is a comparison, not an assignment.
                if let ExprKind::Binary(BinOp::Eq, lhs, _) = &s.kind {
                    if matches!(lhs.kind, ExprKind::Ident(_) | ExprKind::Member(..)) {
                        let lt = self
                            .src
                            .get(lhs.span.start as usize..lhs.span.end as usize)
                            .unwrap_or("X")
                            .to_string();
                        let d = self.err(
                            "V5002",
                            s.span,
                            "`=` compares two values; it does not assign",
                        );
                        d.help = Some(format!("to change a variable write `set {} = ...`; to define a new name write `{} := ...`", lt, lt));
                        d.fix = Some(reverse_syntax::Fix {
                            span: Span::new(s.span.start, s.span.start),
                            replacement: "set ".into(),
                        });
                        let saved = std::mem::replace(&mut self.failure_ctx, true);
                        self.expr(s, None);
                        self.failure_ctx = saved;
                        continue;
                    }
                }
            }
            last = self.expr(s, if is_last { expect } else { None });
            if last == Ty::Never && !is_last {
                let next = &stmts[i + 1];
                self.warn("V9002", next.span, "this code is never reached");
            }
        }
        self.scopes.pop();
        last
    }

    fn expr(&mut self, e: &Expr, expect: Option<&Ty>) -> Ty {
        match &e.kind {
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Float(_) => Ty::Float,
            ExprKind::Char(_) => Ty::Char,
            ExprKind::Logic(v) => {
                if !*v {
                    if let Some(t @ Ty::Opt(_)) = expect {
                        return t.clone();
                    }
                }
                Ty::Logic
            }
            ExprKind::Unit => Ty::Void,
            ExprKind::Str(segs) => {
                for seg in segs {
                    if let StrSeg::Expr(inner) = seg {
                        let t = self.expr(inner, None);
                        if !self.stringable(&t) {
                            let shown = self.show(&t);
                            let help = match &t {
                                Ty::Logic => "`logic` has no `ToString`; write `if (Flag?) then \"true\" else \"false\"`".to_string(),
                                Ty::Opt(_) => "unwrap the option first, for example `if (Value := Opt?)`".to_string(),
                                Ty::Array(_) => "interpolate the elements one at a time in a `for` loop, or build the text with `Join`".to_string(),
                                Ty::Void => "this expression does not produce a value".to_string(),
                                _ => format!("define `ToString(Value : {}) : string` for this type, or interpolate its fields", shown),
                            };
                            self.err(
                                "V2007",
                                inner.span,
                                format!(
                                    "a value of type `{}` cannot be interpolated into a string",
                                    shown
                                ),
                            )
                            .help = Some(help);
                        }
                    }
                }
                Ty::Str
            }
            ExprKind::Path(_) => Ty::Unknown,
            ExprKind::Ident(name) => self.ident(name, e.span),
            ExprKind::Tuple(items) => {
                let expected: Option<&Vec<Ty>> = match expect {
                    Some(Ty::Tuple(ts)) if ts.len() == items.len() => Some(ts),
                    _ => None,
                };
                let mut ts = Vec::new();
                for (i, it) in items.iter().enumerate() {
                    ts.push(self.expr(it, expected.map(|x| &x[i])));
                }
                Ty::Tuple(ts)
            }
            ExprKind::Block(stmts) => self.block(stmts, expect),
            ExprKind::Def(d) => self.def(d),
            ExprKind::Set { target, op, value } => self.set(e, target, *op, value),
            ExprKind::Unary(op, inner) => self.unary(e, *op, inner),
            ExprKind::Binary(op, l, r) => self.binary(e, *op, l, r, expect),
            ExprKind::Range(lo, hi) => {
                for side in [lo, hi] {
                    let t = self.expr(side, Some(&Ty::Int));
                    self.require(side, &t, &Ty::Int, "range bound");
                }
                self.err("V2009", e.span, "a range `A..B` can only be iterated by `for`").help =
                    Some("write `for (I := A..B):`; to keep the numbers, collect them: `Numbers := for (I := A..B) { I }`".into());
                Ty::Unknown
            }
            ExprKind::Query(inner) => {
                let t = self.expr(inner, None);
                self.failable(e.span, "the query `?`");
                match t {
                    Ty::Opt(inner_t) => *inner_t,
                    Ty::Logic => Ty::Logic,
                    Ty::Unknown | Ty::Any | Ty::Never => Ty::Unknown,
                    other => {
                        let shown = self.show(&other);
                        self.warn(
                            "V3005",
                            e.span,
                            format!(
                                "`?` on a value of type `{}` always succeeds; it only tests options and logic values",
                                shown
                            ),
                        )
                        .help = Some(
                            "remove the `?`, or compare the value instead (`X > 0`, `X = Y`)"
                                .into(),
                        );
                        other
                    }
                }
            }
            ExprKind::Call {
                callee,
                args,
                bracket,
            } => self.call(e, callee, args, *bracket),
            ExprKind::Member(obj, name, nsp) => self.member(e, obj, name, *nsp),
            ExprKind::SuperCall { method, args } => {
                let parent_sig = self.cur_class.and_then(|c| {
                    let me = c;
                    self.ancestors(c)
                        .into_iter()
                        .filter(move |a| *a != me)
                        .find_map(|a| {
                            self.classes[a]
                                .methods
                                .iter()
                                .find(|m| &m.name == method)
                                .cloned()
                        })
                });
                match parent_sig {
                    Some(sig) => self.apply_sig(&sig, e, e.span.end, args, false, false),
                    None => {
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        if self.external.is_none() {
                            self.err(
                                "V1003",
                                e.span,
                                format!("no parent class declares a method `{}`", method),
                            );
                        }
                        Ty::Unknown
                    }
                }
            }
            ExprKind::If { arms, else_body } => {
                let mut result: Option<Ty> = None;
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    let before = self.failable_seen;
                    let saved = std::mem::replace(&mut self.failure_ctx, true);
                    let mut cond_tys = Vec::new();
                    for c in &arm.cond {
                        cond_tys.push(self.expr(c, None));
                    }
                    self.failure_ctx = saved;
                    if self.failable_seen == before && !arm.cond.is_empty() {
                        let c = &arm.cond[arm.cond.len() - 1];
                        let span = arm.cond[0].span.to(c.span);
                        let single_logic = arm.cond.len() == 1
                            && cond_tys[0] == Ty::Logic
                            && !matches!(c.kind, ExprKind::Logic(_));
                        let d = self.err("V3004", span, "this `if` condition can never fail");
                        if single_logic {
                            d.help = Some("a `logic` value does not fail by itself; query it with `?`, as in `if (Flag?)`".into());
                            d.fix = Some(reverse_syntax::Fix {
                                span: Span::new(c.span.end, c.span.end),
                                replacement: "?".into(),
                            });
                        } else {
                            d.help = Some("a condition needs an expression that can fail: a comparison (`X > 0`), a query (`Opt?`), or a `[]` call (`Items[0]`)".into());
                        }
                    }
                    if arm.cond.is_empty() {
                        self.err("V3004", arm.body.span, "an `if` needs a condition");
                    }
                    self.failable_seen = before;
                    let t = self.expr(&arm.body, expect);
                    self.scopes.pop();
                    result = Some(match result {
                        None => t,
                        Some(r) => self.join(&r, &t),
                    });
                }
                match else_body {
                    Some(eb) => {
                        let t = self.expr(eb, expect);
                        let r = result.unwrap_or(Ty::Never);
                        self.join(&r, &t)
                    }
                    None => Ty::Void,
                }
            }
            ExprKind::For { clauses, body } => {
                self.scopes.push(HashMap::new());
                let before = self.failable_seen;
                let saved = std::mem::replace(&mut self.failure_ctx, true);
                for c in clauses {
                    match c {
                        ForClause::Gen {
                            key,
                            name,
                            name_span,
                            iter,
                        } => {
                            let (kt, vt) = if let ExprKind::Range(lo, hi) = &iter.kind {
                                for side in [lo, hi] {
                                    let t = self.expr(side, Some(&Ty::Int));
                                    self.require(side, &t, &Ty::Int, "range bound");
                                }
                                (Ty::Int, Ty::Int)
                            } else {
                                match self.expr(iter, None) {
                                    Ty::Array(t) => (Ty::Int, *t),
                                    Ty::Map(k, v) => (*k, *v),
                                    Ty::Str => (Ty::Int, Ty::Char),
                                    Ty::Unknown | Ty::Any | Ty::Never => (Ty::Unknown, Ty::Unknown),
                                    other => {
                                        let shown = self.show(&other);
                                        let d = self.err(
                                            "V2001",
                                            iter.span,
                                            format!(
                                                "`for` cannot iterate a value of type `{}`",
                                                shown
                                            ),
                                        );
                                        d.help = Some(match other {
                                            Ty::Int => "to count, iterate a range: `for (I := 0..N - 1)`".into(),
                                            Ty::Opt(_) => "unwrap the option first with `?`".into(),
                                            _ => "`for` iterates arrays, maps, strings and int ranges".into(),
                                        });
                                        (Ty::Unknown, Ty::Unknown)
                                    }
                                }
                            };
                            if let Some((kname, ksp)) = key {
                                self.declare(kname, *ksp, kt, false);
                            }
                            self.declare(name, *name_span, vt, false);
                        }
                        ForClause::Expr(x) => {
                            self.expr(x, None);
                        }
                    }
                }
                self.failure_ctx = saved;
                self.failable_seen = before;
                let saved_loop = std::mem::replace(&mut self.loop_depth, 0);
                let elem_expect = match expect {
                    Some(Ty::Array(t)) => Some((**t).clone()),
                    _ => None,
                };
                self.no_branch_in_loop(body);
                let t = self.expr(body, elem_expect.as_ref());
                self.loop_depth = saved_loop;
                self.scopes.pop();
                Ty::Array(Box::new(if t == Ty::Never { Ty::Void } else { t }))
            }
            ExprKind::Loop(body) => {
                self.no_branch_in_loop(body);
                self.loop_depth += 1;
                self.expr(body, None);
                self.loop_depth -= 1;
                // In suspending code an endless loop is normal: it waits, and
                // cancellation ends it.
                let suspending = self
                    .cur_fn
                    .as_ref()
                    .map(|f| f.effects.suspends)
                    .unwrap_or(false)
                    || self.in_spawn;
                if !suspending && !contains_exit(body) {
                    self.warn(
                        "V9003",
                        e.span,
                        "this `loop` has no `break` or `return`, so it never ends",
                    )
                    .help = Some("add an exit such as `if (Count >= Limit) then break`".into());
                }
                Ty::Void
            }
            ExprKind::Break => {
                if self.loop_depth == 0 {
                    self.err("V5003", e.span, "`break` is only allowed inside `loop`").help =
                        Some("`for` cannot be exited early; filter in the header instead: `for (X : Xs, X > 0)`".into());
                }
                Ty::Never
            }
            ExprKind::Return(value) => {
                let ret = self.cur_fn.as_ref().map(|f| f.ret.clone());
                match (ret, value) {
                    (None, _) => {
                        self.err(
                            "V9001",
                            e.span,
                            "`return` is only allowed inside a function",
                        );
                    }
                    (Some(rt), Some(v)) => {
                        let t = self.expr(v, Some(&rt));
                        self.require(v, &t, &rt, "return value");
                    }
                    (Some(rt), None) => {
                        if rt != Ty::Void && rt != Ty::Unknown {
                            let shown = self.show(&rt);
                            self.err(
                                "V2006",
                                e.span,
                                format!("`return` needs a value of type `{}`", shown),
                            );
                        }
                    }
                }
                Ty::Never
            }
            ExprKind::Case { scrutinee, arms } => self.case(e, scrutinee, arms, expect),
            ExprKind::ArrayLit(items) => {
                let elem_expect = match expect {
                    Some(Ty::Array(t)) => Some((**t).clone()),
                    Some(Ty::Str) => Some(Ty::Char),
                    _ => None,
                };
                let mut elem = Ty::Never;
                for it in items {
                    let t = self.expr(it, elem_expect.as_ref());
                    elem = self.join(&elem, &t);
                }
                if let Some(ex) = elem_expect {
                    if self.assignable(&elem, &ex) {
                        return Ty::Array(Box::new(ex));
                    }
                }
                Ty::Array(Box::new(elem))
            }
            ExprKind::MapLit(items) => {
                let (ke, ve) = match expect {
                    Some(Ty::Map(k, v)) => (Some((**k).clone()), Some((**v).clone())),
                    _ => (None, None),
                };
                let (mut kt, mut vt) = (Ty::Never, Ty::Never);
                for (k, v) in items {
                    let a = self.expr(k, ke.as_ref());
                    let b = self.expr(v, ve.as_ref());
                    if matches!(a, Ty::Array(_) | Ty::Map(..) | Ty::Float) {
                        let shown = self.show(&a);
                        self.err("V2001", k.span, format!("a value of type `{}` cannot be a map key", shown)).help =
                            Some("keys must be comparable: int, string, char, logic, enums, tuples of those, or `<unique>` classes".into());
                    }
                    kt = self.join(&kt, &a);
                    vt = self.join(&vt, &b);
                }
                if let (Some(k), Some(v)) = (ke, ve) {
                    if self.assignable(&kt, &k) && self.assignable(&vt, &v) {
                        return Ty::Map(Box::new(k), Box::new(v));
                    }
                }
                Ty::Map(Box::new(kt), Box::new(vt))
            }
            ExprKind::OptionLit(inner) => {
                let inner_expect = match expect {
                    Some(Ty::Opt(t)) => Some((**t).clone()),
                    _ => None,
                };
                match inner {
                    Some(x) => {
                        let (t, _) = self.in_hidden_failure(x, inner_expect.as_ref());
                        Ty::Opt(Box::new(t))
                    }
                    None => Ty::Opt(Box::new(inner_expect.unwrap_or(Ty::Never))),
                }
            }
            ExprKind::LogicLit(inner) => {
                let (_, failed) = self.in_hidden_failure(inner, None);
                if !failed {
                    self.warn(
                        "V9004",
                        inner.span,
                        "`logic{...}` wraps an expression that cannot fail, so it is always `true`",
                    );
                }
                Ty::Logic
            }
            ExprKind::Archetype { ty, fields } => self.archetype(e, ty, fields),
            ExprKind::Func(f) => {
                self.err("V6001", f.name_span, "nested functions are not supported")
                    .help = Some("define the function at module level or as a class method".into());
                Ty::Unknown
            }
            ExprKind::Class(c) => {
                self.err(
                    "V9001",
                    c.name_span,
                    "classes, structs and interfaces must be defined at module level",
                );
                Ty::Unknown
            }
            ExprKind::Enum(en) => {
                self.err(
                    "V9001",
                    en.name_span,
                    "enums must be defined at module level",
                );
                Ty::Unknown
            }
            ExprKind::Module { .. } | ExprKind::Using(_) => Ty::Void,
            ExprKind::Concurrent { kind, arms } => {
                let suspends = self
                    .cur_fn
                    .as_ref()
                    .map(|f| f.effects.suspends)
                    .unwrap_or(false);
                if !suspends && !self.in_spawn {
                    self.err(
                        "V4001",
                        e.span,
                        format!("`{}` needs a `<suspends>` function", kind.as_str()),
                    )
                    .help = Some("add `<suspends>` to the enclosing function".into());
                }
                let saved_spawn = self.in_spawn;
                self.in_spawn = self.in_spawn || !suspends;
                if *kind != ConcKind::Branch && arms.len() < 2 {
                    self.err(
                        "V2002",
                        e.span,
                        format!(
                            "`{}` needs at least two expressions to run concurrently",
                            kind.as_str()
                        ),
                    );
                }
                let mut ts = Vec::new();
                for a in arms {
                    ts.push(self.expr(a, None));
                }
                self.in_spawn = saved_spawn;
                match kind {
                    ConcKind::Sync => Ty::Tuple(ts),
                    ConcKind::Race | ConcKind::Rush => {
                        ts.iter().fold(Ty::Never, |acc, t| self.join(&acc, t))
                    }
                    ConcKind::Branch => Ty::Void,
                }
            }
            ExprKind::Spawn(body) => {
                let saved = std::mem::replace(&mut self.in_spawn, true);
                // A spawned body is not part of any enclosing failure context.
                let saved_ctx = std::mem::replace(&mut self.failure_ctx, false);
                let t = self.expr(body, None);
                self.failure_ctx = saved_ctx;
                self.in_spawn = saved;
                Ty::Task(Box::new(t))
            }
            ExprKind::Defer(body) => {
                self.expr(body, None);
                Ty::Void
            }
            ExprKind::Error => Ty::Unknown,
        }
    }

    /// `branch` and `rush` leave tasks running, so the Book forbids them
    /// directly in a loop body, where they would pile up.
    fn no_branch_in_loop(&mut self, body: &Expr) {
        let stmts: &[Expr] = match &body.kind {
            ExprKind::Block(s) => s,
            _ => std::slice::from_ref(body),
        };
        for s in stmts {
            let inner = match &s.kind {
                ExprKind::Def(d) => d.value.as_ref(),
                _ => Some(s),
            };
            if let Some(Expr {
                kind:
                    ExprKind::Concurrent {
                        kind: kind @ (ConcKind::Branch | ConcKind::Rush),
                        ..
                    },
                span,
            }) = inner
            {
                self.err(
                    "V4007",
                    *span,
                    format!("`{}` cannot be used directly in a loop body", kind.as_str()),
                )
                .help =
                    Some("move it into a `<suspends>` function and call that from the loop".into());
            }
        }
    }

    fn ident(&mut self, name: &str, span: Span) -> Ty {
        if let Some(v) = self.lookup_var(name) {
            if v.mutable {
                self.need_effect(
                    span,
                    &format!("reading the variable `{}`", name),
                    true,
                    false,
                    false,
                );
            }
            return v.ty;
        }
        if name == "Self" {
            if let Some(c) = self.cur_class {
                return Ty::Class(c);
            }
            self.err(
                "V1001",
                span,
                "`Self` is only available inside a class method",
            );
            return Ty::Unknown;
        }
        if let Some(c) = self.cur_class {
            if let Some(f) = self.find_field(c, name) {
                self.check_access(f.owner, f.access, name, span);
                if f.mutable {
                    self.need_effect(
                        span,
                        &format!("reading the field `{}`", name),
                        true,
                        false,
                        false,
                    );
                }
                return f.ty;
            }
            if let Some(m) = self.find_method(c, name) {
                return Ty::Func(Box::new(m));
            }
        }
        if let Some(v) = self.globals.get(name) {
            return v.ty.clone();
        }
        if let Some(sig) = self.funcs.get(name) {
            return Ty::Func(Box::new(sig.clone()));
        }
        if let Some(id) = self.class_ids.get(name) {
            return Ty::TypeOf(Box::new(Ty::Class(*id)));
        }
        if let Some(id) = self.enum_ids.get(name) {
            return Ty::TypeOf(Box::new(Ty::Enum(*id)));
        }
        if let Some(sig) = builtins::fixed_sig(name) {
            return Ty::Func(Box::new(sig));
        }
        if builtins::is_generic(name) || self.modules.contains(name) {
            return Ty::Unknown;
        }
        if matches!(
            name,
            "int" | "float" | "string" | "logic" | "char" | "void" | "any"
        ) {
            return Ty::TypeOf(Box::new(Ty::Unknown));
        }
        self.unknown_name(name, span);
        Ty::Unknown
    }

    fn unknown_name(&mut self, name: &str, span: Span) {
        let foreign: Option<(&str, Option<&str>)> = match name {
            "null" | "nil" | "None" | "nullptr" | "undefined" => {
                Some(("Verse has no null; an absent value is an empty option, written `false`, with type `?type`", Some("false")))
            }
            "this" | "self" => Some(("the current object is `Self`", Some("Self"))),
            "print" | "println" | "console" | "Log" | "printf" => Some(("print with `Print(\"text {Value}\")`", None)),
            "True" | "TRUE" => Some(("the logic literals are lowercase", Some("true"))),
            "False" | "FALSE" => Some(("the logic literals are lowercase", Some("false"))),
            "len" | "length" | "size" | "count" => Some(("the length of an array, map or string is the member `.Length`", None)),
            _ => None,
        };
        if let Some((help, fix)) = foreign {
            let d = self.err("V1001", span, format!("unknown name `{}`", name));
            d.help = Some(help.to_string());
            if let Some(f) = fix {
                d.fix = Some(reverse_syntax::Fix {
                    span,
                    replacement: f.to_string(),
                });
            }
            return;
        }
        let mut cands: Vec<String> = Vec::new();
        for s in &self.scopes {
            cands.extend(s.keys().cloned());
        }
        cands.extend(self.globals.keys().cloned());
        cands.extend(self.funcs.keys().cloned());
        cands.extend(self.class_ids.keys().cloned());
        cands.extend(self.enum_ids.keys().cloned());
        if let Some(c) = self.cur_class {
            for a in self.ancestors(c) {
                cands.extend(self.classes[a].fields.iter().map(|f| f.name.clone()));
                cands.extend(self.classes[a].methods.iter().map(|m| m.name.clone()));
            }
        }
        cands.extend(builtins::ALL.iter().map(|s| s.to_string()));
        let sug = closest(name, cands.iter().map(|s| s.as_str()));
        if let (Some(ext), None) = (self.external.clone(), &sug) {
            self.warn(
                "V6002",
                span,
                format!("`{}` is not defined in this file", name),
            )
            .help = Some(format!(
                "it may come from `{}`; API digests are not loaded, so uses of it are not checked",
                ext
            ));
            return;
        }
        let d = self.err("V1001", span, format!("unknown name `{}`", name));
        if let Some(s) = sug {
            d.help = Some(format!("did you mean `{}`?", s));
            d.fix = Some(reverse_syntax::Fix {
                span,
                replacement: s,
            });
        } else {
            d.help = Some("define it first with `Name := Value`, or check the spelling (Verse names are case-sensitive)".into());
        }
    }

    fn def(&mut self, d: &Def) -> Ty {
        let ann = d.ty.as_ref().map(|t| self.resolve_type(t));
        let vt = match &d.value {
            Some(v) => {
                let t = self.expr(v, ann.as_ref());
                if let Some(a) = &ann {
                    self.require(v, &t, a, "initial value");
                }
                t
            }
            None => {
                self.err(
                    "V2012",
                    d.name_span,
                    format!("`{}` needs an initial value", d.name),
                )
                .help = Some(format!(
                    "write `{}{} : type = Value`",
                    if d.is_var { "var " } else { "" },
                    d.name
                ));
                Ty::Unknown
            }
        };
        if self.scopes.is_empty() {
            return Ty::Void;
        }
        if d.is_var {
            self.need_effect(
                d.name_span,
                &format!("declaring the variable `{}`", d.name),
                false,
                false,
                true,
            );
        }
        let ty = match ann {
            Some(a) => a,
            None => match vt {
                Ty::Void => {
                    self.err(
                        "V2001",
                        d.name_span,
                        format!(
                            "`{}` is defined from an expression that produces no value",
                            d.name
                        ),
                    );
                    Ty::Unknown
                }
                Ty::Array(ref t) if **t == Ty::Never => {
                    self.err(
                        "V2014",
                        d.name_span,
                        format!(
                            "the element type of `{}` cannot be inferred from an empty literal",
                            d.name
                        ),
                    )
                    .help = Some(format!("add a type: `{} : []int = array{{}}`", d.name));
                    Ty::Unknown
                }
                t => t,
            },
        };
        self.declare(&d.name, d.name_span, ty.clone(), d.is_var);
        ty
    }

    /// Type and mutability of an assignment target.
    fn place(&mut self, target: &Expr) -> (Ty, Result<(), String>) {
        match &target.kind {
            ExprKind::Ident(name) => {
                if let Some(v) = self.lookup_var(name) {
                    let m = if v.mutable {
                        Ok(())
                    } else {
                        Err(format!("`{}` is not declared with `var`", name))
                    };
                    return (v.ty, m);
                }
                if let Some(c) = self.cur_class {
                    if let Some(f) = self.find_field(c, name) {
                        let m = if f.mutable {
                            Ok(())
                        } else {
                            Err(format!("field `{}` is not declared with `var`", name))
                        };
                        return (f.ty, m);
                    }
                }
                if let Some(v) = self.globals.get(name).cloned() {
                    let m = if v.mutable {
                        Ok(())
                    } else {
                        Err(format!("`{}` is a constant", name))
                    };
                    return (v.ty, m);
                }
                let t = self.ident(name, target.span);
                (t, Ok(()))
            }
            ExprKind::Member(obj, name, nsp) => {
                let ot = if let ExprKind::Ident(n) = &obj.kind {
                    match (n.as_str(), self.cur_class, self.lookup_var(n)) {
                        ("Self", Some(c), _) => Ty::Class(c),
                        // Reading through a `var` to reach a field is checked by `place`.
                        (_, _, Some(v)) => v.ty,
                        _ => self.expr(obj, None),
                    }
                } else {
                    self.expr(obj, None)
                };
                match ot {
                    Ty::Class(cid) => match self.find_field(cid, name) {
                        Some(f) => {
                            if self.classes[cid].kind == ClassKind::Struct {
                                // A struct is a value: the variable holding it must be mutable.
                                let (_, base) = self.place_quiet(obj);
                                (f.ty, base.map_err(|why| format!("{}; a struct field can only change through a `var` that holds the struct", why)))
                            } else {
                                let m = if f.mutable {
                                    Ok(())
                                } else {
                                    Err(format!("field `{}` is not declared with `var`", name))
                                };
                                (f.ty, m)
                            }
                        }
                        None => {
                            let t = self.no_member(cid, name, *nsp);
                            (t, Ok(()))
                        }
                    },
                    _ => (Ty::Unknown, Ok(())),
                }
            }
            ExprKind::Call {
                callee,
                args,
                bracket: true,
            } => {
                let (ct, base) = self.place(callee);
                let elem = match &ct {
                    Ty::Array(t) => {
                        if let Some(a) = args.first() {
                            let it = self.expr(&a.value, Some(&Ty::Int));
                            self.require(&a.value, &it, &Ty::Int, "array index");
                        }
                        self.failable(target.span, "assigning to an array element");
                        (**t).clone()
                    }
                    Ty::Map(k, v) => {
                        if let Some(a) = args.first() {
                            let kt = self.expr(&a.value, Some(k));
                            self.require(&a.value, &kt, k, "map key");
                        }
                        self.failable(target.span, "assigning to a map entry");
                        (**v).clone()
                    }
                    Ty::Str => {
                        self.failable(target.span, "assigning to a string character");
                        Ty::Char
                    }
                    _ => {
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        Ty::Unknown
                    }
                };
                (elem, base)
            }
            _ => {
                let t = self.expr(target, None);
                (
                    t,
                    Err("this expression is not something that can be assigned to".into()),
                )
            }
        }
    }

    fn place_quiet(&mut self, target: &Expr) -> (Ty, Result<(), String>) {
        let n = self.diags.len();
        let seen = self.failable_seen;
        let r = self.place(target);
        self.diags.truncate(n);
        self.failable_seen = seen;
        r
    }

    fn set(&mut self, e: &Expr, target: &Expr, op: Option<BinOp>, value: &Expr) -> Ty {
        let (tt, mutable) = self.place(target);
        if let Err(why) = mutable {
            let d = self.err("V5001", target.span, format!("cannot assign: {}", why));
            d.help =
                Some("only names declared with `var` can change: `var Name : type = Value`".into());
        }
        self.need_effect(e.span, "`set`", false, true, false);
        let vt = self.expr(value, Some(&tt));
        match op {
            None => self.require(value, &vt, &tt, "assigned value"),
            Some(op) => {
                let rt = self.arith(e, op, &tt, &vt, target, value);
                if !self.assignable(&rt, &tt) && rt != Ty::Unknown {
                    let (a, b) = (self.show(&rt), self.show(&tt));
                    self.err(
                        "V2001",
                        e.span,
                        format!(
                            "`{}=` produces `{}`, which cannot be stored in a `{}`",
                            op.symbol(),
                            a,
                            b
                        ),
                    );
                }
            }
        }
        tt
    }

    fn unary(&mut self, e: &Expr, op: UnOp, inner: &Expr) -> Ty {
        match op {
            UnOp::Neg | UnOp::Pos => {
                let t = self.expr(inner, None);
                match t {
                    Ty::Int | Ty::Float | Ty::Rational | Ty::Unknown => t,
                    other => {
                        let shown = self.show(&other);
                        self.err(
                            "V2004",
                            e.span,
                            format!(
                                "unary `{}` is not defined for `{}`",
                                if op == UnOp::Neg { "-" } else { "+" },
                                shown
                            ),
                        );
                        Ty::Unknown
                    }
                }
            }
            UnOp::Not => {
                let before = self.failable_seen;
                let saved = std::mem::replace(&mut self.failure_ctx, true);
                // Names bound inside `not` are not visible afterwards.
                self.scopes.push(HashMap::new());
                let t = self.expr(inner, None);
                let had_scope = self.scopes.pop().is_some();
                let _ = had_scope;
                self.failure_ctx = saved;
                if self.failable_seen == before {
                    let d = self.err(
                        "V3004",
                        inner.span,
                        "`not` needs an expression that can fail",
                    );
                    if t == Ty::Logic {
                        d.help = Some(
                            "a `logic` value does not fail by itself; query it: `not Flag?`".into(),
                        );
                        d.fix = Some(reverse_syntax::Fix {
                            span: Span::new(inner.span.end, inner.span.end),
                            replacement: "?".into(),
                        });
                    } else {
                        d.help = Some("`not X` succeeds when `X` fails; `X` must be a comparison, a query (`Opt?`) or a `[]` call".into());
                    }
                }
                self.failable_seen = before;
                self.failable(e.span, "`not`");
                Ty::Void
            }
        }
    }

    fn arith(&mut self, e: &Expr, op: BinOp, lt: &Ty, rt: &Ty, l: &Expr, r: &Expr) -> Ty {
        use Ty::*;
        if *lt == Unknown || *rt == Unknown || *lt == Never || *rt == Never {
            return Unknown;
        }
        let text = |s: &Self, x: &Expr| {
            s.src
                .get(x.span.start as usize..x.span.end as usize)
                .unwrap_or("X")
                .to_string()
        };
        match (op, lt, rt) {
            (BinOp::Add | BinOp::Sub | BinOp::Mul, Int, Int) => Int,
            (BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div, Float, Float) => Float,
            (BinOp::Mul, Int, Float) | (BinOp::Mul, Float, Int) => Float,
            (BinOp::Div, Int, Int) => {
                self.failable(
                    e.span,
                    "integer division `/` (it fails when dividing by zero)",
                );
                Rational
            }
            (BinOp::Add, Str, Str) => Str,
            (BinOp::Add, Array(a), Array(b)) => Array(Box::new(self.join(a, b))),
            (BinOp::Add, Str, Array(_)) | (BinOp::Add, Array(_), Str) => Str,
            (BinOp::Add, Array(a), Tuple(ts)) if ts.iter().all(|t| self.assignable(t, a)) => {
                lt.clone()
            }
            (BinOp::Add, Str, other) | (BinOp::Add, other, Str) => {
                let (ls, rs) = (text(self, l), text(self, r));
                let shown = self.show(other);
                let can_show = self.stringable(other);
                let d = self.err(
                    "V2004",
                    e.span,
                    format!("`+` cannot join `string` and `{}`", shown),
                );
                if can_show {
                    let (a, b) = if *lt == Str {
                        (ls.clone(), format!("\"{{{}}}\"", rs))
                    } else {
                        (format!("\"{{{}}}\"", ls), rs.clone())
                    };
                    d.help = Some(format!("Verse does not convert to string implicitly; interpolate instead, for example `{} + {}` or one string like `\"Score: {{Score}}\"`", a, b));
                } else {
                    d.help = Some("only strings can be joined with `+`; convert the other value to a string first".into());
                }
                Unknown
            }
            (BinOp::Add | BinOp::Sub | BinOp::Div, Int, Float)
            | (BinOp::Add | BinOp::Sub | BinOp::Div, Float, Int) => {
                let which = if *lt == Int {
                    text(self, l)
                } else {
                    text(self, r)
                };
                self.err(
                    "V2004",
                    e.span,
                    format!("`{}` cannot mix `int` and `float`", op.symbol()),
                )
                .help = Some(format!("convert the int first: `{} * 1.0`", which));
                Unknown
            }
            (BinOp::Add | BinOp::Sub | BinOp::Mul, Rational | Int, Rational | Int) => Rational,
            (_, Rational, _) | (_, _, Rational) => {
                self.err("V6001", e.span, "arithmetic on `rational` values is not supported by this toolchain yet").help =
                    Some("convert with `Floor(A / B)` or `Ceil(A / B)` first, or use `Quotient[A, B]` and `Mod[A, B]`".into());
                Unknown
            }
            _ => {
                let (a, b) = (self.show(lt), self.show(rt));
                self.err(
                    "V2004",
                    e.span,
                    format!("`{}` is not defined for `{}` and `{}`", op.symbol(), a, b),
                );
                Unknown
            }
        }
    }

    fn binary(&mut self, e: &Expr, op: BinOp, l: &Expr, r: &Expr, expect: Option<&Ty>) -> Ty {
        match op {
            BinOp::And => {
                self.expr(l, None);
                self.expr(r, expect)
            }
            BinOp::Or => {
                let (lt, left_can_fail) = self.in_hidden_failure(l, expect);
                if !left_can_fail {
                    self.warn(
                        "V9004",
                        l.span,
                        "the left side of `or` cannot fail, so the right side is never used",
                    );
                }
                let rt = self.expr(r, Some(&lt));
                self.join(&lt, &rt)
            }
            _ if op.is_comparison() => {
                let lt = self.expr(l, None);
                let rt = self.expr(r, Some(&lt));
                let ordered = matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
                let numeric = |t: &Ty| {
                    matches!(
                        t,
                        Ty::Int | Ty::Float | Ty::Rational | Ty::Unknown | Ty::Never
                    )
                };
                if ordered && (!numeric(&lt) || !numeric(&rt)) {
                    let (a, b) = (self.show(&lt), self.show(&rt));
                    self.err(
                        "V2004",
                        e.span,
                        format!(
                            "`{}` only orders numbers, but this compares `{}` and `{}`",
                            op.symbol(),
                            a,
                            b
                        ),
                    );
                } else if !self.assignable(&lt, &rt) && !self.assignable(&rt, &lt) {
                    let (a, b) = (self.show(&lt), self.show(&rt));
                    let d = self.warn(
                        "V2008",
                        e.span,
                        format!("comparing `{}` with `{}` always fails", a, b),
                    );
                    d.help = Some(match (&lt, &rt) {
                        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => "values of different types are never equal; convert the int with `* 1.0`".into(),
                        (Ty::Opt(_), _) => "unwrap the option before comparing: `Opt? = Value`".into(),
                        _ => "values of different types are never equal".into(),
                    });
                }
                if let (Ty::Class(c), false) = (&lt, ordered) {
                    if self.classes[*c].kind != ClassKind::Struct && !self.is_unique(*c) {
                        let cname = self.classes[*c].name.clone();
                        self.err("V2015", e.span, format!("`{}` instances cannot be compared: the class is not `<unique>`", cname)).help =
                            Some(format!("declare it `{} := class<unique>:` to compare instances by identity, or compare fields instead", cname));
                    }
                }
                self.failable(e.span, &format!("the comparison `{}`", op.symbol()));
                lt
            }
            _ => {
                let lt = self.expr(l, expect);
                let rt = self.expr(r, Some(&lt));
                self.arith(e, op, &lt, &rt, l, r)
            }
        }
    }

    fn member(&mut self, _e: &Expr, obj: &Expr, name: &str, nsp: Span) -> Ty {
        if let ExprKind::Ident(n) = &obj.kind {
            if self.lookup_var(n).is_none() {
                if let Some(&eid) = self.enum_ids.get(n) {
                    if !self.enums[eid].values.iter().any(|v| v == name) {
                        let sug = closest(name, self.enums[eid].values.iter().map(|s| s.as_str()));
                        let all = self.enums[eid].values.join(", ");
                        let d = self.err(
                            "V1003",
                            nsp,
                            format!("enum `{}` has no value `{}`", n, name),
                        );
                        d.help = Some(match &sug {
                            Some(s) => format!("did you mean `{}.{}`?", n, s),
                            None => format!("its values are: {}", all),
                        });
                        if let Some(s) = sug {
                            d.fix = Some(reverse_syntax::Fix {
                                span: nsp,
                                replacement: s,
                            });
                        }
                    }
                    return Ty::Enum(eid);
                }
                if self.modules.contains(n) {
                    return self.ident(name, nsp);
                }
            }
        }
        let ot = self.expr(obj, None);
        match ot {
            Ty::Class(cid) => {
                if let Some(f) = self.find_field(cid, name) {
                    self.check_access(f.owner, f.access, name, nsp);
                    if f.mutable {
                        self.need_effect(
                            nsp,
                            &format!("reading the field `{}`", name),
                            true,
                            false,
                            false,
                        );
                    }
                    return f.ty;
                }
                if let Some(m) = self.find_method(cid, name) {
                    return Ty::Func(Box::new(m));
                }
                self.no_member(cid, name, nsp)
            }
            Ty::Array(_) | Ty::Str | Ty::Map(..) => {
                if name == "Length" {
                    return Ty::Int;
                }
                let shown = self.show(&ot);
                let d = self.err(
                    "V1003",
                    nsp,
                    format!("`{}` has no member `{}`", shown, name),
                );
                d.help = Some(match name {
                    "length" | "size" | "Size" | "Count" | "count" | "Num" | "len" => "the element count is `.Length`".into(),
                    "Add" | "Push" | "Append" | "push" | "append" | "add" => "arrays are values; append with `set Items += array{NewItem}`".into(),
                    "Contains" | "contains" | "Has" => "test membership with `Items.Find[Value]` in a failure context".into(),
                    _ => "containers have `.Length`; arrays also have methods such as `Find[]`, `Slice[]`, `RemoveElement[]`".into(),
                });
                if matches!(
                    name,
                    "length" | "size" | "Size" | "Count" | "count" | "Num" | "len"
                ) {
                    d.fix = Some(reverse_syntax::Fix {
                        span: nsp,
                        replacement: "Length".into(),
                    });
                }
                Ty::Unknown
            }
            Ty::Opt(_) => {
                let text = self
                    .src
                    .get(obj.span.start as usize..obj.span.end as usize)
                    .unwrap_or("Opt")
                    .to_string();
                self.err(
                    "V1003",
                    nsp,
                    format!("cannot access `{}` on an option", name),
                )
                .help = Some(format!(
                    "unwrap it first in a failure context: `if (Value := {}?) {{ Value.{} }}`",
                    text, name
                ));
                Ty::Unknown
            }
            Ty::Unknown | Ty::Any | Ty::Never | Ty::TypeOf(_) => Ty::Unknown,
            other => {
                let shown = self.show(&other);
                self.err(
                    "V1003",
                    nsp,
                    format!("`{}` has no member `{}`", shown, name),
                );
                Ty::Unknown
            }
        }
    }

    fn no_member(&mut self, cid: usize, name: &str, nsp: Span) -> Ty {
        // A class with a parent from an unloaded digest may inherit anything.
        let cname = self.classes[cid].name.clone();
        let mut cands: Vec<String> = Vec::new();
        for a in self.ancestors(cid) {
            cands.extend(self.classes[a].fields.iter().map(|f| f.name.clone()));
            cands.extend(self.classes[a].methods.iter().map(|m| m.name.clone()));
        }
        let sug = closest(name, cands.iter().map(|s| s.as_str()));
        if self.external.is_some() && sug.is_none() {
            self.warn(
                "V6002",
                nsp,
                format!("`{}` has no member `{}` in this file", cname, name),
            )
            .help = Some("it may be inherited from an API type whose digest is not loaded, or the built-in stand-in may not model it; compile in UEFN to verify".into());
            return Ty::Unknown;
        }
        let d = self.err(
            "V1003",
            nsp,
            format!("`{}` has no member `{}`", cname, name),
        );
        if let Some(s) = sug {
            d.help = Some(format!("did you mean `{}`?", s));
            d.fix = Some(reverse_syntax::Fix {
                span: nsp,
                replacement: s,
            });
        }
        Ty::Unknown
    }

    /// The type written inside `event(...)` when it appears as an expression.
    fn payload_type(&self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Ident(n) => match n.as_str() {
                "int" => Ty::Int,
                "float" => Ty::Float,
                "string" | "message" => Ty::Str,
                "logic" => Ty::Logic,
                "char" => Ty::Char,
                "void" => Ty::Void,
                "any" => Ty::Any,
                other => match (self.class_ids.get(other), self.enum_ids.get(other)) {
                    (Some(c), _) => Ty::Class(*c),
                    (_, Some(en)) => Ty::Enum(*en),
                    _ => Ty::Unknown,
                },
            },
            // `?t`, which the parser hands over as a call to `?`.
            ExprKind::Call { callee, args, .. } if matches!(&callee.kind, ExprKind::Ident(n) if n == "?") => {
                match args.first() {
                    Some(a) => Ty::Opt(Box::new(self.payload_type(&a.value))),
                    None => Ty::Unknown,
                }
            }
            _ => Ty::Unknown,
        }
    }

    fn archetype(&mut self, e: &Expr, ty: &Expr, fields: &[(String, Span, Expr)]) -> Ty {
        if let ExprKind::Call {
            callee,
            args,
            bracket: false,
        } = &ty.kind
        {
            if matches!(&callee.kind, ExprKind::Ident(n) if n == "event") {
                let inner = match args.first() {
                    None => Ty::Void,
                    Some(a) => self.payload_type(&a.value),
                };
                if let Some((name, nsp, _)) = fields.first() {
                    self.err("V1003", *nsp, format!("an event has no field `{}`", name))
                        .help = Some("create an event with empty braces: `event(int){}`".into());
                }
                return Ty::Event(Box::new(inner));
            }
        }
        let cid = match &ty.kind {
            ExprKind::Ident(n) => self.class_ids.get(n).copied(),
            ExprKind::Member(_, n, _) => self.class_ids.get(n).copied(),
            _ => None,
        };
        let Some(cid) = cid else {
            if let ExprKind::Ident(n) = &ty.kind {
                if self.enum_ids.contains_key(n) {
                    self.err(
                        "V2011",
                        ty.span,
                        format!("`{}` is an enum; write `{}.Value`", n, n),
                    );
                } else {
                    self.unknown_type(n, ty.span);
                }
            }
            for (_, _, v) in fields {
                self.expr(v, None);
            }
            return Ty::Unknown;
        };
        if self.classes[cid].is_abstract {
            let (kind, name) = (self.classes[cid].kind, self.classes[cid].name.clone());
            self.err(
                "V2011",
                ty.span,
                format!(
                    "`{}` is {} and cannot be instantiated",
                    name,
                    if kind == ClassKind::Interface {
                        "an interface"
                    } else {
                        "abstract"
                    }
                ),
            )
            .help = Some("instantiate a concrete subclass instead".into());
        }
        let all = self.all_fields(cid);
        let mut given: Vec<&str> = Vec::new();
        for (name, nsp, value) in fields {
            if given.contains(&name.as_str()) {
                self.err(
                    "V1004",
                    *nsp,
                    format!("field `{}` is initialised twice", name),
                );
            }
            given.push(name);
            match all.iter().find(|f| &f.name == name) {
                Some(f) => {
                    let t = self.expr(value, Some(&f.ty));
                    self.require(value, &t, &f.ty, &format!("field `{}`", name));
                }
                None => {
                    self.expr(value, None);
                    let cname = self.classes[cid].name.clone();
                    let sug = closest(name, all.iter().map(|f| f.name.as_str()));
                    if self.external.is_some() && sug.is_none() {
                        continue;
                    }
                    let d = self.err(
                        "V1003",
                        *nsp,
                        format!("`{}` has no field `{}`", cname, name),
                    );
                    if let Some(s) = sug {
                        d.help = Some(format!("did you mean `{}`?", s));
                        d.fix = Some(reverse_syntax::Fix {
                            span: *nsp,
                            replacement: s,
                        });
                    } else if !all.is_empty() {
                        d.help = Some(format!(
                            "its fields are: {}",
                            all.iter()
                                .map(|f| f.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }
        }
        let missing: Vec<String> = all
            .iter()
            .filter(|f| !f.has_default && !given.contains(&f.name.as_str()))
            .map(|f| f.name.clone())
            .collect();
        if !missing.is_empty() {
            let cname = self.classes[cid].name.clone();
            let example = missing
                .iter()
                .map(|m| format!("{} := ...", m))
                .collect::<Vec<_>>()
                .join(", ");
            self.err(
                "V2005",
                e.span,
                format!(
                    "`{}` is missing required field(s): {}",
                    cname,
                    missing.join(", ")
                ),
            )
            .help = Some(format!(
                "fields without a default must be set: `{}{{{}}}`",
                cname, example
            ));
        }
        Ty::Class(cid)
    }

    fn case(&mut self, e: &Expr, scrutinee: &Expr, arms: &[CaseArm], expect: Option<&Ty>) -> Ty {
        let st = self.expr(scrutinee, None);
        if !matches!(
            st,
            Ty::Int | Ty::Logic | Ty::Char | Ty::Str | Ty::Enum(_) | Ty::Unknown | Ty::Never
        ) {
            let shown = self.show(&st);
            self.err("V2010", scrutinee.span, format!("`case` cannot match a value of type `{}`", shown)).help =
                Some("`case` matches int, logic, char, string and enum values; use `if`/`else if` for anything else".into());
        }
        let mut result = Ty::Never;
        let mut has_wildcard = false;
        let mut covered: Vec<String> = Vec::new();
        for arm in arms {
            match &arm.pattern {
                None => {
                    if has_wildcard {
                        self.err("V8002", arm.span, "`case` has more than one `_` arm");
                    }
                    has_wildcard = true;
                }
                Some(p) => {
                    if has_wildcard {
                        self.warn(
                            "V9002",
                            arm.span,
                            "this arm is never reached because `_` comes before it",
                        );
                    }
                    let saved = std::mem::replace(&mut self.failure_ctx, true);
                    let pt = self.expr(p, Some(&st));
                    self.failure_ctx = saved;
                    self.require(p, &pt, &st, "case pattern");
                    if let ExprKind::Member(_, v, _) = &p.kind {
                        if covered.contains(v) {
                            self.warn(
                                "V9002",
                                arm.span,
                                format!("`{}` is already matched by an earlier arm", v),
                            );
                        }
                        covered.push(v.clone());
                    }
                }
            }
            let t = self.expr(&arm.body, expect);
            result = self.join(&result, &t);
        }
        if !has_wildcard {
            let missing: Option<Vec<String>> = match &st {
                Ty::Enum(id) if !self.enums[*id].open => {
                    let m: Vec<String> = self.enums[*id]
                        .values
                        .iter()
                        .filter(|v| !covered.contains(v))
                        .cloned()
                        .collect();
                    if m.is_empty() {
                        None
                    } else {
                        Some(m)
                    }
                }
                Ty::Unknown | Ty::Never => None,
                _ => Some(Vec::new()),
            };
            if let Some(m) = missing {
                if self.failure_ctx {
                    self.failable_seen += 1;
                } else {
                    let d = self.err(
                        "V8001",
                        e.span,
                        if m.is_empty() {
                            "`case` does not cover every value".to_string()
                        } else {
                            format!("`case` does not cover: {}", m.join(", "))
                        },
                    );
                    d.help = Some("add the missing arms, or a default arm: `_ => Value`".into());
                }
            }
        }
        result
    }

    // ------------------------------------------------------------ calls

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg], bracket: bool) -> Ty {
        let callee_end = callee.span.end;
        match &callee.kind {
            ExprKind::Ident(name) => {
                if let Some(v) = self.lookup_var(name) {
                    return self.call_value(e, callee, v.ty, args, bracket);
                }
                if let Some(c) = self.cur_class {
                    if let Some(m) = self.pick_method(c, name, args) {
                        if let Some((owner, access)) = self.method_access(c, name) {
                            self.check_access(owner, access, name, callee.span);
                        }
                        return self.apply_sig(&m, e, callee_end, args, bracket, true);
                    }
                    if let Some(f) = self.find_field(c, name) {
                        return self.call_value(e, callee, f.ty, args, bracket);
                    }
                }
                if let Some(v) = self.globals.get(name).cloned() {
                    return self.call_value(e, callee, v.ty, args, bracket);
                }
                if let Some(sigs) = self.overloads.get(name).cloned() {
                    let ats = self.quiet_arg_types(args);
                    let n = ats.len();
                    let pick = sigs.iter().find(|s| {
                        let pos: Vec<&ParamSig> = s.params.iter().filter(|p| !p.named).collect();
                        let required = pos.iter().filter(|p| !p.has_default).count();
                        n >= required
                            && n <= pos.len()
                            && ats.iter().zip(&pos).all(|(a, p)| {
                                self.assignable(a, &p.ty) || matches!(p.ty, Ty::Param(_))
                            })
                    });
                    match pick {
                        Some(sig) => {
                            let sig = sig.clone();
                            return self.apply_sig(&sig, e, callee_end, args, bracket, true);
                        }
                        None if name == "ToString" => {
                            return builtins::generic_call(
                                self, name, e, callee_end, args, bracket,
                            );
                        }
                        None => {
                            for a in args {
                                self.expr(&a.value, None);
                            }
                            let given = ats
                                .iter()
                                .map(|t| self.show(t))
                                .collect::<Vec<_>>()
                                .join(", ");
                            let all = sigs
                                .iter()
                                .map(|s| format!("`{}`", self.show_sig(s)))
                                .collect::<Vec<_>>()
                                .join(", ");
                            self.err(
                                "V2016",
                                e.span,
                                format!("no overload of `{}` takes ({})", name, given),
                            )
                            .help = Some(format!("the overloads are: {}", all));
                            if bracket {
                                self.failable_seen += 1;
                            }
                            return Ty::Unknown;
                        }
                    }
                }
                if let Some(sig) = self.funcs.get(name).cloned() {
                    // A user `ToString` extends the built-in one rather than replacing it.
                    if name == "ToString" && args.len() == 1 {
                        let n = self.diags.len();
                        let seen = self.failable_seen;
                        let at = self.expr(&args[0].value, None);
                        self.diags.truncate(n);
                        self.failable_seen = seen;
                        if !sig
                            .params
                            .first()
                            .map(|p| self.assignable(&at, &p.ty))
                            .unwrap_or(false)
                        {
                            return builtins::generic_call(
                                self, name, e, callee_end, args, bracket,
                            );
                        }
                    }
                    return self.apply_sig(&sig, e, callee_end, args, bracket, true);
                }
                if let Some(&cid) = self.class_ids.get(name) {
                    for a in args {
                        self.expr(&a.value, None);
                    }
                    if bracket {
                        self.failable(e.span, &format!("the cast `{}[...]`", name));
                    }
                    return Ty::Class(cid);
                }
                if let Some(sig) = builtins::fixed_sig(name) {
                    return self.apply_sig(&sig, e, callee_end, args, bracket, true);
                }
                if builtins::is_generic(name) {
                    return builtins::generic_call(self, name, e, callee_end, args, bracket);
                }
                self.unknown_name(name, callee.span);
                for a in args {
                    self.expr(&a.value, None);
                }
                if bracket {
                    // An unknown `[]` call may well be failable; do not report it.
                    self.failable_seen += 1;
                }
                Ty::Unknown
            }
            ExprKind::Member(obj, name, nsp) => {
                if let ExprKind::Ident(n) = &obj.kind {
                    if self.lookup_var(n).is_none() && self.modules.contains(n) {
                        let inner = Expr {
                            kind: ExprKind::Ident(name.clone()),
                            span: *nsp,
                        };
                        return self.call(e, &inner, args, bracket);
                    }
                }
                let ot = self.expr(obj, None);
                let own = match &ot {
                    Ty::Class(cid) => {
                        self.find_method(*cid, name).is_some()
                            || self.find_field(*cid, name).is_some()
                    }
                    Ty::Array(_) | Ty::Str => builtins::array_method(self, &ot, name).is_some(),
                    Ty::Event(_) | Ty::Task(_) | Ty::Listenable(_) => {
                        matches!(name.as_str(), "Await" | "Signal" | "Subscribe")
                    }
                    Ty::Cancelable => name == "Cancel",
                    _ => false,
                };
                if !own {
                    let found = self.ext.get(name).and_then(|sigs| {
                        sigs.iter()
                            .find(|s| {
                                s.params
                                    .first()
                                    .map(|p| {
                                        self.assignable(&ot, &p.ty) || matches!(p.ty, Ty::Param(_))
                                    })
                                    .unwrap_or(false)
                            })
                            .cloned()
                    });
                    if let Some(mut sig) = found {
                        // Bind any type variable from the receiver, then check the rest.
                        let recv = sig.params.remove(0);
                        let mut map = HashMap::new();
                        unify(&recv.ty, &ot, &mut map);
                        if !map.is_empty() {
                            for p in sig.params.iter_mut() {
                                p.ty = subst(&p.ty, &map, false);
                            }
                            sig.ret = subst(&sig.ret, &map, false);
                        }
                        return self.apply_sig(&sig, e, callee_end, args, bracket, true);
                    }
                }
                match &ot {
                    Ty::Class(cid) => {
                        if let Some((owner, access)) = self.method_access(*cid, name) {
                            self.check_access(owner, access, name, *nsp);
                        }
                        if let Some(m) = self.pick_method(*cid, name, args) {
                            return self.apply_sig(&m, e, callee_end, args, bracket, true);
                        }
                        if let Some(f) = self.find_field(*cid, name) {
                            return self.call_value(e, callee, f.ty, args, bracket);
                        }
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        if bracket {
                            self.failable_seen += 1;
                        }
                        self.no_member(*cid, name, *nsp)
                    }
                    Ty::Cancelable => {
                        if name == "Cancel" {
                            let sig = FuncSig::simple("Cancel", &[], Ty::Void, Effects::TRANSACTS);
                            return self.apply_sig(&sig, e, callee_end, args, bracket, true);
                        }
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        self.err(
                            "V1003",
                            *nsp,
                            format!("`cancelable` has no method `{}`", name),
                        )
                        .help = Some("a cancelable has `Cancel()`".into());
                        Ty::Unknown
                    }
                    Ty::Event(inner) | Ty::Task(inner) | Ty::Listenable(inner) => {
                        let is_event = !matches!(ot, Ty::Task(_));
                        if name == "Signal"
                            && matches!(ot, Ty::Listenable(_))
                            && nsp.start < self.lib_start
                        {
                            self.err(
                                "V6005",
                                *nsp,
                                "a `listenable` event cannot be signalled from outside its device",
                            )
                            .help = Some(
                                "device events are fired by the game; in a test, use the device's \
                                 `Sim...` method (for example `Button.SimInteract(Agent)`)"
                                    .into(),
                            );
                        }
                        let sig = match name.as_str() {
                            "Await" => Some(FuncSig::simple(
                                "Await",
                                &[],
                                (**inner).clone(),
                                Effects::COMPUTES.suspends(),
                            )),
                            "Signal" if is_event => Some(if **inner == Ty::Void {
                                FuncSig::simple("Signal", &[], Ty::Void, Effects::TRANSACTS)
                            } else {
                                FuncSig::simple(
                                    "Signal",
                                    &[(**inner).clone()],
                                    Ty::Void,
                                    Effects::TRANSACTS,
                                )
                            }),
                            "Subscribe" if is_event => {
                                let params: Vec<Ty> = if **inner == Ty::Void {
                                    vec![]
                                } else {
                                    vec![(**inner).clone()]
                                };
                                Some(FuncSig::simple(
                                    "Subscribe",
                                    &[Ty::Func(Box::new(FuncSig::simple(
                                        "handler",
                                        &params,
                                        Ty::Void,
                                        Effects::TRANSACTS,
                                    )))],
                                    Ty::Cancelable,
                                    Effects::TRANSACTS,
                                ))
                            }
                            _ => None,
                        };
                        match sig {
                            Some(sig) => self.apply_sig(&sig, e, callee_end, args, bracket, true),
                            None => {
                                for a in args {
                                    self.expr(&a.value, None);
                                }
                                let shown = self.show(&ot);
                                self.err(
                                    "V1003",
                                    *nsp,
                                    format!("`{}` has no method `{}`", shown, name),
                                )
                                .help = Some(if is_event {
                                    "an event has `Await()`, `Signal(Value)` and `Subscribe(Handler)`"
                                        .to_string()
                                } else {
                                    "a task has `Await()`".to_string()
                                });
                                Ty::Unknown
                            }
                        }
                    }
                    Ty::Array(_) | Ty::Str => match builtins::array_method(self, &ot, name) {
                        Some(sig) => self.apply_sig(&sig, e, callee_end, args, bracket, true),
                        None => {
                            for a in args {
                                self.expr(&a.value, None);
                            }
                            self.member(e, obj, name, *nsp);
                            Ty::Unknown
                        }
                    },
                    Ty::Unknown | Ty::Any | Ty::Never | Ty::TypeOf(_) => {
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        if bracket {
                            self.failable_seen += 1;
                        }
                        Ty::Unknown
                    }
                    _ => {
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        self.member(e, obj, name, *nsp);
                        Ty::Unknown
                    }
                }
            }
            _ => {
                let ct = self.expr(callee, None);
                self.call_value(e, callee, ct, args, bracket)
            }
        }
    }

    /// Call or index a value: a function value, an array, a map, a string
    /// or a tuple.
    fn call_value(&mut self, e: &Expr, callee: &Expr, ct: Ty, args: &[Arg], bracket: bool) -> Ty {
        let text = self
            .src
            .get(callee.span.start as usize..callee.span.end as usize)
            .unwrap_or("X")
            .to_string();
        match ct {
            Ty::Func(sig) => self.apply_sig(&sig, e, callee.span.end, args, bracket, true),
            Ty::Array(_) | Ty::Map(..) | Ty::Str if !bracket => {
                for a in args {
                    self.expr(&a.value, None);
                }
                let d = self.err(
                    "V2003",
                    e.span,
                    format!(
                        "`{}` is indexed with square brackets, not parentheses",
                        text
                    ),
                );
                d.help = Some(format!(
                    "write `{}[Index]`; indexing can fail, so use it in a failure context",
                    text
                ));
                Ty::Unknown
            }
            Ty::Array(t) => {
                self.index_arg(e, args, &Ty::Int, "array index");
                self.failable(e.span, "indexing an array (the index may be out of range)");
                *t
            }
            Ty::Str => {
                self.index_arg(e, args, &Ty::Int, "string index");
                self.failable(e.span, "indexing a string (the index may be out of range)");
                Ty::Char
            }
            Ty::Map(k, v) => {
                self.index_arg(e, args, &k, "map key");
                self.failable(e.span, "looking up a map key (the key may be missing)");
                *v
            }
            Ty::Tuple(ts) => {
                if bracket {
                    for a in args {
                        self.expr(&a.value, None);
                    }
                    self.err(
                        "V2003",
                        e.span,
                        "tuple elements are accessed with parentheses, not square brackets",
                    )
                    .help = Some(format!("write `{}(0)` for the first element", text));
                    return Ty::Unknown;
                }
                match args.first().map(|a| &a.value.kind) {
                    Some(ExprKind::Int(i)) if args.len() == 1 => {
                        if *i < 0 || *i as usize >= ts.len() {
                            self.err(
                                "V2002",
                                e.span,
                                format!(
                                    "tuple index {} is out of range; this tuple has {} element(s)",
                                    i,
                                    ts.len()
                                ),
                            );
                            Ty::Unknown
                        } else {
                            ts[*i as usize].clone()
                        }
                    }
                    _ => {
                        for a in args {
                            self.expr(&a.value, None);
                        }
                        self.err(
                            "V2002",
                            e.span,
                            "a tuple index must be an integer literal, such as `Pair(0)`",
                        );
                        Ty::Unknown
                    }
                }
            }
            Ty::Opt(_) => {
                for a in args {
                    self.expr(&a.value, None);
                }
                self.err(
                    "V2003",
                    e.span,
                    format!(
                        "`{}` is an option; unwrap it with `?` before using it",
                        text
                    ),
                );
                Ty::Unknown
            }
            Ty::Unknown | Ty::Any | Ty::Never | Ty::TypeOf(_) => {
                for a in args {
                    self.expr(&a.value, None);
                }
                if bracket {
                    self.failable_seen += 1;
                }
                Ty::Unknown
            }
            other => {
                for a in args {
                    self.expr(&a.value, None);
                }
                let shown = self.show(&other);
                self.err(
                    "V2003",
                    e.span,
                    format!("a value of type `{}` cannot be called", shown),
                );
                Ty::Unknown
            }
        }
    }

    fn index_arg(&mut self, e: &Expr, args: &[Arg], want: &Ty, what: &str) {
        if args.len() != 1 {
            for a in args {
                self.expr(&a.value, None);
            }
            self.err(
                "V2002",
                e.span,
                format!("expected exactly one {}, found {}", what, args.len()),
            );
            return;
        }
        let t = self.expr(&args[0].value, Some(want));
        self.require(&args[0].value, &t, want, what);
    }

    /// Check a call against a signature: call style, effects and arguments.
    pub(crate) fn apply_sig(
        &mut self,
        sig: &FuncSig,
        e: &Expr,
        callee_end: u32,
        args: &[Arg],
        bracket: bool,
        check_style: bool,
    ) -> Ty {
        let inner = self
            .src
            .get(callee_end as usize + 1..(e.span.end as usize).saturating_sub(1))
            .unwrap_or("")
            .to_string();
        let call_span = Span::new(callee_end, e.span.end);
        if check_style {
            if sig.effects.decides && !bracket {
                let d = self.err(
                    "V3002",
                    e.span,
                    format!(
                        "`{}` is `<decides>`, so it must be called with square brackets",
                        sig.name
                    ),
                );
                d.help = Some(format!("write `{}[{}]`; the call can fail, so use it in a failure context such as `if (...)`", sig.name, inner));
                d.fix = Some(reverse_syntax::Fix {
                    span: call_span,
                    replacement: format!("[{}]", inner),
                });
            } else if !sig.effects.decides && bracket {
                let d = self.err(
                    "V3003",
                    e.span,
                    format!(
                        "`{}` cannot fail, so it is called with parentheses",
                        sig.name
                    ),
                );
                d.help = Some(format!("write `{}({})`", sig.name, inner));
                d.fix = Some(reverse_syntax::Fix {
                    span: call_span,
                    replacement: format!("({})", inner),
                });
            }
        }
        if sig.effects.decides {
            if check_style && !bracket {
                // Already reported as V3002; one diagnostic per mistake.
                self.failable_seen += 1;
            } else {
                self.failable(e.span, &format!("calling `{}`", sig.name));
            }
        }
        if sig.effects.suspends {
            let ok = self.in_spawn
                || self
                    .cur_fn
                    .as_ref()
                    .map(|f| f.effects.suspends)
                    .unwrap_or(false);
            if !ok {
                self.err("V4001", e.span, format!("`{}` is `<suspends>` and can only be called from a `<suspends>` function", sig.name)).help =
                    Some(format!("add `<suspends>` to the enclosing function, or start it in the background with `spawn{{{}({})}}`", sig.name, inner));
            } else if self.failure_ctx {
                self.err("V4006", e.span, format!("`{}` suspends, so it cannot be called inside a failure context", sig.name)).help =
                    Some(format!("call it first and test the result afterwards: `Result := {}({})` on its own line, then `if (Result ...)`", sig.name, inner));
            }
        }
        if self.cur_fn.is_some() {
            self.need_effect(
                e.span,
                &format!("calling `{}`", sig.name),
                sig.effects.reads,
                sig.effects.writes,
                sig.effects.allocates,
            );
        }

        // A parametric function: infer its type variables from the arguments.
        let instantiated;
        let sig = if sig.params.iter().any(|p| has_param(&p.ty)) || has_param(&sig.ret) {
            let ats = self.quiet_arg_types(args);
            let mut map = HashMap::new();
            for (p, a) in sig.params.iter().filter(|p| !p.named).zip(&ats) {
                unify(&p.ty, a, &mut map);
            }
            let mut s2 = sig.clone();
            for p in s2.params.iter_mut() {
                p.ty = subst(&p.ty, &map, true);
            }
            s2.ret = subst(&s2.ret, &map, true);
            instantiated = s2;
            &instantiated
        } else {
            sig
        };
        let positional: Vec<&ParamSig> = sig.params.iter().filter(|p| !p.named).collect();
        let pos_args: Vec<&Arg> = args.iter().filter(|a| a.name.is_none()).collect();
        // One tuple argument spreads across several parameters.
        if pos_args.len() == 1 && positional.len() > 1 {
            let n = self.diags.len();
            let seen = self.failable_seen;
            let t = self.expr(&pos_args[0].value, None);
            if let Ty::Tuple(ts) = &t {
                if ts.len() == positional.len() {
                    for (pt, p) in ts.iter().zip(&positional) {
                        if !self.assignable(pt, &p.ty) {
                            let (a, b) = (self.show(&p.ty), self.show(pt));
                            self.err(
                                "V2001",
                                pos_args[0].value.span,
                                format!("argument `{}`: expected `{}`, found `{}`", p.name, a, b),
                            );
                        }
                    }
                    self.named_args(sig, args);
                    return sig.ret.clone();
                }
            }
            self.diags.truncate(n);
            self.failable_seen = seen;
        }
        let required = positional.iter().filter(|p| !p.has_default).count();
        if pos_args.len() < required || pos_args.len() > positional.len() {
            for a in &pos_args {
                self.expr(&a.value, None);
            }
            let shown = self.show_sig(sig);
            let want = if required == positional.len() {
                format!("{}", required)
            } else {
                format!("{} to {}", required, positional.len())
            };
            self.err(
                "V2002",
                e.span,
                format!(
                    "`{}` takes {} argument(s), but {} were given",
                    sig.name,
                    want,
                    pos_args.len()
                ),
            )
            .help = Some(format!("its signature is `{}`", shown));
        } else {
            for (a, p) in pos_args.iter().zip(&positional) {
                let t = self.expr(&a.value, Some(&p.ty));
                let before = self.diags.len();
                self.require(
                    &a.value,
                    &t,
                    &p.ty,
                    &format!("argument `{}` of `{}`", p.name, sig.name),
                );
                // Say why an event handler was turned down.
                if self.diags.len() > before && sig.name == "Subscribe" {
                    if let (Ty::Func(found), Ty::Func(want)) = (&t, &p.ty) {
                        let help = if found.effects.suspends {
                            "an event handler cannot be `<suspends>`; subscribe a plain function \
                             and start the slow part inside it with `spawn{...}`"
                                .to_string()
                        } else if found.effects.decides {
                            "an event handler cannot be `<decides>`; handle the failure inside it"
                                .to_string()
                        } else {
                            let wanted = match want.params.first() {
                                Some(w) => format!("one parameter of type `{}`", self.show(&w.ty)),
                                None => "no parameters".to_string(),
                            };
                            format!("this event calls its handlers with {}", wanted)
                        };
                        if let Some(d) = self.diags.last_mut() {
                            d.help = Some(help);
                        }
                    }
                }
            }
        }
        self.named_args(sig, args);
        sig.ret.clone()
    }

    /// Types of the positional arguments, computed without reporting.
    fn quiet_arg_types(&mut self, args: &[Arg]) -> Vec<Ty> {
        let n = self.diags.len();
        let seen = self.failable_seen;
        let ts = args
            .iter()
            .filter(|a| a.name.is_none())
            .map(|a| self.expr(&a.value, None))
            .collect();
        self.diags.truncate(n);
        self.failable_seen = seen;
        ts
    }

    fn method_access(&self, cid: usize, name: &str) -> Option<(usize, Access)> {
        for c in self.ancestors(cid) {
            if let Some(i) = self.classes[c].methods.iter().position(|m| m.name == name) {
                return Some((c, self.classes[c].method_access[i]));
            }
        }
        None
    }

    /// `<private>` members are visible inside their class, `<protected>`
    /// ones inside the class and its subclasses.
    fn check_access(&mut self, owner: usize, access: Access, name: &str, span: Span) {
        let ok = match access {
            Access::Public | Access::Internal => true,
            Access::Private => self.cur_class == Some(owner),
            Access::Protected => self
                .cur_class
                .map(|c| self.is_subclass(c, owner))
                .unwrap_or(false),
        };
        if !ok {
            let cname = self.classes[owner].name.clone();
            let (word, scope) = if access == Access::Private {
                ("private", "inside that class")
            } else {
                ("protected", "inside that class and its subclasses")
            };
            self.err(
                "V7005",
                span,
                format!("`{}` is {} to `{}`", name, word, cname),
            )
            .help = Some(format!(
                "it can only be used {}; add a `<public>` method to `{}` if outside code needs it",
                scope, cname
            ));
        }
    }

    fn is_unique(&self, cid: usize) -> bool {
        self.ancestors(cid)
            .iter()
            .any(|c| self.classes[*c].unique || self.classes[*c].unknown_parent)
    }

    fn named_args(&mut self, sig: &FuncSig, args: &[Arg]) {
        let mut given: Vec<&str> = Vec::new();
        for a in args {
            let Some(name) = &a.name else { continue };
            given.push(name);
            match sig.params.iter().find(|p| p.named && &p.name == name) {
                Some(p) => {
                    let t = self.expr(&a.value, Some(&p.ty));
                    self.require(&a.value, &t, &p.ty, &format!("argument `?{}`", name));
                }
                None => {
                    self.expr(&a.value, None);
                    let sug = closest(
                        name,
                        sig.params
                            .iter()
                            .filter(|p| p.named)
                            .map(|p| p.name.as_str()),
                    );
                    let d = self.err(
                        "V2002",
                        a.value.span,
                        format!("`{}` has no named parameter `?{}`", sig.name, name),
                    );
                    if let Some(s) = sug {
                        d.help = Some(format!("did you mean `?{}`?", s));
                    }
                }
            }
        }
        for p in sig.params.iter().filter(|p| p.named && !p.has_default) {
            if !given.contains(&p.name.as_str()) {
                let sp = args.last().map(|a| a.value.span).unwrap_or_default();
                self.err(
                    "V2002",
                    sp,
                    format!(
                        "`{}` requires the named argument `?{} := ...`",
                        sig.name, p.name
                    ),
                );
            }
        }
    }
}

fn last_expr(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Block(stmts) if !stmts.is_empty() => last_expr(&stmts[stmts.len() - 1]),
        _ => e,
    }
}

/// Does this loop body contain a `break` for it, or a `return`?
fn contains_exit(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Break | ExprKind::Return(_) => true,
        ExprKind::Block(s) => s.iter().any(contains_exit),
        ExprKind::If { arms, else_body } => {
            arms.iter().any(|a| contains_exit(&a.body))
                || else_body
                    .as_ref()
                    .map(|b| contains_exit(b))
                    .unwrap_or(false)
        }
        ExprKind::For { body, .. } => contains_return(body),
        ExprKind::Loop(body) => contains_return(body),
        ExprKind::Case { arms, .. } => arms.iter().any(|a| contains_exit(&a.body)),
        ExprKind::Def(d) => d.value.as_ref().map(contains_exit).unwrap_or(false),
        // A call that suspends lets the task be cancelled from outside.
        ExprKind::Call { callee, .. } => matches!(&callee.kind, ExprKind::Ident(n) if n == "Sleep"),
        _ => false,
    }
}

fn contains_return(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Return(_) => true,
        ExprKind::Block(s) => s.iter().any(contains_return),
        ExprKind::If { arms, else_body } => {
            arms.iter().any(|a| contains_return(&a.body))
                || else_body
                    .as_ref()
                    .map(|b| contains_return(b))
                    .unwrap_or(false)
        }
        ExprKind::For { body, .. } | ExprKind::Loop(body) => contains_return(body),
        ExprKind::Case { arms, .. } => arms.iter().any(|a| contains_return(&a.body)),
        _ => false,
    }
}

fn has_param(t: &Ty) -> bool {
    match t {
        Ty::Param(_) => true,
        Ty::Array(a)
        | Ty::Opt(a)
        | Ty::Event(a)
        | Ty::Listenable(a)
        | Ty::Task(a)
        | Ty::TypeOf(a) => has_param(a),
        Ty::Map(k, v) => has_param(k) || has_param(v),
        Ty::Tuple(ts) => ts.iter().any(has_param),
        _ => false,
    }
}

/// Bind type variables in `pattern` from the matching parts of `actual`.
fn unify(pattern: &Ty, actual: &Ty, map: &mut HashMap<String, Ty>) {
    match (pattern, actual) {
        (Ty::Param(n), a) => {
            if !matches!(a, Ty::Unknown | Ty::Never) {
                map.entry(n.clone()).or_insert_with(|| a.clone());
            }
        }
        (Ty::Array(p), Ty::Array(a))
        | (Ty::Opt(p), Ty::Opt(a))
        | (Ty::Event(p), Ty::Event(a))
        | (Ty::Listenable(p), Ty::Listenable(a) | Ty::Event(a))
        | (Ty::Task(p), Ty::Task(a)) => unify(p, a, map),
        (Ty::Array(p), Ty::Str) => unify(p, &Ty::Char, map),
        (Ty::Map(pk, pv), Ty::Map(ak, av)) => {
            unify(pk, ak, map);
            unify(pv, av, map);
        }
        (Ty::Tuple(ps), Ty::Tuple(as_)) if ps.len() == as_.len() => {
            for (p, a) in ps.iter().zip(as_) {
                unify(p, a, map);
            }
        }
        _ => {}
    }
}

/// Replace bound type variables. With `erase`, unbound ones become unknown.
fn subst(t: &Ty, map: &HashMap<String, Ty>, erase: bool) -> Ty {
    match t {
        Ty::Param(n) => match map.get(n) {
            Some(b) => b.clone(),
            None if erase => Ty::Unknown,
            None => t.clone(),
        },
        Ty::Array(a) => Ty::Array(Box::new(subst(a, map, erase))),
        Ty::Opt(a) => Ty::Opt(Box::new(subst(a, map, erase))),
        Ty::Event(a) => Ty::Event(Box::new(subst(a, map, erase))),
        Ty::Listenable(a) => Ty::Listenable(Box::new(subst(a, map, erase))),
        Ty::Task(a) => Ty::Task(Box::new(subst(a, map, erase))),
        Ty::Map(k, v) => Ty::Map(
            Box::new(subst(k, map, erase)),
            Box::new(subst(v, map, erase)),
        ),
        Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|x| subst(x, map, erase)).collect()),
        _ => t.clone(),
    }
}
