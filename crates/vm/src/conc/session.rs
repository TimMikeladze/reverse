use super::{acall_fn, outcome, poll_tasks, reset_round, Fut, HostClock, Sh, TaskFut};
use crate::{Ctrl, Outcome, RuntimeError, Value, Vm};
use reverse_syntax::Span;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

struct Entry {
    name: String,
    task: Fut<'static>,
}

/// A VM driven by a host clock. Entries and detached tasks can suspend
/// across ticks. Use one session for each independent set of Verse globals.
pub struct Session {
    sh: Sh,
    entries: Vec<Entry>,
    tasks: Vec<TaskFut>,
    fault: Option<RuntimeError>,
}

impl Session {
    pub fn new(vm: Vm) -> Self {
        let sh = Rc::new(RefCell::new(vm));
        {
            let mut vm = sh.borrow_mut();
            vm.sched.me = Rc::downgrade(&sh);
            vm.sched.host_clock = Some(HostClock::default());
        }
        Self {
            sh,
            entries: Vec::new(),
            tasks: Vec::new(),
            fault: None,
        }
    }

    /// Advance the clock and run all ready work within the VM's step limit.
    /// Pending entries resume on later ticks. An uncaught fault stops the
    /// session and rolls back the host's staged effects from this tick.
    pub fn tick(
        &mut self,
        delta: Duration,
        calls: impl IntoIterator<Item = (String, Vec<Value>)>,
    ) -> Result<Vec<(String, Outcome)>, RuntimeError> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        let checkpoint = {
            let mut vm = self.sh.borrow_mut();
            vm.steps = 0;
            vm.out_bytes = 0;
            let clock = vm
                .sched
                .host_clock
                .as_mut()
                .expect("session has a host clock");
            clock.tick += 1;
            clock.elapsed = clock.elapsed.saturating_add(delta);
            vm.now = clock.elapsed.as_secs_f64();
            vm.host_checkpoint()
        };
        let result = self.run_ready(calls);
        if let Err(fault) = &result {
            self.cancel_tasks();
            self.sh.borrow_mut().rollback_host(checkpoint);
            self.fault = Some(fault.clone());
        }
        result
    }

    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sh.borrow_mut().output)
    }

    fn run_ready(
        &mut self,
        calls: impl IntoIterator<Item = (String, Vec<Value>)>,
    ) -> Result<Vec<(String, Outcome)>, RuntimeError> {
        for (name, args) in calls {
            self.entries.push(entry(&self.sh, name, args)?);
        }
        let mut cx = Context::from_waker(Waker::noop());
        let mut completed = Vec::new();
        while !self.entries.is_empty() || !self.tasks.is_empty() {
            {
                let mut vm = self.sh.borrow_mut();
                reset_round(&mut vm);
                if let Err(Ctrl::Error(e)) = vm.step(Span::new(0, 0)) {
                    return Err(*e);
                }
            }
            let mut failed = None;
            self.entries.retain_mut(|entry| {
                if failed.is_some() {
                    return true;
                }
                match entry.task.as_mut().poll(&mut cx) {
                    Poll::Pending => true,
                    Poll::Ready(result) => {
                        match outcome(result) {
                            Outcome::Error(e) => failed = Some(e),
                            result => completed.push((std::mem::take(&mut entry.name), result)),
                        }
                        false
                    }
                }
            });
            if let Some(fault) = failed {
                return Err(fault);
            }
            poll_tasks(&self.sh, &mut self.tasks, &mut cx);
            let mut vm = self.sh.borrow_mut();
            if let Some(fault) = vm.sched.fatal.take() {
                return Err(fault);
            }
            if !vm.sched.progress {
                break;
            }
        }
        Ok(completed)
    }

    fn cancel_tasks(&mut self) {
        self.sh.borrow_mut().sched.me = Weak::new();
        self.entries.clear();
        self.tasks.clear();
        let spawned = std::mem::take(&mut self.sh.borrow_mut().sched.spawned);
        drop(spawned);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let checkpoint = self.sh.borrow().host_checkpoint();
        self.cancel_tasks();
        self.sh.borrow_mut().rollback_host(checkpoint);
    }
}

fn entry(sh: &Sh, name: String, args: Vec<Value>) -> Result<Entry, RuntimeError> {
    let vm = sh.borrow();
    let span = vm
        .prog
        .funcs
        .get(&name)
        .map(|def| def.name_span)
        .ok_or_else(|| RuntimeError {
            kind: "runtime_error",
            message: format!("no function named `{name}`"),
            span: None,
        })?;
    let args: Vec<_> = args.into_iter().map(|value| (None, value)).collect();
    let def = vm.pick_overload(&name, &args).ok_or_else(|| RuntimeError {
        kind: "runtime_error",
        message: format!("no overload of `{name}` accepts these arguments"),
        span: Some(span),
    })?;
    // `spawn` puts a name in the async index even inside an immediate failable
    // function. Such an entry still needs a transaction around the whole call.
    let decides = def.effects.iter().any(|effect| effect.name == "decides");
    let suspends = !decides && vm.prog.async_funcs.contains(&name);
    let sh = sh.clone();
    let task = Box::pin(async move {
        if suspends {
            acall_fn(&sh, def, None, None, args, span).await
        } else {
            let mut vm = sh.borrow_mut();
            vm.in_entry(|vm| vm.call_fn(&def, None, None, args, span))
        }
    });
    Ok(Entry { name, task })
}
