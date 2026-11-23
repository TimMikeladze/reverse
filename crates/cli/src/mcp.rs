//! A Model Context Protocol server over stdio.
//!
//! Written directly against JSON-RPC 2.0 with newline-delimited messages, so
//! the only dependency is `serde_json`. It exposes tools only; everything is
//! computed in-process and nothing touches the network.

use crate::*;
use reverse_vm::Limits;
use serde_json::{json, Value as Json};
use std::io::{BufRead, Write};

const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Tools for writing Verse (Epic Games' language) without Unreal Editor. \
Call verse_guide once before writing Verse: it lists the mistakes language models make most often. \
After writing or editing Verse, always call verse_check and fix every error before showing the code; \
each diagnostic has a code, a help sentence and often a fix. Use verse_test to run `Test...()` functions \
and verse_run to execute `Main()`. Files that import /Fortnite.com modules are checked and run against built-in stand-ins for common devices (button_device, trigger_device, timer_device and more); tests drive them with `Sim...` helpers such as `SimAddPlayer(\"Ana\")` and `Button.SimInteract(Agent)`. API names the stand-ins do not model are not checked (warning V6002).";

fn source_schema(extra: Json) -> Json {
    let mut props = json!({
        "source": { "type": "string", "description": "Verse source code. Give this or `path`." },
        "path": { "type": "string", "description": "Path of a .verse file to read instead of `source`." },
        "files": {
            "type": "array",
            "description": "More files checked together with `source`/`path` as one program, in order. Use it to keep tests (which may use `Sim...` helpers) in a separate file from the device code that goes to UEFN.",
            "items": { "type": "object", "properties": {
                "name": { "type": "string" },
                "source": { "type": "string" },
                "path": { "type": "string" }
            } }
        },
    });
    if let (Some(p), Some(e)) = (props.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            p.insert(k.clone(), v.clone());
        }
    }
    json!({ "type": "object", "properties": props })
}

pub fn tool_list() -> Json {
    json!([
        {
            "name": "verse_check",
            "title": "Check Verse code",
            "description": "Parse and type-check Verse source: names, types, failure contexts and effect specifiers. Returns diagnostics with a stable code, exact position, a help sentence and, where possible, a machine-applicable fix. Runs in milliseconds and never executes the code. Call it after every edit.",
            "inputSchema": source_schema(json!({})),
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false },
        },
        {
            "name": "verse_run",
            "title": "Run Verse code",
            "description": "Check the source, then run a function that takes no arguments (default `Main`) in a sandboxed interpreter with a step limit. Returns the lines printed by Print, the result value, and any runtime error with its line. Nothing runs if checking finds errors. `<suspends>` code cannot be run yet.",
            "inputSchema": source_schema(json!({
                "entry": { "type": "string", "description": "Function to run. Default: Main." },
                "max_steps": { "type": "integer", "description": "Evaluation step budget. Default: 5000000." },
                "seed": { "type": "integer", "description": "Seed for GetRandomInt, GetRandomFloat and Shuffle." },
            })),
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false },
        },
        {
            "name": "verse_test",
            "title": "Run Verse tests",
            "description": "Check the source, then run every function named `Test...` that takes no arguments, each in a fresh interpreter. A test passes when it returns; a `<decides>` test passes when every line succeeds, so comparisons act as assertions, for example `TestAdd()<decides><transacts> : void = Add(1, 2) = 3`.",
            "inputSchema": source_schema(json!({
                "max_steps": { "type": "integer", "description": "Evaluation step budget per test. Default: 5000000." },
            })),
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false },
        },
        {
            "name": "verse_explain",
            "title": "Explain a diagnostic",
            "description": "Explain a diagnostic code such as V3002, with a short example that raises it and the corrected version.",
            "inputSchema": {
                "type": "object",
                "properties": { "code": { "type": "string", "description": "Diagnostic code, for example V3001." } },
                "required": ["code"],
            },
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false },
        },
        {
            "name": "verse_guide",
            "title": "Verse guide for agents",
            "description": "A compact guide to writing correct Verse: the ten mistakes to avoid, failure contexts, effects, types, control flow, classes and the testing convention. Every example in it passes verse_check. Read it once before writing Verse.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false },
        },
    ])
}

fn load_sources(args: &Json) -> Result<Vec<(String, String)>, String> {
    let one = |item: &Json, default_name: &str| -> Result<Option<(String, String)>, String> {
        if let Some(src) = item.get("source").and_then(|s| s.as_str()) {
            let name = item
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(default_name);
            return Ok(Some((name.to_string(), src.to_string())));
        }
        if let Some(p) = item.get("path").and_then(|p| p.as_str()) {
            return std::fs::read_to_string(p)
                .map(|s| Some((p.to_string(), s)))
                .map_err(|e| format!("cannot read {}: {}", p, e));
        }
        Ok(None)
    };
    let mut files = Vec::new();
    if let Some(f) = one(args, "input.verse")? {
        files.push(f);
    }
    if let Some(list) = args.get("files").and_then(|f| f.as_array()) {
        for (i, item) in list.iter().enumerate() {
            match one(item, &format!("file{}.verse", i + 1))? {
                Some(f) => files.push(f),
                None => return Err("each entry of `files` needs `source` or `path`".into()),
            }
        }
    }
    if files.is_empty() {
        return Err("give `source` (Verse code), `path` (a .verse file) or `files`".to_string());
    }
    Ok(files)
}

fn limits_from(args: &Json) -> Limits {
    let mut l = Limits::default();
    if let Some(n) = args.get("max_steps").and_then(|n| n.as_u64()) {
        l.max_steps = n.clamp(1, 200_000_000);
    }
    if let Some(n) = args.get("seed").and_then(|n| n.as_u64()) {
        l.seed = n;
    }
    l
}

/// Run a tool. Returns the text for the model and the structured report.
pub fn call_tool(name: &str, args: &Json) -> Result<(String, Json), String> {
    match name {
        "verse_check" => {
            let r = check_files(&load_sources(args)?);
            Ok((render_check(&r), r))
        }
        "verse_run" => {
            let entry = args.get("entry").and_then(|e| e.as_str());
            let r = run_files(&load_sources(args)?, entry, limits_from(args), false);
            let mut text = render_run(&r, true);
            if r["ok"] == true && text.trim().is_empty() {
                text = "ran to completion with no output\n".into();
            }
            Ok((text, r))
        }
        "verse_test" => {
            let r = test_files(&load_sources(args)?, limits_from(args));
            Ok((render_test(&r), r))
        }
        "verse_explain" => {
            let code = args
                .get("code")
                .and_then(|c| c.as_str())
                .ok_or("`code` is required, for example V3002")?;
            let r = explain_report(code);
            Ok((render_explain(&r), r))
        }
        "verse_guide" => Ok((GUIDE.to_string(), json!({ "ok": true }))),
        other => Err(format!("unknown tool `{}`", other)),
    }
}

fn response(id: &Json, result: Json) -> Json {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Json, code: i64, message: &str) -> Json {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Handle one JSON-RPC message. Notifications produce no reply.
pub fn handle_message(line: &str) -> Option<Json> {
    let msg: Json = match serde_json::from_str(line) {
        Ok(m) => m,
        Err(e) => {
            return Some(rpc_error(
                &Json::Null,
                -32700,
                &format!("parse error: {}", e),
            ))
        }
    };
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let Some(id) = msg.get("id").cloned() else {
        return None; // a notification
    };
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    Some(match method {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                PROTOCOL_VERSIONS[0]
            };
            response(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "reverse", "title": "reverse Verse toolchain", "version": VERSION },
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => response(&id, json!({})),
        "tools/list" => response(&id, json!({ "tools": tool_list() })),
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            // A panic in a tool must not take the server down.
            let outcome = std::panic::catch_unwind(|| call_tool(name, &args));
            match outcome {
                Ok(Ok((text, report))) => response(
                    &id,
                    json!({
                        "content": [{ "type": "text", "text": text }],
                        "structuredContent": report,
                        "isError": false,
                    }),
                ),
                Ok(Err(message)) => response(
                    &id,
                    json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
                ),
                Err(_) => response(
                    &id,
                    json!({
                        "content": [{ "type": "text", "text": "internal error in the Verse toolchain while handling this input; please report the source that caused it" }],
                        "isError": true,
                    }),
                ),
            }
        }
        _ => rpc_error(&id, -32601, &format!("method not found: {}", method)),
    })
}

pub fn serve_stdio() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle_message(&line) {
            if writeln!(stdout, "{}", reply).is_err() || stdout.flush().is_err() {
                break;
            }
        }
    }
}
