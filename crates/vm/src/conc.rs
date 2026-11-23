//! Structured concurrency on a deterministic, single-threaded scheduler.
//!
//! Suspending code runs as Rust futures polled by a small executor. No
//! threads and no real time are involved: `Sleep` advances a simulated clock,
//! so a program that sleeps for an hour finishes in microseconds and every
//! run is reproducible. This also works unchanged in WebAssembly.
//!
//! The mapping from Verse to futures is direct:
//!
//! * `sync`  - poll every arm; finish when all have finished.
//! * `race`  - poll arms in order; the first to finish wins and the rest are
//!   dropped, which is cancellation at their current suspension point.
//! * `rush`  - like `race`, but the losers move to the enclosing function's
//!   background list and keep running until it returns.
//! * `branch` - a background task owned by the enclosing function call.
//! * `spawn` - a detached task owned by the executor.
//!
//! Code that cannot suspend is still run by the ordinary synchronous
//! evaluator; `may_suspend` decides per expression.

use crate::builtins;
use crate::value::*;
use crate::{error, Ctrl, Env, Outcome, Program, RuntimeError, Slot, Vm, R};
use reverse_syntax::ast::*;
use reverse_syntax::Span;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

mod session;
pub use session::Session;

pub(crate) type Sh = Rc<RefCell<Vm>>;
pub(crate) type Fut<'a> = Pin<Box<dyn Future<Output = R> + 'a>>;
type TaskFut = Pin<Box<dyn Future<Output = ()>>>;
pub(crate) type BgList = RefCell<Vec<Fut<'static>>>;

/// Simulated seconds that pass when every runnable task only yielded.
const TICK: f64 = 1.0 / 30.0;

#[derive(Default)]
pub(crate) struct Sched {
    me: Weak<RefCell<Vm>>,
    round: u64,
    host_clock: Option<HostClock>,
    /// Wake times registered by sleeping tasks during the current round.
    wakes: Vec<f64>,
    /// Some task yielded with `Sleep(0.0)` this round.
    yielded: bool,
    /// Something happened this round that may unblock a waiting task.
    pub(crate) progress: bool,
    /// Detached tasks created this round, not yet owned by the executor.
    spawned: Vec<TaskFut>,
    fatal: Option<RuntimeError>,
    suspend_cache: HashMap<usize, bool>,
    /// Bodies of detached tasks, copied once per source location.
    bodies: HashMap<usize, Rc<Expr>>,
}

#[derive(Clone, Copy, Default)]
struct HostClock {
    tick: u64,
    elapsed: Duration,
}

impl Sched {
    fn sleep_round(&self) -> u64 {
        self.host_clock.map_or(self.round, |clock| clock.tick)
    }

    pub(super) fn truncate_spawned(&mut self, len: usize) {
        self.spawned.truncate(len);
    }
}

// ---------------------------------------------------------------- analysis

fn children<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match &e.kind {
        ExprKind::Str(segs) => {
            for s in segs {
                if let StrSeg::Expr(x) = s {
                    out.push(x);
                }
            }
        }
        ExprKind::Tuple(xs) | ExprKind::Block(xs) | ExprKind::ArrayLit(xs) => out.extend(xs.iter()),
        ExprKind::Def(d) => out.extend(d.value.iter()),
        ExprKind::Set { target, value, .. } => {
            out.push(target);
            out.push(value);
        }
        ExprKind::Unary(_, x)
        | ExprKind::Query(x)
        | ExprKind::Loop(x)
        | ExprKind::LogicLit(x)
        | ExprKind::Defer(x) => out.push(x),
        ExprKind::Binary(_, l, r) | ExprKind::Range(l, r) => {
            out.push(l);
            out.push(r);
        }
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Member(obj, _, _) = &callee.kind {
                out.push(obj);
            } else if !matches!(callee.kind, ExprKind::Ident(_)) {
                out.push(callee);
            }
            out.extend(args.iter().map(|a| &a.value));
        }
        ExprKind::Member(obj, _, _) => out.push(obj),
        ExprKind::SuperCall { args, .. } => out.extend(args.iter().map(|a| &a.value)),
        ExprKind::If { arms, else_body } => {
            for a in arms {
                out.extend(a.cond.iter());
                out.push(&a.body);
            }
            if let Some(b) = else_body {
                out.push(b);
            }
        }
        ExprKind::For { clauses, body } => {
            for c in clauses {
                match c {
                    ForClause::Gen { iter, .. } => out.push(iter),
                    ForClause::Expr(x) => out.push(x),
                }
            }
            out.push(body);
        }
        ExprKind::Return(v) => out.extend(v.iter().map(|b| &**b)),
        ExprKind::Case { scrutinee, arms } => {
            out.push(scrutinee);
            for a in arms {
                out.extend(a.pattern.iter());
                out.push(&a.body);
            }
        }
        ExprKind::MapLit(items) => {
            for (k, v) in items {
                out.push(k);
                out.push(v);
            }
        }
        ExprKind::OptionLit(x) => out.extend(x.iter().map(|b| &**b)),
        ExprKind::Archetype { fields, .. } => out.extend(fields.iter().map(|(_, _, v)| v)),
        _ => {}
    }
}

/// Could evaluating this expression suspend? Conservative: a call whose
/// target is not known to be synchronous counts as suspending.
pub(crate) fn may_suspend(prog: &Program, e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Concurrent { .. } | ExprKind::Spawn(_) => return true,
        ExprKind::SuperCall { method, .. } if prog.async_methods.contains(method) => return true,
        ExprKind::Call {
            callee,
            args,
            bracket: false,
        } => {
            let suspends = match &callee.kind {
                ExprKind::Ident(n) => {
                    let tuple_index =
                        args.len() == 1 && matches!(args[0].value.kind, ExprKind::Int(_));
                    prog.async_funcs.contains(n)
                        || prog.async_methods.contains(n)
                        || !(prog.funcs.contains_key(n)
                            || prog.all_methods.contains(n)
                            || builtins::is_builtin(n)
                            || prog.host_functions.contains(n)
                            || prog.class_ids.contains_key(n)
                            || tuple_index)
                }
                ExprKind::Member(_, m, _) => {
                    m == "Await"
                        || prog.async_methods.contains(m)
                        || prog.async_funcs.contains(m)
                        || !(prog.all_methods.contains(m)
                            || prog.funcs.contains_key(m)
                            || prog.ext.contains_key(m)
                            || builtins::is_method(m)
                            || matches!(m.as_str(), "Signal" | "Subscribe" | "Cancel"))
                }
                _ => true,
            };
            if suspends {
                return true;
            }
        }
        _ => {}
    }
    let mut kids = Vec::new();
    children(e, &mut kids);
    kids.into_iter().any(|k| may_suspend(prog, k))
}

impl Vm {
    fn suspends(&mut self, e: &Expr) -> bool {
        let key = e as *const Expr as usize;
        if let Some(v) = self.sched.suspend_cache.get(&key) {
            return *v;
        }
        let prog = self.prog.clone();
        let v = may_suspend(&prog, e);
        self.sched.suspend_cache.insert(key, v);
        v
    }

    pub(crate) fn body_rc(&mut self, e: &Expr) -> Rc<Expr> {
        let key = e as *const Expr as usize;
        self.sched
            .bodies
            .entry(key)
            .or_insert_with(|| Rc::new(e.clone()))
            .clone()
    }

    fn step(&mut self, span: Span) -> Result<(), Ctrl> {
        self.steps += 1;
        if self.steps > self.limits.max_steps {
            return Err(error(
                "step_limit",
                format!(
                    "stopped after {} steps; the program probably contains an infinite loop",
                    self.limits.max_steps
                ),
                span,
            ));
        }
        Ok(())
    }

    /// Run a top-level function to completion, driving any tasks it starts.
    /// Simulated time passes only while every task is asleep.
    pub fn run_entry(&mut self, name: &str) -> Outcome {
        let Some(def) = self.prog.funcs.get(name).cloned() else {
            return self.call(name, Vec::new());
        };
        if !self.prog.async_funcs.contains(name) {
            return self.call(name, Vec::new());
        }
        let blank = Vm::blank(self.prog.clone(), self.limits.clone());
        let sh: Sh = Rc::new(RefCell::new(std::mem::replace(self, blank)));
        sh.borrow_mut().sched.me = Rc::downgrade(&sh);
        let outcome = execute(&sh, def);
        // Drop every task before taking the machine back.
        // Tasks are dropped with the machine free, so the `defer` bodies of
        // cancelled tasks can still run.
        let leftover = std::mem::take(&mut sh.borrow_mut().sched.spawned);
        drop(leftover);
        let mut vm = sh.borrow_mut();
        vm.sched.spawned.clear();
        vm.sched.suspend_cache.clear();
        vm.sched.bodies.clear();
        vm.sched.me = Weak::new();
        let blank = Vm::blank(vm.prog.clone(), vm.limits.clone());
        *self = std::mem::replace(&mut *vm, blank);
        outcome
    }
}

fn execute(sh: &Sh, def: Rc<FuncDef>) -> Outcome {
    let span = def.name_span;
    let mut root: Fut<'static> = {
        let sh = sh.clone();
        Box::pin(async move { acall_fn(&sh, def, None, None, Vec::new(), span).await })
    };
    let mut tasks: Vec<TaskFut> = Vec::new();
    let mut cx = Context::from_waker(Waker::noop());
    let fail = |kind: &'static str, message: String| {
        Outcome::Error(RuntimeError {
            kind,
            message,
            span: None,
        })
    };
    loop {
        {
            let mut vm = sh.borrow_mut();
            reset_round(&mut vm);
            if vm.step(span).is_err() {
                let max = vm.limits.max_steps;
                return fail(
                    "step_limit",
                    format!(
                        "stopped after {} steps; the program probably contains an infinite loop",
                        max
                    ),
                );
            }
        }
        if let Poll::Ready(r) = root.as_mut().poll(&mut cx) {
            drop(tasks);
            return outcome(r);
        }
        poll_tasks(sh, &mut tasks, &mut cx);
        let mut vm = sh.borrow_mut();
        if let Some(e) = vm.sched.fatal.take() {
            return Outcome::Error(e);
        }
        if vm.sched.progress {
            continue;
        }
        let mut next = vm.sched.wakes.iter().copied().fold(f64::INFINITY, f64::min);
        if vm.sched.yielded {
            next = next.min(vm.now + TICK);
        }
        if next.is_finite() {
            vm.now = vm.now.max(next);
        } else {
            return fail(
                "deadlock",
                "every task is waiting for an event or task that will never arrive; nothing is left to run".into(),
            );
        }
    }
}

fn reset_round(vm: &mut Vm) {
    vm.sched.round += 1;
    vm.sched.wakes.clear();
    vm.sched.yielded = false;
    vm.sched.progress = false;
}

fn poll_tasks(sh: &Sh, tasks: &mut Vec<TaskFut>, cx: &mut Context<'_>) {
    let mut i = 0;
    loop {
        tasks.append(&mut sh.borrow_mut().sched.spawned);
        if i >= tasks.len() {
            break;
        }
        if tasks[i].as_mut().poll(cx).is_ready() {
            drop(tasks.remove(i));
            sh.borrow_mut().sched.progress = true;
        } else {
            i += 1;
        }
    }
}

pub(crate) fn outcome(result: R) -> Outcome {
    match result {
        Ok(v) | Err(Ctrl::Return(v)) => Outcome::Value(v),
        Err(Ctrl::Fail) => Outcome::Failed,
        Err(Ctrl::Error(e)) => Outcome::Error(*e),
        Err(Ctrl::Break) => Outcome::Error(RuntimeError {
            kind: "runtime_error",
            message: "`break` outside a loop".into(),
            span: None,
        }),
    }
}

// ---------------------------------------------------------------- leaf futures

async fn poll_once(f: &mut Fut<'_>) -> Option<R> {
    poll_fn(|cx| {
        Poll::Ready(match f.as_mut().poll(cx) {
            Poll::Ready(r) => Some(r),
            Poll::Pending => None,
        })
    })
    .await
}

enum SleepDeadline {
    Never,
    NextRound(u64),
    Simulated(f64),
    Hosted(Duration),
}

async fn sleep(sh: &Sh, secs: f64) -> R {
    use SleepDeadline::*;
    let deadline = {
        let vm = sh.borrow();
        if secs.is_nan() || secs == f64::INFINITY {
            Never
        } else if secs <= 0.0 {
            NextRound(vm.sched.sleep_round())
        } else if let Some(clock) = vm.sched.host_clock {
            // Keep host deadlines in integer time so a decimal duration ending
            // exactly on a tick does not drift to the following tick.
            Duration::try_from_secs_f64(secs)
                .ok()
                .and_then(|delta| {
                    clock
                        .elapsed
                        .checked_add(delta.max(Duration::from_nanos(1)))
                })
                .map_or(Never, Hosted)
        } else {
            Simulated(vm.now + secs)
        }
    };
    poll_fn(|_| {
        let mut vm = sh.borrow_mut();
        match deadline {
            Never => Poll::Pending,
            NextRound(start) if vm.sched.sleep_round() <= start => {
                vm.sched.yielded = true;
                Poll::Pending
            }
            Simulated(wake) if vm.now < wake => {
                vm.sched.wakes.push(wake);
                Poll::Pending
            }
            Hosted(wake)
                if vm
                    .sched
                    .host_clock
                    .is_some_and(|clock| clock.elapsed < wake) =>
            {
                Poll::Pending
            }
            _ => Poll::Ready(Ok(Value::Void)),
        }
    })
    .await
}

/// Removes an awaiter's ticket if the awaiting task is cancelled, so a
/// later signal is not spent on a task that no longer exists.
struct Ticket(Rc<EventCell>, u64);
impl Drop for Ticket {
    fn drop(&mut self) {
        self.0.waiting.borrow_mut().retain(|t| *t != self.1);
        self.0.delivered.borrow_mut().retain(|(t, _)| *t != self.1);
    }
}

async fn await_event(ev: Rc<EventCell>) -> R {
    let ticket = ev.next_ticket.get();
    ev.next_ticket.set(ticket + 1);
    ev.waiting.borrow_mut().push_back(ticket);
    let guard = Ticket(ev.clone(), ticket);
    let r = poll_fn(|_| {
        let mut delivered = ev.delivered.borrow_mut();
        match delivered.iter().position(|(t, _)| *t == ticket) {
            Some(pos) => Poll::Ready(Ok(delivered.remove(pos).1)),
            None => Poll::Pending,
        }
    })
    .await;
    drop(guard);
    r
}

async fn await_task(task: Rc<TaskCell>) -> R {
    poll_fn(|_| match &*task.result.borrow() {
        Some(v) => Poll::Ready(Ok(v.clone())),
        None => Poll::Pending,
    })
    .await
}

/// Owns a task's scope. If the task is cancelled (its future is dropped
/// while suspended), the `defer` bodies of its open blocks still run.
struct Scope {
    sh: Sh,
    env: Env,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self.env.defers.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.env.defers);
        if let Ok(mut vm) = self.sh.try_borrow_mut() {
            for (body, mut scope) in pending.into_iter().rev() {
                let _ = vm.eval(&body, &mut scope);
            }
        }
    }
}

/// A task that owns everything it needs, so it can outlive its creator.
fn detached(sh: Sh, body: Rc<Expr>, env: Env) -> Fut<'static> {
    Box::pin(async move {
        let mut scope = Scope {
            sh: sh.clone(),
            env,
        };
        aeval(&sh, &body, &mut scope.env).await
    })
}

/// Hand a still-running background future to the executor.
fn adopt(sh: &Sh, fut: Fut<'static>) {
    let sh2 = sh.clone();
    let task: TaskFut = Box::pin(async move {
        if let Err(Ctrl::Error(e)) = fut.await {
            sh2.borrow_mut().sched.fatal.get_or_insert(*e);
        }
    });
    sh.borrow_mut().sched.spawned.push(task);
}

fn spawn_task(sh: &Sh, body: Rc<Expr>, mut env: Env) -> (Rc<TaskCell>, TaskFut) {
    let cell = Rc::new(TaskCell::default());
    env.bg = None;
    let (sh2, cell2) = (sh.clone(), cell.clone());
    let fut: TaskFut = Box::pin(async move {
        let mut scope = Scope {
            sh: sh2.clone(),
            env,
        };
        match aeval(&sh2, &body, &mut scope.env).await {
            Ok(v) | Err(Ctrl::Return(v)) => *cell2.result.borrow_mut() = Some(v),
            Err(Ctrl::Error(e)) => {
                sh2.borrow_mut().sched.fatal.get_or_insert(*e);
            }
            Err(_) => *cell2.result.borrow_mut() = Some(Value::Void),
        }
    });
    (cell, fut)
}

/// `spawn` reached from synchronous code (inside a failure context): the
/// task is queued and starts at the scheduler's next round.
pub(crate) fn spawn_deferred(vm: &mut Vm, body: &Expr, env: &mut Env, span: Span) -> R {
    let Some(sh) = vm.sched.me.upgrade() else {
        return Err(error(
            "runtime_error",
            "`spawn` cannot run while module-level values are being initialised",
            span,
        ));
    };
    if vm.txn > 0 && vm.sched.host_clock.is_some() {
        vm.log.push(crate::Undo::Spawned(vm.sched.spawned.len()));
    }
    let body = vm.body_rc(body);
    let cell = Rc::new(TaskCell::default());
    let mut env = env.child();
    env.bg = None;
    let cell2 = cell.clone();
    let task: TaskFut = Box::pin(async move {
        let mut scope = Scope {
            sh: sh.clone(),
            env,
        };
        match aeval(&sh, &body, &mut scope.env).await {
            Ok(v) | Err(Ctrl::Return(v)) => *cell2.result.borrow_mut() = Some(v),
            Err(Ctrl::Error(e)) => {
                sh.borrow_mut().sched.fatal.get_or_insert(*e);
            }
            Err(_) => *cell2.result.borrow_mut() = Some(Value::Void),
        }
    });
    vm.sched.spawned.push(task);
    Ok(Value::Task(cell))
}

// ---------------------------------------------------------------- calls

struct DepthGuard<'a>(&'a Sh);
impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut vm) = self.0.try_borrow_mut() {
            vm.depth = vm.depth.saturating_sub(1);
        }
    }
}

fn acall_fn<'a>(
    sh: &'a Sh,
    def: Rc<FuncDef>,
    this: Option<Value>,
    class: Option<usize>,
    args: Vec<(Option<String>, Value)>,
    span: Span,
) -> Fut<'a> {
    let name = def.clone();
    let mut call = Box::pin(async move {
        let Some(body) = &def.body else {
            return Err(error(
                "runtime_error",
                format!("`{}` has no body", def.name),
                span,
            ));
        };
        let env = sh.borrow_mut().bind_params(&def, this, class, args, span)?;
        let mut scope = Scope {
            sh: sh.clone(),
            env,
        };
        let bg: Rc<BgList> = Rc::new(RefCell::new(Vec::new()));
        scope.env.bg = Some(bg.clone());
        let r = {
            let mut main = aeval(sh, body, &mut scope.env);
            poll_fn(|cx| {
                if let Poll::Ready(r) = main.as_mut().poll(cx) {
                    return Poll::Ready(r);
                }
                // Drive this call's background tasks alongside its body.
                let mut list = std::mem::take(&mut *bg.borrow_mut());
                let mut failed = None;
                list.retain_mut(|f| match f.as_mut().poll(cx) {
                    Poll::Ready(Err(Ctrl::Error(e))) => {
                        failed.get_or_insert(e);
                        false
                    }
                    Poll::Ready(_) => false,
                    Poll::Pending => true,
                });
                let mut cur = bg.borrow_mut();
                let added = std::mem::replace(&mut *cur, list);
                cur.extend(added);
                match failed {
                    Some(e) => Poll::Ready(Err(Ctrl::Error(e))),
                    None => Poll::Pending,
                }
            })
            .await
        };
        // Leaving the function cancels whatever it left running.
        bg.borrow_mut().clear();
        let returns_void = matches!(&def.ret, Some(TypeExpr::Named { name, .. }) if name == "void");
        match r {
            Ok(v) | Err(Ctrl::Return(v)) => Ok(if returns_void { Value::Void } else { v }),
            Err(Ctrl::Break) => Err(error("runtime_error", "`break` outside a loop", span)),
            Err(other) => Err(other),
        }
    });
    Box::pin(poll_fn(move |cx| {
        {
            let mut vm = sh.borrow_mut();
            if vm.depth >= vm.limits.max_depth {
                return Poll::Ready(Err(error(
                    "stack_overflow",
                    format!(
                        "call depth exceeded {} while calling `{}`; the recursion probably never ends",
                        vm.limits.max_depth, name.name
                    ),
                    span,
                )));
            }
            vm.depth += 1;
        }
        // A pending call has no stack frame until it is polled again.
        let _depth = DepthGuard(sh);
        call.as_mut().poll(cx)
    }))
}

enum Target {
    Fn {
        def: Rc<FuncDef>,
        this: Option<Value>,
        class: Option<usize>,
    },
    Val(Value),
    Cast(usize),
    Builtin(String),
    Method(Value, String),
}

impl Vm {
    /// Work out what a call expression calls, mirroring `call_expr`.
    fn resolve(
        &mut self,
        callee: &Expr,
        env: &mut Env,
        recv: Option<Value>,
    ) -> Result<Target, Ctrl> {
        match &callee.kind {
            ExprKind::Ident(name) => self.resolve_name(name, env, callee.span),
            ExprKind::Member(obj, name, nsp) => {
                let recv = match recv {
                    Some(v) => v,
                    None => {
                        if let ExprKind::Ident(n) = &obj.kind {
                            if env.slot(n).is_none() && self.prog.modules.contains(n) {
                                return self.resolve_name(name, env, *nsp);
                            }
                        }
                        self.eval(obj, env)?
                    }
                };
                if let Value::Object(o) = &recv {
                    let prog = self.prog.clone();
                    let cls = &prog.classes[o.class];
                    if let Some((def, c)) = cls.methods.get(name) {
                        return Ok(Target::Fn {
                            def: def.clone(),
                            this: Some(recv.clone()),
                            class: Some(*c),
                        });
                    }
                    if let Some(idx) = cls.field_index(name) {
                        return Ok(Target::Val(o.fields.borrow()[idx].clone()));
                    }
                    if !self.prog.ext.contains_key(name) {
                        return Err(error(
                            "runtime_error",
                            format!("`{}` has no method `{}`", o.class_name, name),
                            callee.span,
                        ));
                    }
                }
                Ok(Target::Method(recv, name.clone()))
            }
            _ => Ok(Target::Val(self.eval(callee, env)?)),
        }
    }

    fn resolve_name(&mut self, name: &str, env: &mut Env, span: Span) -> Result<Target, Ctrl> {
        if let Some(v) = env.get(name) {
            return Ok(Target::Val(v));
        }
        if let Some(Value::Object(o)) = env.this.clone() {
            let prog = self.prog.clone();
            let cls = &prog.classes[o.class];
            if let Some((def, c)) = cls.methods.get(name) {
                return Ok(Target::Fn {
                    def: def.clone(),
                    this: env.this.clone(),
                    class: Some(*c),
                });
            }
            if let Some(idx) = cls.field_index(name) {
                return Ok(Target::Val(o.fields.borrow()[idx].clone()));
            }
        }
        if let Some(cell) = self.globals.get(name) {
            return Ok(Target::Val(cell.borrow().clone()));
        }
        if let Some(def) = self.prog.funcs.get(name) {
            return Ok(Target::Fn {
                def: def.clone(),
                this: None,
                class: None,
            });
        }
        if let Some(&cid) = self.prog.class_ids.get(name) {
            return Ok(Target::Cast(cid));
        }
        if builtins::is_builtin(name) || self.prog.host_functions.contains(name) {
            return Ok(Target::Builtin(name.to_string()));
        }
        Err(error(
            "runtime_error",
            format!("unknown function `{}`", name),
            span,
        ))
    }
}

fn values(args: Vec<(Option<String>, Value)>) -> Vec<Value> {
    args.into_iter().map(|(_, v)| v).collect()
}

async fn acall_expr(
    sh: &Sh,
    e: &Expr,
    callee: &Expr,
    args: &[Arg],
    bracket: bool,
    env: &mut Env,
) -> R {
    // A receiver that may itself suspend is evaluated first.
    let recv = match &callee.kind {
        ExprKind::Member(obj, _, _) if sh.borrow_mut().suspends(obj) => {
            Some(aeval(sh, obj, env).await?)
        }
        _ => None,
    };
    let target = sh.borrow_mut().resolve(callee, env, recv)?;
    let mut a = Vec::with_capacity(args.len());
    for arg in args {
        a.push((arg.name.clone(), aeval(sh, &arg.value, env).await?));
    }
    let span = e.span;
    match target {
        Target::Fn { def, this, class } => {
            let printable = matches!(
                a.first(),
                Some((
                    _,
                    Value::Int(_)
                        | Value::Float(_)
                        | Value::Str(_)
                        | Value::Char(_)
                        | Value::Rational(..)
                ))
            );
            if def.name == "ToString" && this.is_none() && printable {
                return builtins::call(&mut sh.borrow_mut(), "ToString", values(a), bracket, span);
            }
            let def = if this.is_none() {
                let picked = sh.borrow().pick_overload(&def.name, &a);
                match picked {
                    Some(d) => d,
                    None => {
                        return Err(error(
                            "runtime_error",
                            format!("no overload of `{}` accepts these arguments", def.name),
                            span,
                        ))
                    }
                }
            } else {
                def
            };
            let (def, class) = match class {
                Some(c) if this.is_some() => {
                    let (d, c) = sh.borrow().pick_method(&this, &def, c, &a);
                    (d, Some(c))
                }
                other => (def, other),
            };
            acall_fn(sh, def, this, class, a, span).await
        }
        Target::Val(Value::Func(f)) => match &*f {
            FuncVal::User { def, this, class } => {
                acall_fn(sh, def.clone(), this.clone(), *class, a, span).await
            }
            FuncVal::Builtin(name) => abuiltin(sh, name, a, bracket, span).await,
        },
        Target::Val(v) => sh.borrow_mut().call_value(v, a, bracket, span),
        Target::Cast(cid) => sh.borrow_mut().cast(cid, a, bracket, span),
        Target::Builtin(name) => abuiltin(sh, &name, a, bracket, span).await,
        Target::Method(recv, name) => {
            let ext = sh.borrow().pick_extension(&recv, &name, &a);
            if let Some(def) = ext {
                let mut full = vec![(None, recv)];
                full.extend(a);
                return acall_fn(sh, def, None, None, full, span).await;
            }
            amethod(sh, recv, name, a, span).await
        }
    }
}

async fn amethod(
    sh: &Sh,
    recv: Value,
    name: String,
    a: Vec<(Option<String>, Value)>,
    span: Span,
) -> R {
    match (&recv, name.as_str()) {
        (Value::Event(ev), "Await") => await_event(ev.clone()).await,
        (Value::Task(t), "Await") => await_task(t.clone()).await,
        (Value::Event(ev), "Signal") => {
            let payload = values(a).into_iter().next().unwrap_or(Value::Void);
            sh.borrow_mut().signal(ev, payload, span)?;
            Ok(Value::Void)
        }
        (Value::Event(ev), "Subscribe") => {
            let handler = values(a).into_iter().next().unwrap_or(Value::Void);
            Ok(sh.borrow_mut().subscribe(ev, handler))
        }
        (Value::Cancelable(c), "Cancel") => {
            c.event.subs.borrow_mut().retain(|(id, _)| *id != c.id);
            Ok(Value::Void)
        }
        (Value::Array(_) | Value::Str(_), _) => {
            builtins::method(&mut sh.borrow_mut(), recv, &name, values(a), span)
        }
        (other, _) => Err(error(
            "runtime_error",
            format!("a {} has no method `{}`", other.type_name(), name),
            span,
        )),
    }
}

async fn abuiltin(
    sh: &Sh,
    name: &str,
    args: Vec<(Option<String>, Value)>,
    bracket: bool,
    span: Span,
) -> R {
    if name == "Sleep" {
        return match args.first() {
            Some((_, Value::Float(secs))) => sleep(sh, *secs).await,
            _ => Err(error(
                "runtime_error",
                "`Sleep` takes a number of seconds as a float",
                span,
            )),
        };
    }
    sh.borrow_mut()
        .call_builtin_or_host(name, args, bracket, span)
}

// ---------------------------------------------------------------- evaluation

type Unit<'a> = Pin<Box<dyn Future<Output = Result<(), Ctrl>> + 'a>>;

fn afor<'a>(
    sh: &'a Sh,
    clauses: &'a [ForClause],
    idx: usize,
    body: &'a Expr,
    env: &'a mut Env,
    out: &'a mut Vec<Value>,
) -> Unit<'a> {
    Box::pin(async move {
        if idx == clauses.len() {
            let mark = env.vars.len();
            let v = aeval(sh, body, env).await;
            env.vars.truncate(mark);
            out.push(v?);
            return Ok(());
        }
        match &clauses[idx] {
            ForClause::Gen {
                key, name, iter, ..
            } => {
                // The header is a failure context, so it runs synchronously.
                let items: Vec<(Value, Value)> = if let ExprKind::Range(lo, hi) = &iter.kind {
                    let (a, b) = {
                        let mut vm = sh.borrow_mut();
                        (vm.eval(lo, env)?, vm.eval(hi, env)?)
                    };
                    let (Value::Int(a), Value::Int(b)) = (a, b) else {
                        return Err(error(
                            "runtime_error",
                            "range bounds must be ints",
                            iter.span,
                        ));
                    };
                    if b.saturating_sub(a) > 10_000_000 {
                        return Err(error(
                            "runtime_error",
                            "this range is too large to iterate in suspending code",
                            iter.span,
                        ));
                    }
                    (a..=b).map(|i| (Value::Int(i), Value::Int(i))).collect()
                } else {
                    let source = sh.borrow_mut().attempt(|vm| vm.eval(iter, env))?;
                    match source {
                        None => return Ok(()),
                        Some(Value::Array(a)) => a
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (Value::Int(i as i64), v.clone()))
                            .collect(),
                        Some(Value::Map(m)) => m.iter().cloned().collect(),
                        Some(Value::Str(s)) => s
                            .chars()
                            .enumerate()
                            .map(|(i, c)| (Value::Int(i as i64), Value::Char(c)))
                            .collect(),
                        Some(other) => {
                            return Err(error(
                                "runtime_error",
                                format!("`for` cannot iterate a {}", other.type_name()),
                                iter.span,
                            ))
                        }
                    }
                };
                for (k, v) in items {
                    let mark = env.vars.len();
                    if let Some((kn, _)) = key {
                        env.vars.push((kn.clone(), Slot::Val(k)));
                    }
                    env.vars.push((name.clone(), Slot::Val(v)));
                    let r = afor(sh, clauses, idx + 1, body, env, out).await;
                    env.vars.truncate(mark);
                    r?;
                }
                Ok(())
            }
            ForClause::Expr(x) => {
                let passed = sh.borrow_mut().attempt(|vm| vm.eval(x, env))?;
                match passed {
                    None => Ok(()),
                    Some(_) => afor(sh, clauses, idx + 1, body, env, out).await,
                }
            }
        }
    })
}

/// Evaluate an expression that may suspend. Anything that cannot suspend is
/// handed straight to the synchronous evaluator.
fn aeval<'a>(sh: &'a Sh, e: &'a Expr, env: &'a mut Env) -> Fut<'a> {
    Box::pin(async move {
        if !sh.borrow_mut().suspends(e) {
            return sh.borrow_mut().eval(e, env);
        }
        sh.borrow_mut().step(e.span)?;
        match &e.kind {
            ExprKind::Block(stmts) => {
                let mark = env.vars.len();
                let dmark = env.defers.len();
                let mut result = Ok(Value::Void);
                for s in stmts {
                    result = aeval(sh, s, env).await;
                    if result.is_err() {
                        break;
                    }
                }
                let result = sh.borrow_mut().run_defers(env, dmark, result);
                env.vars.truncate(mark);
                result
            }
            ExprKind::Def(d) => {
                let v = match &d.value {
                    Some(x) => aeval(sh, x, env).await?,
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
                let rhs = aeval(sh, value, env).await?;
                let mut vm = sh.borrow_mut();
                let new = match op {
                    None => rhs,
                    Some(op) => {
                        let old = vm.eval(target, env)?;
                        vm.arith(*op, old, rhs, e.span)?
                    }
                };
                vm.write_place(target, new.clone(), env)?;
                Ok(new)
            }
            ExprKind::Unary(op, inner) if *op != UnOp::Not => {
                let v = aeval(sh, inner, env).await?;
                if *op == UnOp::Pos {
                    return Ok(v);
                }
                sh.borrow_mut().negate(v, e.span)
            }
            ExprKind::Binary(op, l, r) => match op {
                BinOp::And => {
                    aeval(sh, l, env).await?;
                    aeval(sh, r, env).await
                }
                BinOp::Or => {
                    let left = sh.borrow_mut().attempt(|vm| vm.eval(l, env))?;
                    match left {
                        Some(v) => Ok(v),
                        None => aeval(sh, r, env).await,
                    }
                }
                _ => {
                    let a = aeval(sh, l, env).await?;
                    let b = aeval(sh, r, env).await?;
                    let mut vm = sh.borrow_mut();
                    if op.is_comparison() {
                        if vm.compare(*op, &a, &b, e.span)? {
                            Ok(a)
                        } else {
                            Err(Ctrl::Fail)
                        }
                    } else {
                        vm.arith(*op, a, b, e.span)
                    }
                }
            },
            ExprKind::Str(segs) => {
                let mut s = String::new();
                for seg in segs {
                    match seg {
                        StrSeg::Lit(l) => s.push_str(l),
                        StrSeg::Expr(x) => {
                            let v = aeval(sh, x, env).await?;
                            s.push_str(&sh.borrow_mut().text_of(&v, x.span)?);
                        }
                    }
                }
                Ok(Value::Str(Rc::from(s)))
            }
            ExprKind::Tuple(items) | ExprKind::ArrayLit(items) => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    vs.push(aeval(sh, it, env).await?);
                }
                Ok(if matches!(e.kind, ExprKind::Tuple(_)) {
                    Value::Tuple(Rc::new(vs))
                } else {
                    Value::Array(Rc::new(vs))
                })
            }
            ExprKind::Query(inner) => match aeval(sh, inner, env).await? {
                Value::Opt(Some(v)) => Ok((*v).clone()),
                Value::Opt(None) | Value::Logic(false) | Value::Void => Err(Ctrl::Fail),
                other => Ok(other),
            },
            ExprKind::Call {
                callee,
                args,
                bracket,
            } => acall_expr(sh, e, callee, args, *bracket, env).await,
            ExprKind::Member(obj, name, _) => {
                let v = aeval(sh, obj, env).await?;
                sh.borrow_mut().member(&v, name, e.span)
            }
            ExprKind::SuperCall { method, args } => {
                let found = {
                    let vm = sh.borrow();
                    env.class.and_then(|cls| {
                        vm.prog.classes[cls]
                            .parents
                            .iter()
                            .find_map(|p| vm.prog.classes[*p].methods.get(method).cloned())
                    })
                };
                let Some((def, owner)) = found else {
                    return Err(error(
                        "runtime_error",
                        format!("no parent method `{}`", method),
                        e.span,
                    ));
                };
                let mut a = Vec::with_capacity(args.len());
                for arg in args {
                    a.push((arg.name.clone(), aeval(sh, &arg.value, env).await?));
                }
                acall_fn(sh, def, env.this.clone(), Some(owner), a, e.span).await
            }
            ExprKind::If { arms, else_body } => {
                for arm in arms {
                    let mark = env.vars.len();
                    // Conditions are failure contexts and never suspend.
                    let ok = sh.borrow_mut().attempt(|vm| {
                        for c in &arm.cond {
                            vm.eval(c, env)?;
                        }
                        Ok(Value::Void)
                    })?;
                    if ok.is_some() {
                        let r = aeval(sh, &arm.body, env).await;
                        env.vars.truncate(mark);
                        return r;
                    }
                    env.vars.truncate(mark);
                }
                match else_body {
                    Some(b) => aeval(sh, b, env).await,
                    None => Ok(Value::Void),
                }
            }
            ExprKind::For { clauses, body } => {
                let mut out = Vec::new();
                let mark = env.vars.len();
                let r = afor(sh, clauses, 0, body, env, &mut out).await;
                env.vars.truncate(mark);
                r?;
                Ok(Value::Array(Rc::new(out)))
            }
            ExprKind::Loop(body) => loop {
                sh.borrow_mut().step(e.span)?;
                let mark = env.vars.len();
                let r = aeval(sh, body, env).await;
                env.vars.truncate(mark);
                match r {
                    Ok(_) => {}
                    Err(Ctrl::Break) => return Ok(Value::Void),
                    Err(c) => return Err(c),
                }
            },
            ExprKind::Return(v) => {
                let val = match v {
                    Some(x) => aeval(sh, x, env).await?,
                    None => Value::Void,
                };
                Err(Ctrl::Return(val))
            }
            ExprKind::Case { scrutinee, arms } => {
                let v = aeval(sh, scrutinee, env).await?;
                for arm in arms {
                    let matched = match &arm.pattern {
                        None => true,
                        Some(p) => sh.borrow_mut().eval(p, env)?.equals(&v),
                    };
                    if matched {
                        return aeval(sh, &arm.body, env).await;
                    }
                }
                Err(Ctrl::Fail)
            }
            ExprKind::MapLit(items) => {
                let mut m: Vec<(Value, Value)> = Vec::with_capacity(items.len());
                for (k, v) in items {
                    let kv = aeval(sh, k, env).await?;
                    let vv = aeval(sh, v, env).await?;
                    m.retain(|(ek, _)| !ek.equals(&kv));
                    m.push((kv, vv));
                }
                Ok(Value::Map(Rc::new(m)))
            }
            ExprKind::Archetype { ty, fields } => {
                let name = match &ty.kind {
                    ExprKind::Ident(n) => n.clone(),
                    ExprKind::Member(_, n, _) => n.clone(),
                    _ => return sh.borrow_mut().eval(e, env),
                };
                let cid = sh.borrow().prog.class_ids.get(&name).copied();
                let Some(cid) = cid else {
                    return sh.borrow_mut().eval(e, env);
                };
                let mut given = Vec::with_capacity(fields.len());
                for (fname, _, fexpr) in fields {
                    given.push((fname.clone(), aeval(sh, fexpr, env).await?));
                }
                sh.borrow_mut().instantiate(cid, given, e.span)
            }
            ExprKind::Concurrent { kind, arms } => aconcurrent(sh, *kind, arms, env).await,
            ExprKind::Spawn(body) => {
                let body = sh.borrow_mut().body_rc(body);
                let (cell, task) = spawn_task(sh, body, env.child());
                // A spawned task runs at once, up to its first suspension.
                let mut task = task;
                let finished = poll_fn(|cx| Poll::Ready(task.as_mut().poll(cx).is_ready())).await;
                if !finished {
                    sh.borrow_mut().sched.spawned.push(task);
                }
                if let Some(err) = sh.borrow_mut().sched.fatal.take() {
                    return Err(Ctrl::Error(Box::new(err)));
                }
                Ok(Value::Task(cell))
            }
            // Failure contexts (`not`, `option{}`, `logic{}`) and everything
            // else cannot suspend part-way; run them synchronously.
            _ => sh.borrow_mut().eval(e, env),
        }
    })
}

async fn aconcurrent(sh: &Sh, kind: ConcKind, arms: &[Expr], env: &mut Env) -> R {
    match kind {
        ConcKind::Sync => {
            let mut envs: Vec<Scope> = arms
                .iter()
                .map(|_| Scope {
                    sh: sh.clone(),
                    env: env.child(),
                })
                .collect();
            let mut futs: Vec<Fut> = envs
                .iter_mut()
                .zip(arms)
                .map(|(en, a)| aeval(sh, a, &mut en.env))
                .collect();
            let mut results: Vec<Option<Value>> = arms.iter().map(|_| None).collect();
            poll_fn(|cx| {
                for i in 0..futs.len() {
                    if results[i].is_none() {
                        match futs[i].as_mut().poll(cx) {
                            Poll::Ready(Ok(v)) => results[i] = Some(v),
                            Poll::Ready(Err(c)) => return Poll::Ready(Err(c)),
                            Poll::Pending => {}
                        }
                    }
                }
                if results.iter().all(|r| r.is_some()) {
                    let vs: Vec<Value> = results.iter_mut().map(|r| r.take().unwrap()).collect();
                    Poll::Ready(Ok(Value::Tuple(Rc::new(vs))))
                } else {
                    Poll::Pending
                }
            })
            .await
        }
        ConcKind::Race => {
            let mut envs: Vec<Scope> = arms
                .iter()
                .map(|_| Scope {
                    sh: sh.clone(),
                    env: env.child(),
                })
                .collect();
            let mut futs: Vec<Fut> = envs
                .iter_mut()
                .zip(arms)
                .map(|(en, a)| aeval(sh, a, &mut en.env))
                .collect();
            // Arms start in order. The first to finish wins; returning drops
            // the others, which cancels them where they are suspended. Arms
            // not yet started never run.
            poll_fn(|cx| {
                for f in futs.iter_mut() {
                    if let Poll::Ready(r) = f.as_mut().poll(cx) {
                        return Poll::Ready(r);
                    }
                }
                Poll::Pending
            })
            .await
        }
        ConcKind::Rush => {
            let mut futs: Vec<Option<Fut<'static>>> = Vec::with_capacity(arms.len());
            for a in arms {
                let body = sh.borrow_mut().body_rc(a);
                futs.push(Some(detached(sh.clone(), body, env.child())));
            }
            let winner = poll_fn(|cx| {
                for slot in futs.iter_mut() {
                    if let Some(f) = slot {
                        if let Poll::Ready(r) = f.as_mut().poll(cx) {
                            *slot = None;
                            return Poll::Ready(r);
                        }
                    }
                }
                Poll::Pending
            })
            .await;
            // The losers keep running until the enclosing function returns.
            for f in futs.into_iter().flatten() {
                match &env.bg {
                    Some(bg) => bg.borrow_mut().push(f),
                    None => adopt(sh, f),
                }
            }
            winner
        }
        ConcKind::Branch => {
            let block = Expr {
                kind: ExprKind::Block(arms.to_vec()),
                span: arms.first().map(|a| a.span).unwrap_or_default(),
            };
            let key = arms.first().map(|a| a as *const Expr as usize).unwrap_or(0);
            let body = {
                let mut vm = sh.borrow_mut();
                vm.sched
                    .bodies
                    .entry(key ^ 1)
                    .or_insert_with(|| Rc::new(block))
                    .clone()
            };
            let mut fut = detached(sh.clone(), body, env.child());
            match poll_once(&mut fut).await {
                Some(Err(c @ Ctrl::Error(_))) => return Err(c),
                Some(_) => {}
                None => match &env.bg {
                    Some(bg) => bg.borrow_mut().push(fut),
                    None => adopt(sh, fut),
                },
            }
            Ok(Value::Void)
        }
    }
}
