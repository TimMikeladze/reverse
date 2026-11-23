//! Every eval task's reference solution must pass its own hidden tests.

use reverse_cli::{render_test, test_report};
use reverse_vm::Limits;
use std::path::Path;

#[test]
fn reference_solutions_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/tasks");
    let mut count = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        assert!(dir.join("task.md").exists(), "{} has no task.md", name);
        let src = format!(
            "{}\n{}",
            std::fs::read_to_string(dir.join("reference.verse")).unwrap(),
            std::fs::read_to_string(dir.join("tests.verse")).unwrap()
        );
        let r = test_report(&src, &name, Limits::default());
        assert_eq!(r["ok"], true, "{}:\n{}", name, render_test(&r));
        assert!(
            r["diagnostics"].as_array().unwrap().is_empty(),
            "{} has warnings:\n{}",
            name,
            render_test(&r)
        );
        count += 1;
    }
    assert!(count >= 10);
}
