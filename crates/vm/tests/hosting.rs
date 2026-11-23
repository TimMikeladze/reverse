use reverse_syntax::Span;
use reverse_vm::{Ctrl, Host, Limits, Outcome, Program, Session, Value, Vm};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Clone, Default)]
struct Writes(Rc<RefCell<Vec<i64>>>);

impl Host for Writes {
    fn call(
        &mut self,
        name: &str,
        args: &[(Option<String>, Value)],
        _: Span,
    ) -> Result<Value, Ctrl> {
        assert_eq!(name, "Stage");
        let [(None, Value::Int(value))] = args else {
            panic!("expected an int")
        };
        self.0.borrow_mut().push(*value);
        Ok(Value::Void)
    }

    fn checkpoint(&self) -> usize {
        self.0.borrow().len()
    }

    fn rollback(&mut self, checkpoint: usize) {
        self.0.borrow_mut().truncate(checkpoint);
    }
}

fn session(src: &str, max_steps: u64, writes: Writes) -> Session {
    let limits = Limits {
        max_steps,
        ..Limits::default()
    };
    session_with_limits(src, limits, writes)
}

fn session_with_limits(src: &str, limits: Limits, writes: Writes) -> Session {
    let (module, diagnostics) = reverse_syntax::parse(src);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let prog = Program::load_with_host(&module, ["Stage".into()]);
    Session::new(Vm::new_with_host(Rc::new(prog), limits, Box::new(writes)).unwrap())
}

#[test]
fn host_time_and_nested_rollback() {
    let writes = Writes::default();
    let mut vm = session(
        r#"
Reject()<decides><transacts> : void =
    Stage(9)
    0 = 1

Interact(Value : int)<suspends> : void =
    if (Reject[]) {}
    Stage(Value)
    Sleep(0.5)
    Stage(2)
    Sleep(0.0)
    Stage(3)
"#,
        100,
        writes.clone(),
    );
    let tick = Duration::from_millis(250);
    let result = vm
        .tick(tick, [("Interact".into(), vec![Value::Int(1)])])
        .unwrap();
    assert!(result.is_empty());
    assert_eq!(*writes.0.borrow(), [1]);
    assert!(vm.tick(tick, []).unwrap().is_empty());
    assert_eq!(*writes.0.borrow(), [1]);
    assert!(vm.tick(tick, []).unwrap().is_empty());
    assert_eq!(*writes.0.borrow(), [1, 2]);
    let result = vm.tick(tick, []).unwrap();
    assert!(
        matches!(result.as_slice(), [(name, Outcome::Value(Value::Void))] if name == "Interact")
    );
    assert_eq!(*writes.0.borrow(), [1, 2, 3]);
}

#[test]
fn a_timer_wakes_on_an_exact_host_tick_even_with_decimal_seconds() {
    let writes = Writes::default();
    let mut vm = session(
        "Start()<suspends> : void =\n    Sleep(0.2)\n    Stage(1)\n",
        100,
        writes.clone(),
    );
    let tick = Duration::from_millis(100);
    vm.tick(tick, [("Start".into(), vec![])]).unwrap();
    vm.tick(tick, []).unwrap();
    assert!(writes.0.borrow().is_empty());
    vm.tick(tick, []).unwrap();
    assert_eq!(*writes.0.borrow(), [1]);
}

#[test]
fn a_fault_discards_host_writes_from_the_current_tick() {
    let writes = Writes::default();
    let mut vm = session(
        r#"
Interact() : void =
    Stage(1)
    loop:
        Stage(9)
"#,
        40,
        writes.clone(),
    );
    let fault = vm
        .tick(Duration::ZERO, [("Interact".into(), vec![])])
        .unwrap_err();
    assert_eq!(fault.kind, "step_limit");
    assert!(writes.0.borrow().is_empty());
}

#[test]
fn independent_suspended_calls_do_not_consume_each_others_stack_depth() {
    let writes = Writes::default();
    let limits = Limits {
        max_depth: 1,
        ..Limits::default()
    };
    let mut vm = session_with_limits(
        "Callback(N : int)<suspends> : void =\n    Sleep(0.0)\n    Stage(N)\n",
        limits,
        writes.clone(),
    );
    let calls = (0..3).map(|n| ("Callback".into(), vec![Value::Int(n)]));
    assert!(vm.tick(Duration::ZERO, calls).unwrap().is_empty());
    assert_eq!(vm.tick(Duration::ZERO, []).unwrap().len(), 3);
    assert_eq!(*writes.0.borrow(), [0, 1, 2]);
}

#[test]
fn a_failed_entry_discards_native_writes_and_its_queued_spawn() {
    let writes = Writes::default();
    let mut vm = session(
        r#"
Later()<suspends> : void =
    Stage(2)
    Sleep(0.0)

Start()<decides><transacts> : void =
    Stage(1)
    spawn{Later()}
    0 = 1
"#,
        100,
        writes.clone(),
    );
    let result = vm.tick(Duration::ZERO, [("Start".into(), vec![])]).unwrap();
    assert!(matches!(result.as_slice(), [(_, Outcome::Failed)]));
    assert!(writes.0.borrow().is_empty());
    assert!(vm.tick(Duration::ZERO, []).unwrap().is_empty());
    assert!(writes.0.borrow().is_empty());
}

#[test]
fn recursive_async_calls_still_use_the_stack_depth_limit() {
    let limits = Limits {
        max_depth: 8,
        ..Limits::default()
    };
    let mut vm = session_with_limits(
        "Recurse()<suspends> : void = Recurse()\n",
        limits,
        Writes::default(),
    );
    let fault = vm
        .tick(Duration::ZERO, [("Recurse".into(), vec![])])
        .unwrap_err();
    assert_eq!(fault.kind, "stack_overflow");
}

#[test]
fn spawned_tasks_persist_and_the_budget_resets() {
    let writes = Writes::default();
    let mut vm = session(
        r#"
Later()<suspends> : void =
    loop:
        Sleep(0.0)
        Stage(1)

Start()<suspends> : void =
    spawn{Later()}
"#,
        40,
        writes.clone(),
    );
    vm.tick(Duration::ZERO, [("Start".into(), vec![])]).unwrap();
    for _ in 0..64 {
        vm.tick(Duration::from_millis(16), []).unwrap();
    }
    assert_eq!(writes.0.borrow().len(), 64);
}

#[test]
fn dropping_a_session_discards_pending_tasks_and_their_cleanup_writes() {
    let writes = Writes::default();
    let mut vm = session(
        "Start()<suspends> : void =\n    defer:\n        Stage(1)\n    Sleep(1.0)\n",
        100,
        writes.clone(),
    );
    vm.tick(Duration::ZERO, [("Start".into(), vec![])]).unwrap();
    assert_eq!(Rc::strong_count(&writes.0), 2);
    drop(vm);
    assert!(writes.0.borrow().is_empty());
    assert_eq!(Rc::strong_count(&writes.0), 1);
}

#[test]
fn host_dispatch_preserves_named_arguments() {
    struct Subtract;
    impl Host for Subtract {
        fn call(
            &mut self,
            name: &str,
            args: &[(Option<String>, Value)],
            _: Span,
        ) -> Result<Value, Ctrl> {
            assert_eq!(name, "Subtract");
            let get = |name| {
                args.iter()
                    .find_map(|(label, value)| match value {
                        Value::Int(number) if label.as_deref() == Some(name) => Some(*number),
                        _ => None,
                    })
                    .unwrap()
            };
            Ok(Value::Int(get("A") - get("B")))
        }

        fn checkpoint(&self) -> usize {
            0
        }
        fn rollback(&mut self, _: usize) {}
    }

    let source = "Start()<suspends> : int = Subtract(?B := 2, ?A := 5)\n";
    let (module, diagnostics) = reverse_syntax::parse(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let prog = Program::load_with_host(&module, ["Subtract".into()]);
    let vm = Vm::new_with_host(Rc::new(prog), Limits::default(), Box::new(Subtract)).unwrap();
    let result = Session::new(vm)
        .tick(Duration::ZERO, [("Start".into(), vec![])])
        .unwrap();
    assert!(matches!(
        result.as_slice(),
        [(_, Outcome::Value(Value::Int(3)))]
    ));
}
