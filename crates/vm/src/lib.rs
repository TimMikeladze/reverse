//! Tree-walking interpreter for the checked subset of Verse.
//!
//! Three ideas carry the semantics:
//!
//! * **Failure is a value.** Evaluation returns `Result<Value, Ctrl>` and
//!   `Ctrl::Fail` travels up to the nearest failure context. Rust unwinding
//!   is never used for Verse failure.
//! * **Transactions use an undo log.** Inside a failure context every write
//!   to a `var` or mutable field records the old value. If the context
//!   fails, the log is replayed backwards; if it succeeds, the entries stay
//!   for any enclosing context.
//! * **Runs are bounded.** A step budget and a call-depth limit turn
//!   infinite loops and runaway recursion into ordinary errors, so the
//!   interpreter is safe to hand to an automated agent.

mod builtins;
mod conc;
pub mod value;

pub use conc::Session;
use reverse_syntax::ast::*;
use reverse_syntax::Span;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
pub use value::*;

#[derive(Clone, Debug)]
pub struct RuntimeError {
    /// `runtime_error`, `step_limit`, `stack_overflow`, `overflow`,
    /// `output_limit` or `unsupported`.
    pub kind: &'static str,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug)]
pub enum Ctrl {
    Fail,
    Break,
    Return(Value),
    Error(Box<RuntimeError>),
}

/// Native calls and their reversible effects. A host stages effects instead of
/// publishing them from `call`, and releases committed effects after a tick.
pub trait Host {
    /// Arguments are in source evaluation order. `Some` carries a named argument's name.
    fn call(
        &mut self,
        name: &str,
        args: &[(Option<String>, Value)],
        span: Span,
    ) -> Result<Value, Ctrl>;
    fn checkpoint(&self) -> usize;
    fn rollback(&mut self, checkpoint: usize);
}

type R = Result<Value, Ctrl>;

fn error(kind: &'static str, message: impl Into<String>, span: Span) -> Ctrl {
    Ctrl::Error(Box::new(RuntimeError {
        kind,
        message: message.into(),
        span: Some(span),
    }))
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_steps: u64,
    pub max_depth: u32,
    pub max_output_bytes: usize,
    /// Seed for `GetRandomInt` and friends; runs are reproducible.
    pub seed: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_steps: 5_000_000,
            // WebAssembly has one fixed stack and no deep-stack thread.
            max_depth: if cfg!(target_arch = "wasm32") {
                200
            } else {
                1000
            },
            max_output_bytes: 1 << 20,
            seed: 0x5EED,
        }
    }
}

struct FieldRt {
    name: String,
    default: Option<Expr>,
}

struct ClassRt {
    name: Rc<str>,
    kind: ClassKind,
    parents: Vec<usize>,
    fields: Vec<FieldRt>,
    /// Method name to (definition, class that defines it), parents included.
    methods: HashMap<String, (Rc<FuncDef>, usize)>,
    /// Every definition of each method name; more than one means overloads.
    overloaded: HashMap<String, Vec<(Rc<FuncDef>, usize)>>,
    blocks: Vec<(Expr, usize)>,
    is_abstract: bool,
}

/// Parameter types as text, so two definitions can be compared for shape.
fn type_key(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Named { name, args, .. } => {
            let a: Vec<String> = args.iter().map(type_key).collect();
            format!("{}({})", name, a.join(","))
        }
        TypeExpr::Array(i, _) => format!("[]{}", type_key(i)),
        TypeExpr::Map(k, v, _) => format!("[{}]{}", type_key(k), type_key(v)),
        TypeExpr::Option(i, _) => format!("?{}", type_key(i)),
        TypeExpr::Tuple(items, _) => {
            let a: Vec<String> = items.iter().map(type_key).collect();
            format!("tuple({})", a.join(","))
        }
        TypeExpr::Opaque(_) => "_".into(),
    }
}

fn shape_key(def: &FuncDef) -> String {
    def.params
        .iter()
        .filter(|p| !p.named)
        .map(|p| type_key(&p.ty))
        .collect::<Vec<_>>()
        .join(";")
}

fn add_overload(list: &mut Vec<(Rc<FuncDef>, usize)>, m: (Rc<FuncDef>, usize)) {
    let key = shape_key(&m.0);
    match list.iter_mut().find(|(d, _)| shape_key(d) == key) {
        // A body replaces a missing one; otherwise the later definition wins.
        Some(slot) => {
            if m.0.body.is_some() || slot.0.body.is_none() {
                *slot = m;
            }
        }
        None => list.push(m),
    }
}

impl ClassRt {
    fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }
}

/// A loaded program: definitions indexed for execution.
pub struct Program {
    classes: Vec<ClassRt>,
    class_ids: HashMap<String, usize>,
    enums: HashMap<String, Vec<String>>,
    /// The first definition of each function name.
    funcs: HashMap<String, Rc<FuncDef>>,
    /// Every definition of each name, in source order.
    overloads: HashMap<String, Vec<Rc<FuncDef>>>,
    /// Extension methods by name; the receiver is the first parameter.
    ext: HashMap<String, Vec<Rc<FuncDef>>>,
    global_defs: Vec<Def>,
    modules: HashSet<String>,
    host_functions: HashSet<String>,
    /// Functions that may suspend (directly or through what they call).
    async_funcs: HashSet<String>,
    /// Method names that may suspend in at least one class.
    async_methods: HashSet<String>,
    /// Every method name defined by any class.
    all_methods: HashSet<String>,
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

impl Program {
    pub fn load(module: &Module) -> Program {
        Self::load_with_host(module, [])
    }

    /// Link synchronous host function names while resolving suspension points.
    pub fn load_with_host(module: &Module, names: impl IntoIterator<Item = String>) -> Program {
        let mut items = Vec::new();
        let mut modules = HashSet::new();
        flatten(&module.items, &mut items, &mut modules);
        let mut class_defs: Vec<&ClassDef> = Vec::new();
        let mut class_ids = HashMap::new();
        let mut enums = HashMap::new();
        let mut funcs = HashMap::new();
        let mut overloads: HashMap<String, Vec<Rc<FuncDef>>> = HashMap::new();
        let mut ext: HashMap<String, Vec<Rc<FuncDef>>> = HashMap::new();
        let mut global_defs = Vec::new();
        for it in &items {
            match &it.kind {
                ExprKind::Class(c) => {
                    if !class_ids.contains_key(&c.name) {
                        class_ids.insert(c.name.clone(), class_defs.len());
                        class_defs.push(c);
                    }
                }
                ExprKind::Enum(e) => {
                    enums.insert(
                        e.name.clone(),
                        e.values.iter().map(|(v, _)| v.clone()).collect(),
                    );
                }
                ExprKind::Func(f) => {
                    let def = Rc::new((**f).clone());
                    if f.extension {
                        ext.entry(f.name.clone()).or_default().push(def);
                    } else {
                        funcs.entry(f.name.clone()).or_insert_with(|| def.clone());
                        overloads.entry(f.name.clone()).or_default().push(def);
                    }
                }
                ExprKind::Def(d) => global_defs.push((**d).clone()),
                _ => {}
            }
        }
        let mut done: Vec<Option<ClassRt>> = (0..class_defs.len()).map(|_| None).collect();
        let mut visiting = vec![false; class_defs.len()];
        for i in 0..class_defs.len() {
            resolve_class(i, &class_defs, &class_ids, &mut done, &mut visiting);
        }
        let mut prog = Program {
            classes: done.into_iter().map(|c| c.unwrap()).collect(),
            class_ids,
            enums,
            funcs,
            overloads,
            ext,
            global_defs,
            modules,
            host_functions: names.into_iter().collect(),
            async_funcs: HashSet::new(),
            async_methods: HashSet::new(),
            all_methods: HashSet::new(),
        };
        prog.find_suspending(&class_defs);
        prog
    }

    /// Work out which functions and methods may suspend, to a fixed point:
    /// those marked `<suspends>` and those whose bodies spawn or call them.
    fn find_suspending(&mut self, class_defs: &[&ClassDef]) {
        let mut methods: Vec<&FuncDef> = Vec::new();
        for c in class_defs {
            for m in &c.members {
                if let ExprKind::Func(f) = &m.kind {
                    self.all_methods.insert(f.name.clone());
                    methods.push(f);
                }
            }
        }
        self.async_funcs.insert("Sleep".to_string());
        let is_marked = |f: &FuncDef| f.effects.iter().any(|s| s.name == "suspends");
        loop {
            let mut changed = false;
            let funcs: Vec<Rc<FuncDef>> = self
                .overloads
                .values()
                .chain(self.ext.values())
                .flat_map(|v| v.iter().cloned())
                .collect();
            for f in &funcs {
                if !self.async_funcs.contains(&f.name)
                    && (is_marked(f)
                        || f.body
                            .as_ref()
                            .map(|b| conc::may_suspend(self, b))
                            .unwrap_or(false))
                {
                    self.async_funcs.insert(f.name.clone());
                    changed = true;
                }
            }
            for f in &methods {
                if !self.async_methods.contains(&f.name)
                    && (is_marked(f)
                        || f.body
                            .as_ref()
                            .map(|b| conc::may_suspend(self, b))
                            .unwrap_or(false))
                {
                    self.async_methods.insert(f.name.clone());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn is_subclass(&self, a: usize, b: usize) -> bool {
        a == b
            || self.classes[a]
                .parents
                .iter()
                .any(|p| self.is_subclass(*p, b))
    }
}

fn resolve_class(
    i: usize,
    defs: &[&ClassDef],
    ids: &HashMap<String, usize>,
    done: &mut Vec<Option<ClassRt>>,
    visiting: &mut Vec<bool>,
) {
    if done[i].is_some() || visiting[i] {
        return;
    }
    visiting[i] = true;
    let c = defs[i];
    let parents: Vec<usize> = c
        .parents
        .iter()
        .filter_map(|(p, _)| ids.get(p).copied())
        .filter(|p| *p != i)
        .collect();
    let mut fields: Vec<FieldRt> = Vec::new();
    let mut methods: HashMap<String, (Rc<FuncDef>, usize)> = HashMap::new();
    let mut overloaded: HashMap<String, Vec<(Rc<FuncDef>, usize)>> = HashMap::new();
    let mut blocks: Vec<(Expr, usize)> = Vec::new();
    let mut live_parents = Vec::new();
    for p in &parents {
        resolve_class(*p, defs, ids, done, visiting);
        let Some(pc) = &done[*p] else { continue };
        live_parents.push(*p);
        for f in &pc.fields {
            if !fields.iter().any(|x| x.name == f.name) {
                fields.push(FieldRt {
                    name: f.name.clone(),
                    default: f.default.clone(),
                });
            }
        }
        for (name, list) in &pc.overloaded {
            for m in list {
                add_overload(overloaded.entry(name.clone()).or_default(), m.clone());
            }
        }
        for (name, m) in &pc.methods {
            // A parent's concrete method beats an interface's abstract one.
            let replace = match methods.get(name) {
                None => true,
                Some((existing, _)) => existing.body.is_none() && m.0.body.is_some(),
            };
            if replace {
                methods.insert(name.clone(), m.clone());
            }
        }
        for b in &pc.blocks {
            blocks.push(b.clone());
        }
    }
    for m in &c.members {
        match &m.kind {
            ExprKind::Def(d) => {
                if let Some(existing) = fields.iter_mut().find(|x| x.name == d.name) {
                    existing.default = d.value.clone();
                } else {
                    fields.push(FieldRt {
                        name: d.name.clone(),
                        default: d.value.clone(),
                    });
                }
            }
            ExprKind::Func(f) => {
                let m = (Rc::new((**f).clone()), i);
                add_overload(overloaded.entry(f.name.clone()).or_default(), m.clone());
                methods.insert(f.name.clone(), m);
            }
            ExprKind::Block(_) => blocks.push((m.clone(), i)),
            _ => {}
        }
    }
    done[i] = Some(ClassRt {
        name: Rc::from(c.name.as_str()),
        kind: c.kind,
        parents: live_parents,
        fields,
        methods,
        overloaded,
        blocks,
        is_abstract: c.kind == ClassKind::Interface || c.specs.iter().any(|s| s.name == "abstract"),
    });
    visiting[i] = false;
}

#[derive(Clone)]
enum Slot {
    Val(Value),
    Var(Rc<RefCell<Value>>),
}

/// Local scope. Cloning it gives a concurrent child its own view: constants
/// are copied, `var` cells are shared.
#[derive(Clone)]
struct Env {
    vars: Vec<(String, Slot)>,
    this: Option<Value>,
    /// Class that defines the running method; `(super:)` starts above it.
    class: Option<usize>,
    /// Background tasks owned by the enclosing function call (`branch`,
    /// and `rush` arms that lost). They are cancelled when it returns.
    bg: Option<Rc<conc::BgList>>,
    /// `defer` bodies registered by blocks that are still open, each with
    /// the scope it was written in.
    defers: Vec<(Rc<Expr>, Env)>,
}

impl Env {
    fn new(this: Option<Value>, class: Option<usize>) -> Env {
        Env {
            vars: Vec::new(),
            this,
            class,
            bg: None,
            defers: Vec::new(),
        }
    }
    /// A copy for a concurrent child or a deferred body: same names, shared
    /// `var` cells, no pending defers of its own.
    fn child(&self) -> Env {
        Env {
            vars: self.vars.clone(),
            this: self.this.clone(),
            class: self.class,
            bg: self.bg.clone(),
            defers: Vec::new(),
        }
    }
    fn slot(&self, name: &str) -> Option<&Slot> {
        self.vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
    }
    fn get(&self, name: &str) -> Option<Value> {
        self.slot(name).map(|s| match s {
            Slot::Val(v) => v.clone(),
            Slot::Var(c) => c.borrow().clone(),
        })
    }
}

enum Undo {
    Cell(Rc<RefCell<Value>>, Value),
    Field(Rc<Object>, usize, Value),
    Spawned(usize),
}

struct Checkpoint {
    writes: usize,
    host: Option<usize>,
}

#[derive(Debug)]
pub enum Outcome {
    Value(Value),
    /// A `<decides>` entry point failed; its effects were rolled back.
    Failed,
    Error(RuntimeError),
}

pub struct Vm {
    prog: Rc<Program>,
    globals: HashMap<String, Rc<RefCell<Value>>>,
    /// Lines written by `Print`, in order.
    pub output: Vec<String>,
    /// Also write each `Print` line to stdout as it happens.
    pub echo: bool,
    out_bytes: usize,
    pub steps: u64,
    limits: Limits,
    depth: u32,
    log: Vec<Undo>,
    txn: u32,
    rng: u64,
    /// Simulated seconds since the run began. Only `Sleep` advances it.
    pub now: f64,
    sched: conc::Sched,
    host: Option<Box<dyn Host>>,
}

impl Vm {
    fn blank(prog: Rc<Program>, limits: Limits) -> Vm {
        let rng = limits.seed | 1;
        Vm {
            prog,
            globals: HashMap::new(),
            output: Vec::new(),
            echo: false,
            out_bytes: 0,
            steps: 0,
            limits,
            depth: 0,
            log: Vec::new(),
            txn: 0,
            rng,
            now: 0.0,
            sched: conc::Sched::default(),
            host: None,
        }
    }

    pub fn new(prog: Rc<Program>, limits: Limits) -> Result<Vm, RuntimeError> {
        Self::new_using_host(prog, limits, None)
    }

    pub fn new_with_host(
        prog: Rc<Program>,
        limits: Limits,
        host: Box<dyn Host>,
    ) -> Result<Vm, RuntimeError> {
        Self::new_using_host(prog, limits, Some(host))
    }

    fn new_using_host(
        prog: Rc<Program>,
        limits: Limits,
        host: Option<Box<dyn Host>>,
    ) -> Result<Vm, RuntimeError> {
        let mut vm = Vm::blank(prog, limits);
        vm.host = host;
        let checkpoint = vm.host_checkpoint();
        if let Err(e) = vm.init_globals() {
            vm.rollback_host(checkpoint);
            return Err(e);
        }
        Ok(vm)
    }

    fn init_globals(&mut self) -> Result<(), RuntimeError> {
        let prog = self.prog.clone();
        for d in &prog.global_defs {
            let Some(value) = &d.value else { continue };
            let mut env = Env::new(None, None);
            match self.eval(value, &mut env) {
                Ok(v) => {
                    self.globals
                        .insert(d.name.clone(), Rc::new(RefCell::new(v)));
                }
                Err(Ctrl::Error(e)) => return Err(*e),
                Err(_) => {
                    return Err(RuntimeError {
                        kind: "runtime_error",
                        message: format!("the initial value of `{}` failed", d.name),
                        span: Some(value.span),
                    })
                }
            }
        }
        Ok(())
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.prog.funcs.contains_key(name)
    }

    /// Call a top-level function. A `<decides>` function that fails yields
    /// `Outcome::Failed` and leaves no trace: its writes are rolled back.
    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Outcome {
        let Some(def) = self.prog.funcs.get(name).cloned() else {
            return Outcome::Error(RuntimeError {
                kind: "runtime_error",
                message: format!("no function named `{}`", name),
                span: None,
            });
        };
        let args = args.into_iter().map(|v| (None, v)).collect();
        conc::outcome(self.in_entry(|vm| vm.call_fn(&def, None, None, args, def.name_span)))
    }

    fn in_entry(&mut self, f: impl FnOnce(&mut Vm) -> R) -> R {
        let cp = self.begin();
        let result = f(self);
        match result {
            Ok(_) | Err(Ctrl::Return(_)) => self.commit(),
            _ => self.rollback(cp),
        }
        result
    }

    // ------------------------------------------------------------ transactions

    fn host_checkpoint(&self) -> Option<usize> {
        self.host.as_ref().map(|host| host.checkpoint())
    }

    fn rollback_host(&mut self, checkpoint: Option<usize>) {
        if let (Some(host), Some(checkpoint)) = (&mut self.host, checkpoint) {
            host.rollback(checkpoint);
        }
    }

    fn begin(&mut self) -> Checkpoint {
        self.txn += 1;
        Checkpoint {
            writes: self.log.len(),
            host: self.host_checkpoint(),
        }
    }
    fn commit(&mut self) {
        self.txn -= 1;
        if self.txn == 0 {
            self.log.clear();
        }
    }
    fn rollback(&mut self, cp: Checkpoint) {
        while self.log.len() > cp.writes {
            match self.log.pop().unwrap() {
                Undo::Cell(c, v) => *c.borrow_mut() = v,
                Undo::Field(o, i, v) => o.fields.borrow_mut()[i] = v,
                Undo::Spawned(len) => self.sched.truncate_spawned(len),
            }
        }
        self.rollback_host(cp.host);
        self.txn -= 1;
    }

    /// Run `f` as a failure context. `Ok(None)` means it failed and every
    /// write it made has been undone.
    fn attempt(&mut self, f: impl FnOnce(&mut Vm) -> R) -> Result<Option<Value>, Ctrl> {
        let cp = self.begin();
        match f(self) {
            Ok(v) => {
                self.commit();
                Ok(Some(v))
            }
            Err(Ctrl::Fail) => {
                self.rollback(cp);
                Ok(None)
            }
            Err(other) => {
                self.commit();
                Err(other)
            }
        }
    }

    fn write_cell(&mut self, cell: &Rc<RefCell<Value>>, v: Value) {
        let old = std::mem::replace(&mut *cell.borrow_mut(), v);
        if self.txn > 0 {
            self.log.push(Undo::Cell(cell.clone(), old));
        }
    }

    fn write_field(&mut self, obj: &Rc<Object>, idx: usize, v: Value) {
        let old = std::mem::replace(&mut obj.fields.borrow_mut()[idx], v);
        if self.txn > 0 {
            self.log.push(Undo::Field(obj.clone(), idx, old));
        }
    }

    pub(crate) fn print(&mut self, line: String, span: Span) -> Result<(), Ctrl> {
        self.out_bytes += line.len() + 1;
        if self.out_bytes > self.limits.max_output_bytes {
            return Err(error(
                "output_limit",
                format!("output exceeded {} bytes", self.limits.max_output_bytes),
                span,
            ));
        }
        if self.echo {
            println!("{}", line);
        }
        self.output.push(line);
        Ok(())
    }

    pub(crate) fn next_random(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    // ------------------------------------------------------------ evaluation

    fn text_of(&mut self, v: &Value, span: Span) -> Result<String, Ctrl> {
        match v {
            Value::Int(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Char(_)
            | Value::Rational(..) => Ok(v.display()),
            _ => {
                // A user-defined ToString makes other types printable.
                let arg = vec![(None, v.clone())];
                if let Some(def) = self.pick_overload("ToString", &arg) {
                    let r = self.call_fn(&def, None, None, arg, span)?;
                    return Ok(r.display());
                }
                Ok(v.display())
            }
        }
    }

    fn lookup(&mut self, name: &str, env: &Env, span: Span) -> R {
        if let Some(v) = env.get(name) {
            return Ok(v);
        }
        if name == "Self" {
            if let Some(t) = &env.this {
                return Ok(t.clone());
            }
        }
        if let Some(Value::Object(o)) = &env.this {
            let cls = &self.prog.classes[o.class];
            if let Some(idx) = cls.field_index(name) {
                return Ok(o.fields.borrow()[idx].clone());
            }
            if let Some((def, c)) = cls.methods.get(name) {
                return Ok(Value::Func(Rc::new(FuncVal::User {
                    def: def.clone(),
                    this: env.this.clone(),
                    class: Some(*c),
                })));
            }
        }
        if let Some(c) = self.globals.get(name) {
            return Ok(c.borrow().clone());
        }
        if let Some(def) = self.prog.funcs.get(name) {
            return Ok(Value::Func(Rc::new(FuncVal::User {
                def: def.clone(),
                this: None,
                class: None,
            })));
        }
        if self.prog.class_ids.contains_key(name)
            || self.prog.enums.contains_key(name)
            || self.prog.modules.contains(name)
        {
            return Ok(Value::Type(Rc::from(name)));
        }
        if builtins::is_builtin(name) || self.prog.host_functions.contains(name) {
            return Ok(Value::Func(Rc::new(FuncVal::Builtin(name.to_string()))));
        }
        Err(error(
            "runtime_error",
            format!("unknown name `{}`", name),
            span,
        ))
    }

    fn eval(&mut self, e: &Expr, env: &mut Env) -> R {
        self.steps += 1;
        if self.steps > self.limits.max_steps {
            return Err(error(
                "step_limit",
                format!(
                    "stopped after {} steps; the program probably contains an infinite loop",
                    self.limits.max_steps
                ),
                e.span,
            ));
        }
        match &e.kind {
            ExprKind::Int(v) => Ok(Value::Int(*v)),
            ExprKind::Float(v) => Ok(Value::Float(*v)),
            ExprKind::Char(c) => Ok(Value::Char(*c)),
            ExprKind::Logic(b) => Ok(Value::Logic(*b)),
            ExprKind::Unit => Ok(Value::Void),
            ExprKind::Str(segs) => {
                let mut s = String::new();
                for seg in segs {
                    match seg {
                        StrSeg::Lit(l) => s.push_str(l),
                        StrSeg::Expr(x) => {
                            let v = self.eval(x, env)?;
                            s.push_str(&self.text_of(&v, x.span)?);
                        }
                    }
                }
                Ok(Value::Str(Rc::from(s)))
            }
            ExprKind::Path(p) => Ok(Value::Type(Rc::from(p.as_str()))),
            ExprKind::Ident(name) => self.lookup(name, env, e.span),
            ExprKind::Tuple(items) => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    vs.push(self.eval(it, env)?);
                }
                Ok(Value::Tuple(Rc::new(vs)))
            }
            ExprKind::Block(stmts) => {
                let mark = env.vars.len();
                let dmark = env.defers.len();
                let mut result = Ok(Value::Void);
                for s in stmts {
                    result = self.eval(s, env);
                    if result.is_err() {
                        break;
                    }
                }
                let result = self.run_defers(env, dmark, result);
                env.vars.truncate(mark);
                result
            }
            ExprKind::Def(d) => {
                let v = match &d.value {
                    Some(x) => self.eval(x, env)?,
                    None => Value::Void,
                };
                let slot = if d.is_var {
                    Slot::Var(Rc::new(RefCell::new(v.clone())))
                } else {
                    Slot::Val(v.clone())
                };
                env.vars.push((d.name.clone(), slot));
                Ok(v)
            }
            ExprKind::Set { target, op, value } => {
                let rhs = self.eval(value, env)?;
                let new = match op {
                    None => rhs,
                    Some(op) => {
                        let old = self.eval(target, env)?;
                        self.arith(*op, old, rhs, e.span)?
                    }
                };
                self.write_place(target, new.clone(), env)?;
                Ok(new)
            }
            ExprKind::Unary(op, inner) => match op {
                UnOp::Not => {
                    // `not X` succeeds when X fails, and discards X's effects
                    // and bindings whether or not X succeeds.
                    let mark = env.vars.len();
                    let cp = self.begin();
                    let r = self.eval(inner, env);
                    env.vars.truncate(mark);
                    match r {
                        Ok(_) => {
                            self.rollback(cp);
                            Err(Ctrl::Fail)
                        }
                        Err(Ctrl::Fail) => {
                            self.rollback(cp);
                            Ok(Value::Void)
                        }
                        Err(c) => {
                            self.commit();
                            Err(c)
                        }
                    }
                }
                UnOp::Pos => self.eval(inner, env),
                UnOp::Neg => match self.eval(inner, env)? {
                    Value::Int(i) => i
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| error("overflow", "integer overflow", e.span)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    Value::Rational(n, d) => n
                        .checked_neg()
                        .map(|n| Value::rational(n, d))
                        .ok_or_else(|| error("overflow", "integer overflow", e.span)),
                    other => Err(error(
                        "runtime_error",
                        format!("cannot negate a {}", other.type_name()),
                        e.span,
                    )),
                },
            },
            ExprKind::Binary(op, l, r) => match op {
                BinOp::And => {
                    self.eval(l, env)?;
                    self.eval(r, env)
                }
                BinOp::Or => match self.attempt(|vm| vm.eval(l, env))? {
                    Some(v) => Ok(v),
                    None => self.eval(r, env),
                },
                _ if op.is_comparison() => {
                    let a = self.eval(l, env)?;
                    let b = self.eval(r, env)?;
                    if self.compare(*op, &a, &b, e.span)? {
                        Ok(a)
                    } else {
                        Err(Ctrl::Fail)
                    }
                }
                _ => {
                    let a = self.eval(l, env)?;
                    let b = self.eval(r, env)?;
                    self.arith(*op, a, b, e.span)
                }
            },
            ExprKind::Range(..) => Err(error(
                "runtime_error",
                "a range can only be iterated by `for`",
                e.span,
            )),
            ExprKind::Query(inner) => match self.eval(inner, env)? {
                Value::Opt(Some(v)) => Ok((*v).clone()),
                Value::Opt(None) | Value::Logic(false) | Value::Void => Err(Ctrl::Fail),
                other => Ok(other),
            },
            ExprKind::Call {
                callee,
                args,
                bracket,
            } => self.call_expr(e, callee, args, *bracket, env),
            ExprKind::Member(obj, name, _) => {
                if let ExprKind::Ident(n) = &obj.kind {
                    if env.slot(n).is_none() {
                        if let Some(vals) = self.prog.enums.get(n) {
                            if vals.iter().any(|v| v == name) {
                                return Ok(Value::Enum {
                                    ty: Rc::from(n.as_str()),
                                    value: Rc::from(name.as_str()),
                                });
                            }
                            return Err(error(
                                "runtime_error",
                                format!("enum `{}` has no value `{}`", n, name),
                                e.span,
                            ));
                        }
                        if self.prog.modules.contains(n) {
                            return self.lookup(name, env, e.span);
                        }
                    }
                }
                let v = self.eval(obj, env)?;
                self.member(&v, name, e.span)
            }
            ExprKind::SuperCall { method, args } => {
                let Some(cls) = env.class else {
                    return Err(error(
                        "runtime_error",
                        "`(super:)` used outside a method",
                        e.span,
                    ));
                };
                let prog = self.prog.clone();
                let found = prog.classes[cls]
                    .parents
                    .iter()
                    .find_map(|p| prog.classes[*p].methods.get(method).cloned());
                let Some((def, owner)) = found else {
                    return Err(error(
                        "runtime_error",
                        format!("no parent method `{}`", method),
                        e.span,
                    ));
                };
                let a = self.eval_args(args, env)?;
                self.call_fn(&def, env.this.clone(), Some(owner), a, e.span)
            }
            ExprKind::If { arms, else_body } => {
                for arm in arms {
                    let mark = env.vars.len();
                    let ok = self.attempt(|vm| {
                        for c in &arm.cond {
                            vm.eval(c, env)?;
                        }
                        Ok(Value::Void)
                    })?;
                    if ok.is_some() {
                        let r = self.eval(&arm.body, env);
                        env.vars.truncate(mark);
                        return r;
                    }
                    env.vars.truncate(mark);
                }
                match else_body {
                    Some(b) => self.eval(b, env),
                    None => Ok(Value::Void),
                }
            }
            ExprKind::For { clauses, body } => {
                let mut out = Vec::new();
                let mark = env.vars.len();
                let r = self.for_rec(clauses, 0, body, env, &mut out);
                env.vars.truncate(mark);
                r?;
                Ok(Value::Array(Rc::new(out)))
            }
            ExprKind::Loop(body) => loop {
                let mark = env.vars.len();
                let r = self.eval(body, env);
                env.vars.truncate(mark);
                match r {
                    Ok(_) => {}
                    Err(Ctrl::Break) => return Ok(Value::Void),
                    Err(c) => return Err(c),
                }
            },
            ExprKind::Break => Err(Ctrl::Break),
            ExprKind::Return(v) => {
                let val = match v {
                    Some(x) => self.eval(x, env)?,
                    None => Value::Void,
                };
                Err(Ctrl::Return(val))
            }
            ExprKind::Case { scrutinee, arms } => {
                let v = self.eval(scrutinee, env)?;
                for arm in arms {
                    match &arm.pattern {
                        None => return self.eval(&arm.body, env),
                        Some(p) => {
                            let pv = self.eval(p, env)?;
                            if pv.equals(&v) {
                                return self.eval(&arm.body, env);
                            }
                        }
                    }
                }
                Err(Ctrl::Fail)
            }
            ExprKind::ArrayLit(items) => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    vs.push(self.eval(it, env)?);
                }
                Ok(Value::Array(Rc::new(vs)))
            }
            ExprKind::MapLit(items) => {
                let mut m: Vec<(Value, Value)> = Vec::with_capacity(items.len());
                for (k, v) in items {
                    let kv = self.eval(k, env)?;
                    let vv = self.eval(v, env)?;
                    // A repeated key keeps the last value, at the last position.
                    m.retain(|(ek, _)| !ek.equals(&kv));
                    m.push((kv, vv));
                }
                Ok(Value::Map(Rc::new(m)))
            }
            ExprKind::OptionLit(inner) => match inner {
                None => Ok(Value::Opt(None)),
                Some(x) => Ok(Value::Opt(self.attempt(|vm| vm.eval(x, env))?.map(Rc::new))),
            },
            ExprKind::LogicLit(inner) => Ok(Value::Logic(
                self.attempt(|vm| vm.eval(inner, env))?.is_some(),
            )),
            ExprKind::Archetype { ty, fields } => {
                let name = match &ty.kind {
                    ExprKind::Ident(n) => n.clone(),
                    ExprKind::Member(_, n, _) => n.clone(),
                    ExprKind::Call { callee, .. } if matches!(&callee.kind, ExprKind::Ident(n) if n == "event") =>
                    {
                        return Ok(Value::Event(Rc::new(EventCell::default())));
                    }
                    _ => {
                        return Err(error(
                            "unsupported",
                            "parametric types cannot be instantiated by this toolchain yet",
                            e.span,
                        ))
                    }
                };
                let Some(&cid) = self.prog.class_ids.get(&name) else {
                    return Err(error(
                        "unsupported",
                        format!(
                            "`{}` is not defined in this file, so it cannot be created here",
                            name
                        ),
                        e.span,
                    ));
                };
                let mut given = Vec::with_capacity(fields.len());
                for (fname, _, fexpr) in fields {
                    given.push((fname.clone(), self.eval(fexpr, env)?));
                }
                self.instantiate(cid, given, e.span)
            }
            ExprKind::Func(_)
            | ExprKind::Class(_)
            | ExprKind::Enum(_)
            | ExprKind::Module { .. }
            | ExprKind::Using(_) => Ok(Value::Void),
            ExprKind::Concurrent { kind, .. } => Err(error(
                "runtime_error",
                format!("`{}` cannot run inside a failure context", kind.as_str()),
                e.span,
            )),
            ExprKind::Spawn(body) => conc::spawn_deferred(self, body, env, e.span),
            ExprKind::Defer(body) => {
                let body = self.body_rc(body);
                let scope = env.child();
                env.defers.push((body, scope));
                Ok(Value::Void)
            }
            ExprKind::Error => Err(error(
                "runtime_error",
                "this part of the program did not parse",
                e.span,
            )),
        }
    }

    fn member(&mut self, v: &Value, name: &str, span: Span) -> R {
        match v {
            Value::Object(o) => {
                let cls = &self.prog.classes[o.class];
                if let Some(idx) = cls.field_index(name) {
                    return Ok(o.fields.borrow()[idx].clone());
                }
                if let Some((def, c)) = cls.methods.get(name) {
                    return Ok(Value::Func(Rc::new(FuncVal::User {
                        def: def.clone(),
                        this: Some(v.clone()),
                        class: Some(*c),
                    })));
                }
                Err(error(
                    "runtime_error",
                    format!("`{}` has no member `{}`", o.class_name, name),
                    span,
                ))
            }
            Value::Struct(s) => match self.prog.classes[s.class].field_index(name) {
                Some(idx) => Ok(s.fields[idx].clone()),
                None => Err(error(
                    "runtime_error",
                    format!("`{}` has no field `{}`", s.class_name, name),
                    span,
                )),
            },
            Value::Array(a) if name == "Length" => Ok(Value::Int(a.len() as i64)),
            // The Book: a string's length counts UTF-8 code units.
            Value::Str(s) if name == "Length" => Ok(Value::Int(s.len() as i64)),
            Value::Map(m) if name == "Length" => Ok(Value::Int(m.len() as i64)),
            other => Err(error(
                "runtime_error",
                format!("a {} has no member `{}`", other.type_name(), name),
                span,
            )),
        }
    }

    fn for_rec(
        &mut self,
        clauses: &[ForClause],
        idx: usize,
        body: &Expr,
        env: &mut Env,
        out: &mut Vec<Value>,
    ) -> Result<(), Ctrl> {
        if idx == clauses.len() {
            let mark = env.vars.len();
            let v = self.eval(body, env);
            env.vars.truncate(mark);
            out.push(v?);
            return Ok(());
        }
        match &clauses[idx] {
            ForClause::Gen {
                key, name, iter, ..
            } => {
                if let ExprKind::Range(lo, hi) = &iter.kind {
                    let (Value::Int(a), Value::Int(b)) = (self.eval(lo, env)?, self.eval(hi, env)?)
                    else {
                        return Err(error(
                            "runtime_error",
                            "range bounds must be ints",
                            iter.span,
                        ));
                    };
                    let mut i = a;
                    while i <= b {
                        let mark = env.vars.len();
                        env.vars.push((name.clone(), Slot::Val(Value::Int(i))));
                        let r = self.for_rec(clauses, idx + 1, body, env, out);
                        env.vars.truncate(mark);
                        r?;
                        if i == i64::MAX {
                            break;
                        }
                        i += 1;
                    }
                    return Ok(());
                }
                // A failing iterable means the loop runs zero times.
                let Some(source) = self.attempt(|vm| vm.eval(iter, env))? else {
                    return Ok(());
                };
                let items: Vec<(Value, Value)> = match &source {
                    Value::Array(a) => a
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (Value::Int(i as i64), v.clone()))
                        .collect(),
                    Value::Map(m) => m.iter().cloned().collect(),
                    Value::Str(s) => s
                        .chars()
                        .enumerate()
                        .map(|(i, c)| (Value::Int(i as i64), Value::Char(c)))
                        .collect(),
                    other => {
                        return Err(error(
                            "runtime_error",
                            format!("`for` cannot iterate a {}", other.type_name()),
                            iter.span,
                        ))
                    }
                };
                for (k, v) in items {
                    let mark = env.vars.len();
                    if let Some((kn, _)) = key {
                        env.vars.push((kn.clone(), Slot::Val(k)));
                    }
                    env.vars.push((name.clone(), Slot::Val(v)));
                    let r = self.for_rec(clauses, idx + 1, body, env, out);
                    env.vars.truncate(mark);
                    r?;
                }
                Ok(())
            }
            ForClause::Expr(x) => {
                // A filter or definition; failing skips this iteration.
                match self.attempt(|vm| vm.eval(x, env))? {
                    None => Ok(()),
                    Some(_) => self.for_rec(clauses, idx + 1, body, env, out),
                }
            }
        }
    }

    fn compare(&mut self, op: BinOp, a: &Value, b: &Value, span: Span) -> Result<bool, Ctrl> {
        use std::cmp::Ordering;
        match op {
            BinOp::Eq => return Ok(a.equals(b)),
            BinOp::Ne => return Ok(!a.equals(b)),
            _ => {}
        }
        let ord: Option<Ordering> = match (a, b) {
            (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
            (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
            (Value::Rational(an, ad), Value::Rational(bn, bd)) => {
                Some((*an as i128 * *bd as i128).cmp(&(*bn as i128 * *ad as i128)))
            }
            (Value::Int(x), Value::Rational(n, d)) => {
                Some((*x as i128 * *d as i128).cmp(&(*n as i128)))
            }
            (Value::Rational(n, d), Value::Int(y)) => {
                Some((*n as i128).cmp(&(*y as i128 * *d as i128)))
            }
            _ => {
                return Err(error(
                    "runtime_error",
                    format!("cannot order a {} and a {}", a.type_name(), b.type_name()),
                    span,
                ))
            }
        };
        Ok(match (op, ord) {
            (_, None) => false,
            (BinOp::Lt, Some(o)) => o == Ordering::Less,
            (BinOp::Le, Some(o)) => o != Ordering::Greater,
            (BinOp::Gt, Some(o)) => o == Ordering::Greater,
            (BinOp::Ge, Some(o)) => o != Ordering::Less,
            _ => false,
        })
    }

    fn arith(&mut self, op: BinOp, a: Value, b: Value, span: Span) -> R {
        use Value::*;
        let overflow = || {
            error(
                "overflow",
                "integer overflow: the result does not fit in 64 bits",
                span,
            )
        };
        match (op, &a, &b) {
            (BinOp::Add, Int(x), Int(y)) => x.checked_add(*y).map(Int).ok_or_else(overflow),
            (BinOp::Sub, Int(x), Int(y)) => x.checked_sub(*y).map(Int).ok_or_else(overflow),
            (BinOp::Mul, Int(x), Int(y)) => x.checked_mul(*y).map(Int).ok_or_else(overflow),
            (BinOp::Div, Int(x), Int(y)) => {
                if *y == 0 {
                    Err(Ctrl::Fail)
                } else if *x == i64::MIN && *y < 0 {
                    Err(overflow())
                } else {
                    Ok(Value::rational(*x, *y))
                }
            }
            (
                BinOp::Add | BinOp::Sub | BinOp::Mul,
                Rational(..) | Int(_),
                Rational(..) | Int(_),
            ) => {
                let parts = |v: &Value| match v {
                    Int(i) => (*i as i128, 1i128),
                    Rational(n, d) => (*n as i128, *d as i128),
                    _ => (0, 1),
                };
                let ((an, ad), (bn, bd)) = (parts(&a), parts(&b));
                let (n, d) = match op {
                    BinOp::Add => (
                        an.checked_mul(bd)
                            .zip(bn.checked_mul(ad))
                            .and_then(|(x, y)| x.checked_add(y)),
                        ad.checked_mul(bd),
                    ),
                    BinOp::Sub => (
                        an.checked_mul(bd)
                            .zip(bn.checked_mul(ad))
                            .and_then(|(x, y)| x.checked_sub(y)),
                        ad.checked_mul(bd),
                    ),
                    _ => (an.checked_mul(bn), ad.checked_mul(bd)),
                };
                let (Some(n), Some(d)) = (n, d) else {
                    return Err(overflow());
                };
                // Reduce in 128 bits, then require the result to fit 64.
                let g = {
                    let (mut x, mut y) = (n.unsigned_abs(), d.unsigned_abs());
                    while y != 0 {
                        (x, y) = (y, x % y);
                    }
                    x.max(1) as i128
                };
                match (i64::try_from(n / g), i64::try_from(d / g)) {
                    (Ok(n), Ok(d)) => Ok(Value::rational(n, d)),
                    _ => Err(overflow()),
                }
            }
            (BinOp::Add, Float(x), Float(y)) => Ok(Float(x + y)),
            (BinOp::Sub, Float(x), Float(y)) => Ok(Float(x - y)),
            (BinOp::Mul, Float(x), Float(y)) => Ok(Float(x * y)),
            (BinOp::Div, Float(x), Float(y)) => Ok(Float(x / y)),
            (BinOp::Mul, Int(x), Float(y)) => Ok(Float(*x as f64 * y)),
            (BinOp::Mul, Float(x), Int(y)) => Ok(Float(x * *y as f64)),
            (BinOp::Add, Str(x), Str(y)) => {
                let mut s = String::with_capacity(x.len() + y.len());
                s.push_str(x);
                s.push_str(y);
                Ok(Str(Rc::from(s)))
            }
            (BinOp::Add, Array(x), Array(y)) | (BinOp::Add, Array(x), Tuple(y)) => {
                let mut v = Vec::with_capacity(x.len() + y.len());
                v.extend(x.iter().cloned());
                v.extend(y.iter().cloned());
                Ok(Array(Rc::new(v)))
            }
            (BinOp::Add, Str(x), Array(y)) => {
                let mut s = x.to_string();
                for c in y.iter() {
                    s.push_str(&c.display());
                }
                Ok(Str(Rc::from(s)))
            }
            _ => Err(error(
                "runtime_error",
                format!(
                    "`{}` is not defined for a {} and a {}",
                    op.symbol(),
                    a.type_name(),
                    b.type_name()
                ),
                span,
            )),
        }
    }

    /// Store `new` into an assignment target. Updating an element of a
    /// value type rebuilds the value and stores it one level up.
    fn write_place(&mut self, target: &Expr, new: Value, env: &mut Env) -> Result<(), Ctrl> {
        match &target.kind {
            ExprKind::Ident(name) => {
                if let Some(Slot::Var(cell)) = env.slot(name) {
                    let cell = cell.clone();
                    self.write_cell(&cell, new);
                    return Ok(());
                }
                if env.slot(name).is_none() {
                    if let Some(Value::Object(o)) = &env.this {
                        if let Some(idx) = self.prog.classes[o.class].field_index(name) {
                            let o = o.clone();
                            self.write_field(&o, idx, new);
                            return Ok(());
                        }
                    }
                    if let Some(cell) = self.globals.get(name).cloned() {
                        self.write_cell(&cell, new);
                        return Ok(());
                    }
                }
                Err(error(
                    "runtime_error",
                    format!("`{}` cannot be assigned", name),
                    target.span,
                ))
            }
            ExprKind::Member(base, name, _) => match self.eval(base, env)? {
                Value::Object(o) => match self.prog.classes[o.class].field_index(name) {
                    Some(idx) => {
                        self.write_field(&o, idx, new);
                        Ok(())
                    }
                    None => Err(error(
                        "runtime_error",
                        format!("`{}` has no field `{}`", o.class_name, name),
                        target.span,
                    )),
                },
                Value::Struct(s) => match self.prog.classes[s.class].field_index(name) {
                    Some(idx) => {
                        let mut copy = (*s).clone();
                        copy.fields[idx] = new;
                        self.write_place(base, Value::Struct(Rc::new(copy)), env)
                    }
                    None => Err(error(
                        "runtime_error",
                        format!("`{}` has no field `{}`", s.class_name, name),
                        target.span,
                    )),
                },
                other => Err(error(
                    "runtime_error",
                    format!("cannot set a field on a {}", other.type_name()),
                    target.span,
                )),
            },
            ExprKind::Call {
                callee,
                args,
                bracket: true,
            } if args.len() == 1 => {
                let container = self.eval(callee, env)?;
                let index = self.eval(&args[0].value, env)?;
                let updated = match (container, index) {
                    (Value::Array(a), Value::Int(i)) => {
                        if i < 0 || i as usize >= a.len() {
                            return Err(Ctrl::Fail);
                        }
                        let mut v = (*a).clone();
                        v[i as usize] = new;
                        Value::Array(Rc::new(v))
                    }
                    (Value::Map(m), key) => {
                        let mut v = (*m).clone();
                        match v.iter_mut().find(|(k, _)| k.equals(&key)) {
                            Some(entry) => entry.1 = new,
                            None => v.push((key, new)),
                        }
                        Value::Map(Rc::new(v))
                    }
                    (Value::Str(s), Value::Int(i)) => {
                        let mut chars: Vec<char> = s.chars().collect();
                        if i < 0 || i as usize >= chars.len() {
                            return Err(Ctrl::Fail);
                        }
                        let Value::Char(c) = new else {
                            return Err(error(
                                "runtime_error",
                                "a string element must be a char",
                                target.span,
                            ));
                        };
                        chars[i as usize] = c;
                        Value::Str(Rc::from(chars.into_iter().collect::<String>()))
                    }
                    (c, _) => {
                        return Err(error(
                            "runtime_error",
                            format!("cannot index a {}", c.type_name()),
                            target.span,
                        ))
                    }
                };
                self.write_place(callee, updated, env)
            }
            _ => Err(error(
                "runtime_error",
                "this expression cannot be assigned",
                target.span,
            )),
        }
    }

    fn instantiate(&mut self, cid: usize, mut given: Vec<(String, Value)>, span: Span) -> R {
        let prog = self.prog.clone();
        let cls = &prog.classes[cid];
        if cls.is_abstract {
            return Err(error(
                "runtime_error",
                format!("`{}` is abstract and cannot be instantiated", cls.name),
                span,
            ));
        }
        let mut fields = Vec::with_capacity(cls.fields.len());
        for f in &cls.fields {
            let v = if let Some(pos) = given.iter().position(|(n, _)| n == &f.name) {
                given.swap_remove(pos).1
            } else if let Some(d) = &f.default {
                let mut env = Env::new(None, Some(cid));
                self.eval(d, &mut env)?
            } else {
                return Err(error(
                    "runtime_error",
                    format!("`{}` needs a value for field `{}`", cls.name, f.name),
                    span,
                ));
            };
            fields.push(v);
        }
        if cls.kind == ClassKind::Struct {
            return Ok(Value::Struct(Rc::new(StructVal {
                class: cid,
                class_name: cls.name.clone(),
                fields,
            })));
        }
        let obj = Value::Object(Rc::new(Object {
            class: cid,
            class_name: cls.name.clone(),
            fields: RefCell::new(fields),
        }));
        for (block, owner) in &cls.blocks {
            let mut env = Env::new(Some(obj.clone()), Some(*owner));
            match self.eval(block, &mut env) {
                Ok(_) => {}
                Err(Ctrl::Fail) => {
                    return Err(error(
                        "runtime_error",
                        "a class `block` clause failed",
                        block.span,
                    ))
                }
                Err(c) => return Err(c),
            }
        }
        Ok(obj)
    }

    // ------------------------------------------------------------ calls

    fn eval_args(
        &mut self,
        args: &[Arg],
        env: &mut Env,
    ) -> Result<Vec<(Option<String>, Value)>, Ctrl> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push((a.name.clone(), self.eval(&a.value, env)?));
        }
        Ok(out)
    }

    fn call_expr(
        &mut self,
        e: &Expr,
        callee: &Expr,
        args: &[Arg],
        bracket: bool,
        env: &mut Env,
    ) -> R {
        match &callee.kind {
            ExprKind::Ident(name) => {
                if let Some(v) = env.get(name) {
                    let a = self.eval_args(args, env)?;
                    return self.call_value(v, a, bracket, e.span);
                }
                if let Some(Value::Object(o)) = env.this.clone() {
                    let prog = self.prog.clone();
                    let cls = &prog.classes[o.class];
                    if let Some((def, c)) = cls.methods.get(name) {
                        let a = self.eval_args(args, env)?;
                        let (def, c) = self.pick_method(&env.this, def, *c, &a);
                        return self.call_fn(&def, env.this.clone(), Some(c), a, e.span);
                    }
                    if let Some(idx) = cls.field_index(name) {
                        let v = o.fields.borrow()[idx].clone();
                        let a = self.eval_args(args, env)?;
                        return self.call_value(v, a, bracket, e.span);
                    }
                }
                if let Some(cell) = self.globals.get(name).cloned() {
                    let v = cell.borrow().clone();
                    let a = self.eval_args(args, env)?;
                    return self.call_value(v, a, bracket, e.span);
                }
                if self.prog.funcs.contains_key(name) {
                    let a = self.eval_args(args, env)?;
                    return match self.pick_overload(name, &a) {
                        Some(def) => self.call_fn(&def, None, None, a, e.span),
                        None if name == "ToString" => {
                            let vals = a.into_iter().map(|(_, v)| v).collect();
                            builtins::call(self, name, vals, bracket, e.span)
                        }
                        None => Err(error(
                            "runtime_error",
                            format!("no overload of `{}` accepts these arguments", name),
                            e.span,
                        )),
                    };
                }
                if let Some(&cid) = self.prog.class_ids.get(name) {
                    let a = self.eval_args(args, env)?;
                    let Some((_, v)) = a.into_iter().next() else {
                        return Err(error(
                            "runtime_error",
                            format!("a cast to `{}` needs a value", name),
                            e.span,
                        ));
                    };
                    if !bracket {
                        return Ok(v);
                    }
                    let ok = match &v {
                        Value::Object(o) => self.prog.is_subclass(o.class, cid),
                        Value::Struct(s) => s.class == cid,
                        _ => false,
                    };
                    return if ok { Ok(v) } else { Err(Ctrl::Fail) };
                }
                if builtins::is_builtin(name) || self.prog.host_functions.contains(name) {
                    let a = self.eval_args(args, env)?;
                    return self.call_builtin_or_host(name, a, bracket, e.span);
                }
                Err(error(
                    "runtime_error",
                    format!("unknown function `{}`", name),
                    callee.span,
                ))
            }
            ExprKind::Member(obj, name, nsp) => {
                if let ExprKind::Ident(n) = &obj.kind {
                    if env.slot(n).is_none() && self.prog.modules.contains(n) {
                        let inner = Expr {
                            kind: ExprKind::Ident(name.clone()),
                            span: *nsp,
                        };
                        return self.call_expr(e, &inner, args, bracket, env);
                    }
                }
                let recv = self.eval(obj, env)?;
                let a = self.eval_args(args, env)?;
                if let Some(def) = self.pick_extension(&recv, name, &a) {
                    let mut full = vec![(None, recv)];
                    full.extend(a);
                    return self.call_fn(&def, None, None, full, e.span);
                }
                match &recv {
                    Value::Object(o) => {
                        let prog = self.prog.clone();
                        let cls = &prog.classes[o.class];
                        if let Some((def, c)) = cls.methods.get(name) {
                            let this = Some(recv.clone());
                            let (def, c) = self.pick_method(&this, def, *c, &a);
                            return self.call_fn(&def, this, Some(c), a, e.span);
                        }
                        if let Some(idx) = cls.field_index(name) {
                            let v = o.fields.borrow()[idx].clone();
                            return self.call_value(v, a, bracket, e.span);
                        }
                        Err(error(
                            "runtime_error",
                            format!("`{}` has no method `{}`", o.class_name, name),
                            e.span,
                        ))
                    }
                    Value::Array(_) | Value::Str(_) => {
                        let vals = a.into_iter().map(|(_, v)| v).collect();
                        builtins::method(self, recv, name, vals, e.span)
                    }
                    Value::Event(ev) if name == "Signal" => {
                        let payload = a.into_iter().next().map(|(_, v)| v).unwrap_or(Value::Void);
                        self.signal(ev, payload, e.span)?;
                        Ok(Value::Void)
                    }
                    Value::Event(ev) if name == "Subscribe" => {
                        let handler = a.into_iter().next().map(|(_, v)| v).unwrap_or(Value::Void);
                        Ok(self.subscribe(ev, handler))
                    }
                    Value::Cancelable(c) if name == "Cancel" => {
                        c.event.subs.borrow_mut().retain(|(id, _)| *id != c.id);
                        Ok(Value::Void)
                    }
                    Value::Event(_) | Value::Task(_) if name == "Await" => Err(error(
                        "runtime_error",
                        "`Await` suspends, so it cannot run inside a failure context",
                        e.span,
                    )),
                    other => Err(error(
                        "runtime_error",
                        format!("a {} has no method `{}`", other.type_name(), name),
                        e.span,
                    )),
                }
            }
            _ => {
                let v = self.eval(callee, env)?;
                let a = self.eval_args(args, env)?;
                self.call_value(v, a, bracket, e.span)
            }
        }
    }

    fn call_value(
        &mut self,
        v: Value,
        args: Vec<(Option<String>, Value)>,
        bracket: bool,
        span: Span,
    ) -> R {
        match v {
            Value::Func(f) => match &*f {
                FuncVal::User { def, this, class } => {
                    self.call_fn(def, this.clone(), *class, args, span)
                }
                FuncVal::Builtin(name) => self.call_builtin_or_host(name, args, bracket, span),
            },
            Value::Array(a) => match args.first() {
                Some((_, Value::Int(i))) if *i >= 0 && (*i as usize) < a.len() => {
                    Ok(a[*i as usize].clone())
                }
                Some((_, Value::Int(_))) => Err(Ctrl::Fail),
                _ => Err(error(
                    "runtime_error",
                    "an array index must be an int",
                    span,
                )),
            },
            Value::Str(s) => match args.first() {
                Some((_, Value::Int(i))) => {
                    match usize::try_from(*i).ok().and_then(|i| s.chars().nth(i)) {
                        Some(c) => Ok(Value::Char(c)),
                        None => Err(Ctrl::Fail),
                    }
                }
                _ => Err(error(
                    "runtime_error",
                    "a string index must be an int",
                    span,
                )),
            },
            Value::Map(m) => match args.first() {
                Some((_, key)) => match m.iter().find(|(k, _)| k.equals(key)) {
                    Some((_, v)) => Ok(v.clone()),
                    None => Err(Ctrl::Fail),
                },
                None => Err(error("runtime_error", "a map lookup needs a key", span)),
            },
            Value::Tuple(t) => match args.first() {
                Some((_, Value::Int(i))) if *i >= 0 && (*i as usize) < t.len() => {
                    Ok(t[*i as usize].clone())
                }
                _ => Err(error("runtime_error", "tuple index out of range", span)),
            },
            other => Err(error(
                "runtime_error",
                format!("a {} cannot be called", other.type_name()),
                span,
            )),
        }
    }

    fn call_builtin_or_host(
        &mut self,
        name: &str,
        args: Vec<(Option<String>, Value)>,
        bracket: bool,
        span: Span,
    ) -> R {
        if self.prog.host_functions.contains(name) && !builtins::is_builtin(name) {
            return match &mut self.host {
                Some(host) => host.call(name, &args, span),
                None => Err(error(
                    "runtime_error",
                    format!("host function `{name}` is unavailable"),
                    span,
                )),
            };
        }
        builtins::call(
            self,
            name,
            args.into_iter().map(|(_, value)| value).collect(),
            bracket,
            span,
        )
    }

    /// Leaving a block runs the `defer` bodies it registered, last first.
    /// They are skipped when the block failed, because a failed scope is
    /// rolled back as if it never ran.
    fn run_defers(&mut self, env: &mut Env, mark: usize, result: R) -> R {
        if env.defers.len() <= mark {
            return result;
        }
        let pending: Vec<(Rc<Expr>, Env)> = env.defers.drain(mark..).collect();
        if matches!(result, Err(Ctrl::Fail) | Err(Ctrl::Error(_))) {
            return result;
        }
        for (body, mut scope) in pending.into_iter().rev() {
            if let Err(Ctrl::Error(e)) = self.eval(&body, &mut scope) {
                return Err(Ctrl::Error(e));
            }
        }
        result
    }

    /// Does a runtime value fit a declared parameter type? Used to choose
    /// between overloads; anything not modelled counts as fitting.
    fn value_fits(&self, ty: &TypeExpr, v: &Value) -> bool {
        match ty {
            TypeExpr::Named { name, args, .. } => match name.as_str() {
                "int" => matches!(v, Value::Int(_)),
                "float" => matches!(v, Value::Float(_)),
                "rational" => matches!(v, Value::Int(_) | Value::Rational(..)),
                "logic" => matches!(v, Value::Logic(_)),
                "char" | "char32" => matches!(v, Value::Char(_)),
                "string" => match v {
                    Value::Str(_) => true,
                    Value::Array(a) => a.iter().all(|c| matches!(c, Value::Char(_))),
                    _ => false,
                },
                "event" => matches!(v, Value::Event(_)),
                "task" => matches!(v, Value::Task(_)),
                _ if !args.is_empty() => true,
                n => {
                    if let Some(&cid) = self.prog.class_ids.get(n) {
                        match v {
                            Value::Object(o) => self.prog.is_subclass(o.class, cid),
                            Value::Struct(s) => s.class == cid,
                            _ => false,
                        }
                    } else if self.prog.enums.contains_key(n) {
                        matches!(v, Value::Enum { ty, .. } if &**ty == n)
                    } else {
                        true
                    }
                }
            },
            TypeExpr::Array(inner, _) => match v {
                Value::Array(a) => a.first().map(|x| self.value_fits(inner, x)).unwrap_or(true),
                Value::Str(_) => matches!(&**inner, TypeExpr::Named { name, .. } if name == "char"),
                _ => false,
            },
            TypeExpr::Map(k, val, _) => match v {
                Value::Map(m) => m
                    .first()
                    .map(|(a, b)| self.value_fits(k, a) && self.value_fits(val, b))
                    .unwrap_or(true),
                _ => false,
            },
            TypeExpr::Option(inner, _) => match v {
                Value::Opt(Some(x)) => self.value_fits(inner, x),
                Value::Opt(None) | Value::Logic(false) => true,
                _ => false,
            },
            TypeExpr::Tuple(items, _) => match v {
                Value::Tuple(t) => {
                    t.len() == items.len()
                        && t.iter().zip(items).all(|(x, i)| self.value_fits(i, x))
                }
                _ => false,
            },
            TypeExpr::Opaque(_) => true,
        }
    }

    fn args_fit(&self, def: &FuncDef, positional: &[&Value]) -> bool {
        let params: Vec<&Param> = def.params.iter().filter(|p| !p.named).collect();
        let required = params.iter().filter(|p| p.default.is_none()).count();
        positional.len() >= required
            && positional.len() <= params.len()
            && positional
                .iter()
                .zip(&params)
                .all(|(v, p)| self.value_fits(&p.ty, v))
    }

    /// For an overloaded method, the definition whose parameters fit the
    /// arguments; otherwise the definition already found.
    fn pick_method(
        &self,
        this: &Option<Value>,
        def: &Rc<FuncDef>,
        class: usize,
        args: &[(Option<String>, Value)],
    ) -> (Rc<FuncDef>, usize) {
        if let Some(Value::Object(o)) = this {
            if let Some(list) = self.prog.classes[o.class].overloaded.get(&def.name) {
                if list.len() > 1 {
                    let positional: Vec<&Value> = args
                        .iter()
                        .filter(|(n, _)| n.is_none())
                        .map(|(_, v)| v)
                        .collect();
                    if let Some((d, c)) = list.iter().find(|(d, _)| self.args_fit(d, &positional)) {
                        return (d.clone(), *c);
                    }
                }
            }
        }
        (def.clone(), class)
    }

    /// Choose the definition of `name` whose parameters fit the arguments.
    fn pick_overload(&self, name: &str, args: &[(Option<String>, Value)]) -> Option<Rc<FuncDef>> {
        let defs = self.prog.overloads.get(name)?;
        if defs.len() == 1 && name != "ToString" {
            return Some(defs[0].clone());
        }
        let positional: Vec<&Value> = args
            .iter()
            .filter(|(n, _)| n.is_none())
            .map(|(_, v)| v)
            .collect();
        // One tuple argument can spread across the parameters.
        let spread: Option<Vec<&Value>> = match positional.as_slice() {
            [Value::Tuple(t)] => Some(t.iter().collect()),
            _ => None,
        };
        defs.iter()
            .find(|d| {
                self.args_fit(d, &positional)
                    || spread
                        .as_ref()
                        .map(|s| self.args_fit(d, s))
                        .unwrap_or(false)
            })
            .cloned()
    }

    /// An extension method `(Receiver : type).Name(...)` that applies to this
    /// receiver, unless the receiver has a member of that name itself.
    fn pick_extension(
        &self,
        recv: &Value,
        name: &str,
        args: &[(Option<String>, Value)],
    ) -> Option<Rc<FuncDef>> {
        let defs = self.prog.ext.get(name)?;
        if let Value::Object(o) = recv {
            let cls = &self.prog.classes[o.class];
            if cls.methods.contains_key(name) || cls.field_index(name).is_some() {
                return None;
            }
        }
        let mut positional: Vec<&Value> = vec![recv];
        positional.extend(args.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| v));
        defs.iter().find(|d| self.args_fit(d, &positional)).cloned()
    }

    fn negate(&mut self, v: Value, span: Span) -> R {
        match v {
            Value::Int(i) => i
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| error("overflow", "integer overflow", span)),
            Value::Float(f) => Ok(Value::Float(-f)),
            Value::Rational(n, d) => n
                .checked_neg()
                .map(|n| Value::rational(n, d))
                .ok_or_else(|| error("overflow", "integer overflow", span)),
            other => Err(error(
                "runtime_error",
                format!("cannot negate a {}", other.type_name()),
                span,
            )),
        }
    }

    /// `Type(Value)` (an upcast, always succeeds) or `Type[Value]` (fails
    /// unless the value really is of that class).
    fn cast(
        &mut self,
        cid: usize,
        args: Vec<(Option<String>, Value)>,
        bracket: bool,
        span: Span,
    ) -> R {
        let Some((_, v)) = args.into_iter().next() else {
            return Err(error("runtime_error", "a cast needs a value", span));
        };
        if !bracket {
            return Ok(v);
        }
        let ok = match &v {
            Value::Object(o) => self.prog.is_subclass(o.class, cid),
            Value::Struct(s) => s.class == cid,
            _ => false,
        };
        if ok {
            Ok(v)
        } else {
            Err(Ctrl::Fail)
        }
    }

    /// Build the scope for a call: positional, named and default arguments.
    fn bind_params(
        &mut self,
        def: &Rc<FuncDef>,
        this: Option<Value>,
        class: Option<usize>,
        args: Vec<(Option<String>, Value)>,
        span: Span,
    ) -> Result<Env, Ctrl> {
        let mut env = Env::new(this, class);
        let mut positional: Vec<Value> = Vec::new();
        let mut named: Vec<(String, Value)> = Vec::new();
        for (n, v) in args {
            match n {
                Some(n) => named.push((n, v)),
                None => positional.push(v),
            }
        }
        let n_pos = def.params.iter().filter(|p| !p.named).count();
        if positional.len() == 1 && n_pos > 1 {
            if let Value::Tuple(t) = &positional[0] {
                if t.len() == n_pos {
                    positional = t.to_vec();
                }
            }
        }
        let mut it = positional.into_iter();
        for p in &def.params {
            let given = if p.named {
                named
                    .iter()
                    .position(|(n, _)| n == &p.name)
                    .map(|i| named.swap_remove(i).1)
            } else {
                it.next()
            };
            let v = match given {
                Some(v) => v,
                None => match &p.default {
                    Some(d) => self.eval(d, &mut env)?,
                    None => {
                        return Err(error(
                            "runtime_error",
                            format!("`{}` is missing the argument `{}`", def.name, p.name),
                            span,
                        ))
                    }
                },
            };
            env.vars.push((p.name.clone(), Slot::Val(v)));
        }
        Ok(env)
    }

    /// Wake the longest-waiting awaiter of an event, if there is one, then
    /// call every subscribed handler in subscription order. A signal nobody
    /// is waiting for or subscribed to is discarded.
    fn signal(&mut self, ev: &Rc<EventCell>, payload: Value, span: Span) -> Result<(), Ctrl> {
        if let Some(ticket) = ev.waiting.borrow_mut().pop_front() {
            ev.delivered.borrow_mut().push((ticket, payload.clone()));
            self.sched.progress = true;
        }
        let subs: Vec<(u64, Value)> = ev.subs.borrow().clone();
        for (id, handler) in subs {
            // A handler cancelled by an earlier handler is skipped.
            if !ev.subs.borrow().iter().any(|(i, _)| *i == id) {
                continue;
            }
            let args = match &payload {
                Value::Void => vec![],
                p => vec![(None, p.clone())],
            };
            match self.call_value(handler, args, false, span) {
                Ok(_) => {}
                Err(Ctrl::Fail) => {
                    return Err(error(
                        "runtime_error",
                        "an event handler failed; handlers cannot fail",
                        span,
                    ))
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn subscribe(&mut self, ev: &Rc<EventCell>, handler: Value) -> Value {
        let id = ev.next_ticket.get();
        ev.next_ticket.set(id + 1);
        ev.subs.borrow_mut().push((id, handler));
        Value::Cancelable(Rc::new(value::CancelCell {
            event: ev.clone(),
            id,
        }))
    }

    fn call_fn(
        &mut self,
        def: &Rc<FuncDef>,
        this: Option<Value>,
        class: Option<usize>,
        args: Vec<(Option<String>, Value)>,
        span: Span,
    ) -> R {
        let Some(body) = &def.body else {
            return Err(error(
                "runtime_error",
                format!("`{}` has no body", def.name),
                span,
            ));
        };
        if self.depth >= self.limits.max_depth {
            return Err(error(
                "stack_overflow",
                format!(
                    "call depth exceeded {} while calling `{}`; the recursion probably never ends",
                    self.limits.max_depth, def.name
                ),
                span,
            ));
        }
        if self.prog.async_funcs.contains(&def.name)
            && def.effects.iter().any(|s| s.name == "suspends")
        {
            return Err(error(
                "runtime_error",
                format!(
                    "`{}` suspends, so it cannot run inside a failure context",
                    def.name
                ),
                span,
            ));
        }
        let mut env = self.bind_params(def, this, class, args, span)?;
        self.depth += 1;
        let r = self.eval(body, &mut env);
        self.depth -= 1;
        let returns_void = matches!(&def.ret, Some(TypeExpr::Named { name, .. }) if name == "void");
        match r {
            Ok(v) | Err(Ctrl::Return(v)) => Ok(if returns_void { Value::Void } else { v }),
            Err(Ctrl::Break) => Err(error("runtime_error", "`break` outside a loop", span)),
            Err(other) => Err(other),
        }
    }
}
