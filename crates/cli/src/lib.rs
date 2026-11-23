//! Shared entry points for the command line and the MCP server.
//!
//! Every operation returns a `serde_json::Value` with a stable shape, so an
//! agent sees the same report whether it shells out or speaks MCP.

pub mod mcp;

use reverse_check::analyze;
use reverse_syntax::{Diagnostic, LineIndex, Severity};
use reverse_vm::{Limits, Outcome, Program, RuntimeError, Vm};
use serde_json::{json, Value as Json};
use std::rc::Rc;

/// The language guide written for AI agents.
pub const GUIDE: &str = include_str!("../../../agents/VERSE_GUIDE.md");

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn diag_json(d: &Diagnostic, src: &str, li: &LineIndex, file: &str) -> Json {
    let (line, col) = li.line_col(d.span.start);
    let (eline, ecol) = li.line_col(d.span.end.max(d.span.start));
    let mut j = json!({
        "code": d.code,
        "severity": d.severity.as_str(),
        "message": d.message,
        "file": file,
        "line": line,
        "column": col,
        "end_line": eline,
        "end_column": ecol,
        "source_line": li.line_text(src, line),
    });
    if let Some(h) = &d.help {
        j["help"] = json!(h);
    }
    if let Some(f) = &d.fix {
        let (fl, fc) = li.line_col(f.span.start);
        let (fel, fec) = li.line_col(f.span.end);
        j["fix"] = json!({
            "line": fl, "column": fc, "end_line": fel, "end_column": fec,
            "start_offset": f.span.start, "end_offset": f.span.end,
            "replacement": f.replacement,
        });
    }
    j
}

fn diags_json(diags: &[Diagnostic], src: &str, file: &str) -> Vec<Json> {
    let li = LineIndex::new(src);
    diags.iter().map(|d| diag_json(d, src, &li, file)).collect()
}

fn counts(diags: &[Diagnostic]) -> (usize, usize) {
    let e = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    (e, diags.len() - e)
}

/// Several files checked as one program, in order. A device and its tests
/// usually live in two files so the device file can go to UEFN unchanged.
struct Joined {
    text: String,
    /// Per file: name, first line (0-based) and first byte in `text`.
    starts: Vec<(String, u64, u64)>,
}

fn join(files: &[(String, String)]) -> Joined {
    let mut text = String::new();
    let mut starts = Vec::new();
    let mut line = 0u64;
    for (name, src) in files {
        starts.push((name.clone(), line, text.len() as u64));
        text.push_str(src);
        if !src.ends_with('\n') {
            text.push('\n');
        }
        line = text.bytes().filter(|b| *b == b'\n').count() as u64;
    }
    Joined { text, starts }
}

impl Joined {
    fn file_of(&self, line: u64) -> &(String, u64, u64) {
        self.starts
            .iter()
            .rev()
            .find(|(_, first, _)| line > *first)
            .unwrap_or(&self.starts[0])
    }

    /// Rewrite positions in a report from the joined text to each file.
    fn remap(&self, j: &mut Json) {
        match j {
            Json::Array(items) => items.iter_mut().for_each(|i| self.remap(i)),
            Json::Object(map) => {
                if let Some(line) = map.get("line").and_then(|l| l.as_u64()) {
                    let (name, first, offset) = self.file_of(line).clone();
                    for key in ["line", "end_line"] {
                        if let Some(n) = map.get(key).and_then(|l| l.as_u64()) {
                            map.insert(key.into(), json!(n.saturating_sub(first)));
                        }
                    }
                    for key in ["start_offset", "end_offset"] {
                        if let Some(n) = map.get(key).and_then(|l| l.as_u64()) {
                            map.insert(key.into(), json!(n.saturating_sub(offset)));
                        }
                    }
                    if map.contains_key("file") || map.contains_key("kind") {
                        map.insert("file".into(), json!(name));
                    }
                }
                map.values_mut().for_each(|v| self.remap(v));
            }
            _ => {}
        }
    }
}

fn with_files(files: &[(String, String)], f: impl FnOnce(&str, &str) -> Json) -> Json {
    match files {
        [] => f("", "input.verse"),
        [(name, src)] => f(src, name),
        many => {
            let joined = join(many);
            let mut report = f(&joined.text, &many[0].0);
            joined.remap(&mut report);
            report
        }
    }
}

/// `check_report` for a program split over several files.
pub fn check_files(files: &[(String, String)]) -> Json {
    with_files(files, check_report)
}

/// `run_report` for a program split over several files.
pub fn run_files(
    files: &[(String, String)],
    entry: Option<&str>,
    limits: Limits,
    echo: bool,
) -> Json {
    with_files(files, |src, file| {
        run_report(src, file, entry, limits, echo)
    })
}

/// `test_report` for a program split over several files.
pub fn test_files(files: &[(String, String)], limits: Limits) -> Json {
    with_files(files, |src, file| test_report(src, file, limits))
}

/// Parse and check. `ok` is true when there are no errors (warnings allowed).
pub fn check_report(src: &str, file: &str) -> Json {
    let (src, file) = (src.to_string(), file.to_string());
    on_big_stack(move || check_impl(&src, &file))
}

fn check_impl(src: &str, file: &str) -> Json {
    let (_, result) = analyze(src);
    let (errors, warnings) = counts(&result.diagnostics);
    json!({
        "ok": errors == 0,
        "errors": errors,
        "warnings": warnings,
        "diagnostics": diags_json(&result.diagnostics, src, file),
        "functions": result.functions.iter().map(|f| f.signature.clone()).collect::<Vec<_>>(),
    })
}

fn runtime_error_json(e: &RuntimeError, src: &str) -> Json {
    let li = LineIndex::new(src);
    let mut j = json!({ "kind": e.kind, "message": e.message });
    if let Some(sp) = e.span {
        if sp.start as usize > src.len() {
            // The failing line is inside the built-in device stand-ins.
            j["message"] = json!(format!(
                "{} (inside the built-in stand-in for a Fortnite type)",
                e.message
            ));
            return j;
        }
        let (line, col) = li.line_col(sp.start);
        j["line"] = json!(line);
        j["column"] = json!(col);
        j["source_line"] = json!(li.line_text(src, line));
    }
    j
}

/// Parsing, checking and interpreting all recurse on the native stack, so
/// every report is computed on a thread with a deep one. A panic there is a
/// bug in this toolchain; it is reported instead of crashing the caller.
#[cfg(target_arch = "wasm32")]
fn on_big_stack(f: impl FnOnce() -> Json + Send + 'static) -> Json {
    // No threads in the browser; the module is linked with a large stack
    // and the limits are lower (see `Limits::default`).
    f()
}

#[cfg(not(target_arch = "wasm32"))]
fn on_big_stack(f: impl FnOnce() -> Json + Send + 'static) -> Json {
    let internal = || {
        json!({
            "ok": false, "stage": "internal", "diagnostics": [], "output": [], "tests": [], "passed": 0, "failed": 0,
            "error": { "kind": "internal", "message": "internal error in the Verse toolchain; please report the input that caused it" },
        })
    };
    match std::thread::Builder::new().stack_size(1 << 30).spawn(f) {
        Ok(handle) => handle.join().unwrap_or_else(|_| internal()),
        Err(_) => internal(),
    }
}

/// Check, then run `entry` (default `Main`). Nothing runs if checking fails.
pub fn run_report(src: &str, file: &str, entry: Option<&str>, limits: Limits, echo: bool) -> Json {
    let (src, file, entry) = (
        src.to_string(),
        file.to_string(),
        entry.map(|e| e.to_string()),
    );
    on_big_stack(move || run_impl(&src, &file, entry.as_deref(), limits, echo))
}

fn run_impl(src: &str, file: &str, entry: Option<&str>, limits: Limits, echo: bool) -> Json {
    let (module, result) = analyze(src);
    let (errors, warnings) = counts(&result.diagnostics);
    let diagnostics = diags_json(&result.diagnostics, src, file);
    if errors > 0 {
        return json!({
            "ok": false, "stage": "check", "errors": errors, "warnings": warnings,
            "diagnostics": diagnostics, "output": [],
        });
    }
    let entry = entry.unwrap_or("Main").to_string();
    let Some(f) = result.functions.iter().find(|f| f.name == entry) else {
        let available: Vec<&str> = result
            .functions
            .iter()
            .filter(|f| f.required_params == 0)
            .map(|f| f.name.as_str())
            .collect();
        return json!({
            "ok": false, "stage": "run", "diagnostics": diagnostics, "output": [],
            "error": {
                "kind": "no_entry",
                "message": format!("no function named `{}` to run", entry),
                "help": format!("define `{}() : void = ...`, or choose an entry point; functions that take no arguments: {}", entry, if available.is_empty() { "(none)".to_string() } else { available.join(", ") }),
            },
        });
    };
    if f.required_params > 0 {
        return json!({
            "ok": false, "stage": "run", "diagnostics": diagnostics, "output": [],
            "error": { "kind": "no_entry", "message": format!("`{}` takes arguments, so it cannot be an entry point", f.signature) },
        });
    }
    let src_owned = src.to_string();
    let run = (move || {
        let prog = Rc::new(Program::load(&module));
        let mut vm = match Vm::new(prog, limits) {
            Ok(vm) => vm,
            Err(e) => return (Vec::new(), 0, 0.0, Err(e), "error"),
        };
        vm.echo = echo;
        let outcome = vm.run_entry(&entry);
        let steps = vm.steps;
        let now = vm.now;
        let out = std::mem::take(&mut vm.output);
        match outcome {
            Outcome::Value(v) => (out, steps, now, Ok(Some(v.repr())), "returned"),
            Outcome::Failed => (out, steps, now, Ok(None), "failed"),
            Outcome::Error(e) => (out, steps, now, Err(e), "error"),
        }
    })();
    let (output, steps, now, result, status) = run;
    let mut j = json!({
        "ok": status == "returned",
        "stage": "run",
        "status": status,
        "warnings": warnings,
        "diagnostics": diagnostics,
        "output": output,
        "steps": steps,
        "simulated_seconds": now,
    });
    match result {
        Ok(Some(v)) => j["result"] = json!(v),
        Ok(None) => {
            j["error"] = json!({ "kind": "failed", "message": "the entry point is `<decides>` and it failed; its effects were rolled back" })
        }
        Err(e) => j["error"] = runtime_error_json(&e, &src_owned),
    }
    j
}

/// Run every function whose name starts with `Test` and takes no arguments.
/// A test passes when it returns; a `<decides>` test passes when it succeeds.
pub fn test_report(src: &str, file: &str, limits: Limits) -> Json {
    let (src, file) = (src.to_string(), file.to_string());
    on_big_stack(move || test_impl(&src, &file, limits))
}

fn test_impl(src: &str, file: &str, limits: Limits) -> Json {
    let (module, result) = analyze(src);
    let (errors, warnings) = counts(&result.diagnostics);
    let diagnostics = diags_json(&result.diagnostics, src, file);
    if errors > 0 {
        return json!({
            "ok": false, "stage": "check", "errors": errors, "warnings": warnings,
            "diagnostics": diagnostics, "tests": [], "passed": 0, "failed": 0,
        });
    }
    let names: Vec<String> = result
        .functions
        .iter()
        .filter(|f| f.name.starts_with("Test") && f.required_params == 0)
        .map(|f| f.name.clone())
        .collect();
    let src_owned = src.to_string();
    let tests: Vec<Json> = {
        let prog = Rc::new(Program::load(&module));
        let mut out = Vec::new();
        for name in names {
            // A fresh interpreter per test keeps tests independent.
            let mut vm = match Vm::new(prog.clone(), limits.clone()) {
                Ok(vm) => vm,
                Err(e) => {
                    out.push(json!({ "name": name, "status": "error", "error": runtime_error_json(&e, &src_owned), "output": [] }));
                    continue;
                }
            };
            let outcome = vm.run_entry(&name);
            let output = std::mem::take(&mut vm.output);
            out.push(match outcome {
                // A test that returns `logic` passes only when it returns true;
                // this is how `<suspends>` tests, which cannot fail, assert.
                Outcome::Value(reverse_vm::Value::Logic(false)) => json!({
                    "name": name, "status": "failed",
                    "message": "the test returned false",
                    "output": output,
                }),
                Outcome::Value(_) => json!({ "name": name, "status": "passed", "output": output }),
                Outcome::Failed => json!({
                    "name": name, "status": "failed",
                    "message": "the test function failed (a comparison, query or `[]` call in its body did not succeed)",
                    "output": output,
                }),
                Outcome::Error(e) => json!({ "name": name, "status": "error", "error": runtime_error_json(&e, &src_owned), "output": output }),
            });
        }
        out
    };
    let passed = tests.iter().filter(|t| t["status"] == "passed").count();
    let failed = tests
        .iter()
        .filter(|t| t["status"] == "failed" || t["status"] == "error")
        .count();
    json!({
        "ok": failed == 0 && !tests.is_empty(),
        "stage": "run",
        "warnings": warnings,
        "diagnostics": diagnostics,
        "tests": tests,
        "passed": passed,
        "failed": failed,
    })
}

pub fn explain_report(code: &str) -> Json {
    match reverse_check::explain(code) {
        Some(e) => {
            let mut j =
                json!({ "ok": true, "code": e.code, "title": e.title, "explanation": e.text });
            if !e.bad.is_empty() {
                j["example_with_error"] = json!(e.bad);
                j["example_fixed"] = json!(e.good);
            }
            j
        }
        None => json!({
            "ok": false,
            "message": format!("unknown diagnostic code `{}`", code),
            "known_codes": reverse_check::CODES.iter().map(|e| e.code).collect::<Vec<_>>(),
        }),
    }
}

// ---------------------------------------------------------------- text rendering

fn render_diag(d: &Json) -> String {
    let mut s = String::new();
    let line = d["line"].as_u64().unwrap_or(1);
    let col = d["column"].as_u64().unwrap_or(1) as usize;
    let end_col = if d["end_line"] == d["line"] {
        d["end_column"].as_u64().unwrap_or(col as u64 + 1) as usize
    } else {
        col + 1
    };
    let width = line.to_string().len();
    s.push_str(&format!(
        "{}[{}]: {}\n",
        d["severity"].as_str().unwrap_or("error"),
        d["code"].as_str().unwrap_or(""),
        d["message"].as_str().unwrap_or("")
    ));
    s.push_str(&format!(
        "{:w$}--> {}:{}:{}\n",
        "",
        d["file"].as_str().unwrap_or(""),
        line,
        col,
        w = width + 1
    ));
    let text = d["source_line"].as_str().unwrap_or("").replace('\t', " ");
    s.push_str(&format!("{:w$} |\n", "", w = width));
    s.push_str(&format!("{} | {}\n", line, text));
    s.push_str(&format!(
        "{:w$} | {}{}\n",
        "",
        " ".repeat(col.saturating_sub(1)),
        "^".repeat(end_col.saturating_sub(col).max(1)),
        w = width
    ));
    if let Some(h) = d["help"].as_str() {
        s.push_str(&format!("{:w$} = help: {}\n", "", h, w = width));
    }
    s
}

pub fn render_diagnostics(report: &Json) -> String {
    let mut s = String::new();
    if let Some(ds) = report["diagnostics"].as_array() {
        for d in ds {
            s.push_str(&render_diag(d));
            s.push('\n');
        }
    }
    s
}

fn render_runtime_error(e: &Json) -> String {
    let mut s = format!(
        "error[{}]: {}\n",
        e["kind"].as_str().unwrap_or("runtime_error"),
        e["message"].as_str().unwrap_or("")
    );
    if let (Some(line), Some(text)) = (e["line"].as_u64(), e["source_line"].as_str()) {
        s.push_str(&format!("  at line {}: {}\n", line, text.trim()));
    }
    if let Some(h) = e["help"].as_str() {
        s.push_str(&format!("  = help: {}\n", h));
    }
    s
}

pub fn render_check(report: &Json) -> String {
    let mut s = render_diagnostics(report);
    let (e, w) = (
        report["errors"].as_u64().unwrap_or(0),
        report["warnings"].as_u64().unwrap_or(0),
    );
    if e == 0 {
        s.push_str(&format!(
            "ok: no errors{}\n",
            if w > 0 {
                format!(", {} warning(s)", w)
            } else {
                String::new()
            }
        ));
    } else {
        s.push_str(&format!("{} error(s), {} warning(s)\n", e, w));
    }
    s
}

/// Text for `run`. With `include_output`, printed lines are repeated (the
/// CLI echoes them live instead).
pub fn render_run(report: &Json, include_output: bool) -> String {
    let mut s = render_diagnostics(report);
    if report["stage"] == "check" {
        s.push_str(&format!(
            "{} error(s); nothing was run\n",
            report["errors"].as_u64().unwrap_or(0)
        ));
        return s;
    }
    if include_output {
        if let Some(lines) = report["output"].as_array() {
            for l in lines {
                s.push_str(l.as_str().unwrap_or(""));
                s.push('\n');
            }
        }
    }
    if let Some(e) = report.get("error") {
        s.push_str(&render_runtime_error(e));
    } else if let Some(r) = report["result"].as_str() {
        if r != "void" {
            s.push_str(&format!("=> {}\n", r));
        }
    }
    s
}

pub fn render_test(report: &Json) -> String {
    let mut s = render_diagnostics(report);
    if report["stage"] == "check" {
        s.push_str(&format!(
            "{} error(s); no tests were run\n",
            report["errors"].as_u64().unwrap_or(0)
        ));
        return s;
    }
    let tests = report["tests"].as_array().cloned().unwrap_or_default();
    if tests.is_empty() {
        s.push_str("no tests found: define functions named `Test...` that take no arguments, for example\n    TestAdd()<decides><transacts> : void = Add(1, 2) = 3\n");
        return s;
    }
    for t in &tests {
        let status = t["status"].as_str().unwrap_or("");
        s.push_str(&format!(
            "{:7} {}\n",
            status,
            t["name"].as_str().unwrap_or("")
        ));
        if status != "passed" {
            if let Some(lines) = t["output"].as_array() {
                for l in lines {
                    s.push_str(&format!("        | {}\n", l.as_str().unwrap_or("")));
                }
            }
            if let Some(m) = t["message"].as_str() {
                s.push_str(&format!("        {}\n", m));
            }
            if let Some(e) = t.get("error") {
                for line in render_runtime_error(e).lines() {
                    s.push_str(&format!("        {}\n", line));
                }
            }
        }
    }
    s.push_str(&format!(
        "{} passed, {} failed\n",
        report["passed"], report["failed"]
    ));
    s
}

pub fn render_explain(report: &Json) -> String {
    if report["ok"] != true {
        return format!(
            "{}\nknown codes: {}\n",
            report["message"].as_str().unwrap_or(""),
            report["known_codes"]
                .as_array()
                .map(|a| a
                    .iter()
                    .filter_map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(" "))
                .unwrap_or_default()
        );
    }
    let mut s = format!(
        "{}: {}\n\n{}\n",
        report["code"].as_str().unwrap_or(""),
        report["title"].as_str().unwrap_or(""),
        report["explanation"].as_str().unwrap_or("")
    );
    if let (Some(bad), Some(good)) = (
        report["example_with_error"].as_str(),
        report["example_fixed"].as_str(),
    ) {
        s.push_str("\nRaises it:\n");
        for l in bad.lines() {
            s.push_str(&format!("    {}\n", l));
        }
        s.push_str("\nFixed:\n");
        for l in good.lines() {
            s.push_str(&format!("    {}\n", l));
        }
    }
    s
}
