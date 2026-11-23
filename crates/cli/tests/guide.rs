//! The agent guide must practise what it preaches: every `verse` block in it
//! checks without errors or warnings, and its example test passes.

use reverse_cli::{check_report, test_report, GUIDE};
use reverse_vm::Limits;

fn verse_blocks() -> Vec<String> {
    blocks_of(GUIDE)
}

fn blocks_of(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        match &mut cur {
            None if line.trim() == "```verse" => cur = Some(String::new()),
            Some(_) if line.trim() == "```" => blocks.push(cur.take().unwrap()),
            Some(b) => {
                b.push_str(line);
                b.push('\n');
            }
            None => {}
        }
    }
    blocks
}

#[test]
fn every_guide_example_checks_clean() {
    let blocks = verse_blocks();
    assert!(blocks.len() >= 8, "expected the guide to contain examples");
    for (i, b) in blocks.iter().enumerate() {
        let r = check_report(b, "guide.verse");
        assert!(
            r["diagnostics"].as_array().unwrap().is_empty(),
            "guide example {} has diagnostics:\n{}\n{}",
            i + 1,
            b,
            reverse_cli::render_check(&r)
        );
    }
}

#[test]
fn guide_examples_run() {
    let all: String = verse_blocks().join("\n");
    let r = test_report(&all, "guide.verse", Limits::default());
    assert_eq!(r["ok"], true, "{}", reverse_cli::render_test(&r));
    assert_eq!(r["passed"], 3);
}

/// Verse shown in the README and the documentation must check clean too.
#[test]
fn documentation_examples_check_clean() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    for file in ["README.md", "docs/devices.md", "docs/diagnostics.md"] {
        let text = std::fs::read_to_string(format!("{}/{}", root, file)).unwrap();
        let blocks = blocks_of(&text);
        assert!(!blocks.is_empty(), "{} has no examples", file);
        // The diagnostics page shows failing examples on purpose.
        if file == "docs/diagnostics.md" {
            continue;
        }
        for b in blocks {
            let r = check_report(&b, file);
            assert!(
                r["diagnostics"].as_array().unwrap().is_empty(),
                "{} example has diagnostics:\n{}\n{}",
                file,
                b,
                reverse_cli::render_check(&r)
            );
        }
    }
}

/// The two-file device example passes, and the device file alone is clean.
#[test]
fn door_example_passes() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/door");
    let read = |n: &str| std::fs::read_to_string(format!("{}/{}", root, n)).unwrap();
    let device = read("door.verse");
    let alone = check_report(&device, "door.verse");
    assert!(
        alone["diagnostics"].as_array().unwrap().is_empty(),
        "{}",
        reverse_cli::render_check(&alone)
    );
    let files = vec![
        ("door.verse".to_string(), device),
        ("door_test.verse".to_string(), read("door_test.verse")),
    ];
    let r = reverse_cli::test_files(&files, Limits::default());
    assert_eq!(r["ok"], true, "{}", reverse_cli::render_test(&r));
    assert_eq!(r["passed"], 3);
    assert!(r["diagnostics"].as_array().unwrap().is_empty(), "{}", r);
}
