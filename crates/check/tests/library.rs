//! The built-in device stand-ins must themselves be clean Verse.

use reverse_check::{analyze, library_diagnostics, stdlib};

#[test]
fn the_library_checks_cleanly() {
    let problems = library_diagnostics();
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn every_definition_names_its_module() {
    for c in stdlib::chunks() {
        assert!(!c.name.is_empty(), "unnamed definition: {}", c.text);
        assert!(
            c.module == "sim" || c.module.starts_with('/'),
            "{} has module {:?}",
            c.name,
            c.module
        );
        // Test helpers are recognisable by name.
        if c.module == "sim" {
            assert!(
                stdlib::is_sim_name(c.name),
                "{} should start with Sim",
                c.name
            );
        }
    }
}

#[test]
fn a_file_can_replace_a_stand_in() {
    // A definition in the file wins over the library's one of the same name.
    let src = "using { /Fortnite.com/Devices }\n\nbutton_device := class:\n    Mine : int = 1\n\nUse() : int = button_device{}.Mine\n";
    let (_, r) = analyze(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn files_without_fortnite_imports_are_untouched() {
    let (_, r) = analyze("X : button_device = button_device{}\n");
    assert!(r.diagnostics.iter().any(|d| d.code == "V1002"));
}
