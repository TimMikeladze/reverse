//! Conformance corpus runner.
//!
//! `tests/conformance/parse` and `check`: a trailing `#! CODE` marks a line
//! that must raise that diagnostic (`#!- CODE` on its own line refers to the
//! line above). Every error must be marked; unmarked warnings are allowed.
//!
//! `tests/conformance/run`: the file must check without errors, then `Main`
//! runs. `#> text` lines give the expected output in order; `#= status: X`
//! gives the expected status (default `returned`); `#= result: X` the result.

use reverse_cli::{check_report, run_report};
use reverse_vm::Limits;
use std::path::{Path, PathBuf};

fn files(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance")
        .join(dir);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", root.display(), e))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "verse").unwrap_or(false))
        .collect();
    v.sort();
    v
}

fn markers(src: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let n = i as u64 + 1;
        if let Some(rest) = line.trim().strip_prefix("#!- ") {
            for code in rest.split_whitespace() {
                out.push((n - 1, code.to_string()));
            }
        } else if let Some(pos) = line.find("#! ") {
            for code in line[pos + 3..].split_whitespace() {
                out.push((n, code.to_string()));
            }
        }
    }
    out
}

fn check_dir(dir: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for path in files(dir) {
        let name = format!("{}/{}", dir, path.file_name().unwrap().to_string_lossy());
        let src = std::fs::read_to_string(&path).unwrap();
        assert!(
            src.contains("# source:") && src.contains("# uefn-verified:"),
            "{}: missing provenance header",
            name
        );
        let report = check_report(&src, &name);
        let diags = report["diagnostics"].as_array().unwrap();
        let expected = markers(&src);
        for (line, code) in &expected {
            if !diags
                .iter()
                .any(|d| d["line"] == *line && d["code"] == code.as_str())
            {
                problems.push(format!(
                    "{}:{}: expected {} but it was not reported",
                    name, line, code
                ));
            }
        }
        for d in diags {
            if d["severity"] != "error" {
                continue;
            }
            let line = d["line"].as_u64().unwrap();
            let code = d["code"].as_str().unwrap();
            if !expected.iter().any(|(l, c)| *l == line && c == code) {
                problems.push(format!(
                    "{}:{}: unexpected {}: {}",
                    name,
                    line,
                    code,
                    d["message"].as_str().unwrap()
                ));
            }
        }
    }
    problems
}

#[test]
fn parse_corpus() {
    let p = check_dir("parse");
    assert!(p.is_empty(), "\n{}", p.join("\n"));
}

#[test]
fn check_corpus() {
    let p = check_dir("check");
    assert!(p.is_empty(), "\n{}", p.join("\n"));
}

#[test]
fn run_corpus() {
    let mut problems = Vec::new();
    for path in files("run") {
        let name = format!("run/{}", path.file_name().unwrap().to_string_lossy());
        let src = std::fs::read_to_string(&path).unwrap();
        assert!(
            src.contains("# source:") && src.contains("# uefn-verified:"),
            "{}: missing provenance header",
            name
        );
        let mut want_out: Vec<String> = Vec::new();
        let mut want_status = "returned".to_string();
        let mut want_result: Option<String> = None;
        for line in src.lines() {
            let t = line.trim_start();
            if t == "#>" {
                want_out.push(String::new());
            } else if let Some(o) = t.strip_prefix("#> ") {
                want_out.push(o.to_string());
            } else if let Some(s) = t.strip_prefix("#= status: ") {
                want_status = s.trim().to_string();
            } else if let Some(s) = t.strip_prefix("#= result: ") {
                want_result = Some(s.trim().to_string());
            }
        }
        let limits = Limits {
            max_steps: 2_000_000,
            ..Limits::default()
        };
        let report = run_report(&src, &name, None, limits, false);
        if report["stage"] == "check" {
            let msgs: Vec<String> = report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["severity"] == "error")
                .map(|d| {
                    format!(
                        "{}:{} {} {}",
                        name,
                        d["line"],
                        d["code"].as_str().unwrap(),
                        d["message"].as_str().unwrap()
                    )
                })
                .collect();
            problems.push(format!(
                "{}: does not check:\n    {}",
                name,
                msgs.join("\n    ")
            ));
            continue;
        }
        let got_out: Vec<String> = report["output"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();
        let got_status = match report["status"].as_str() {
            Some("error") => format!("error:{}", report["error"]["kind"].as_str().unwrap_or("?")),
            Some(s) => s.to_string(),
            None => format!("error:{}", report["error"]["kind"].as_str().unwrap_or("?")),
        };
        if got_out != want_out {
            problems.push(format!(
                "{}: output differs\n    want: {:?}\n    got:  {:?}",
                name, want_out, got_out
            ));
        }
        if got_status != want_status {
            problems.push(format!(
                "{}: status: want {}, got {} ({})",
                name,
                want_status,
                got_status,
                report["error"]["message"].as_str().unwrap_or("")
            ));
        }
        if let Some(r) = want_result {
            if report["result"].as_str() != Some(r.as_str()) {
                problems.push(format!(
                    "{}: result: want {}, got {:?}",
                    name, r, report["result"]
                ));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn several_files_are_one_program_with_per_file_positions() {
    let device = "using { /Fortnite.com/Devices }\nusing { /Verse.org/Simulation }\n\ncounter := class(creative_device):\n    @editable\n    Button : button_device = button_device{}\n    var Count : int = 0\n    OnBegin<override>()<suspends> : void =\n        Button.InteractedWithEvent.Subscribe(OnPressed)\n    OnPressed(Agent : agent) : void =\n        set Count += 1\n";
    let tests = "TestCounts()<decides><transacts> : void =\n    D := counter{}\n    SimStart(D)\n    D.Button.SimInteract(SimAddPlayer(\"Ana\"))\n    D.Count = 1\n";
    let files = vec![
        ("counter.verse".to_string(), device.to_string()),
        ("counter_test.verse".to_string(), tests.to_string()),
    ];
    let report = reverse_cli::test_files(&files, Limits::default());
    assert_eq!(report["passed"], 1, "{}", report);
    assert_eq!(
        report["diagnostics"].as_array().unwrap().len(),
        0,
        "{}",
        report
    );

    // An error in the second file is reported against that file's own lines.
    let broken = tests.replace("D.Count = 1", "D.Count = \"one\"");
    let files = vec![
        ("counter.verse".to_string(), device.to_string()),
        ("counter_test.verse".to_string(), broken),
    ];
    let report = reverse_cli::check_files(&files);
    let d = &report["diagnostics"][0];
    assert_eq!(d["file"], "counter_test.verse", "{}", report);
    assert_eq!(d["line"], 5, "{}", report);
    // The device file on its own has no warnings: nothing in it is simulation-only.
    let alone = reverse_cli::check_report(device, "counter.verse");
    assert_eq!(
        alone["diagnostics"].as_array().unwrap().len(),
        0,
        "{}",
        alone
    );
}
