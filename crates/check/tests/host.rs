use reverse_check::types::{Effects, FuncSig, Ty};
use reverse_check::{analyze, analyze_with_host};

#[test]
fn host_functions_have_checked_types_and_effects() {
    let bindings = [FuncSig::simple(
        "TryReadDoor",
        &[Ty::Logic],
        Ty::Int,
        Effects::TRANSACTS.decides(),
    )];
    let valid = "Open(IsAllowed : logic)<decides><transacts> : int = TryReadDoor[IsAllowed]\n";
    let (_, result) = analyze_with_host(valid, &bindings);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.functions.len(), 1);
    assert_eq!(result.functions[0].name, "Open");
    assert_eq!(result.functions[0].params[0].ty, Ty::Logic);
    assert_eq!(result.functions[0].return_type, Ty::Int);
    assert!(analyze(valid).1.has_errors());

    let invalid = "Open()<decides><transacts> : int = TryReadDoor[42]\n";
    let (_, result) = analyze_with_host(invalid, &bindings);
    assert!(result.diagnostics.iter().any(|d| d.code == "V2001"));

    let invalid = "Open() : int = TryReadDoor(true)\n";
    let (_, result) = analyze_with_host(invalid, &bindings);
    assert!(result.diagnostics.iter().any(|d| d.code == "V3002"));

    let invalid = "Open()<decides><transacts> : logic = TryReadDoor[true]\n";
    let (_, result) = analyze_with_host(invalid, &bindings);
    assert!(result.diagnostics.iter().any(|d| d.code == "V2006"));

    let invalid = "Open()<decides><computes> : int = TryReadDoor[true]\n";
    let (_, result) = analyze_with_host(invalid, &bindings);
    assert!(result.diagnostics.iter().any(|d| d.code == "V4002"));
}

#[test]
fn user_functions_and_builtins_shadow_host_functions() {
    let bindings = [
        FuncSig::simple("CurrentValue", &[Ty::Str], Ty::Int, Effects::COMPUTES),
        FuncSig::simple("Sleep", &[Ty::Str], Ty::Int, Effects::COMPUTES),
    ];
    let source = "CurrentValue() : logic = true\n\
                  Main()<suspends> : logic =\n    Sleep(0.0)\n    CurrentValue()\n";
    let (_, result) = analyze_with_host(source, &bindings);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.functions.len(), 2);
}
