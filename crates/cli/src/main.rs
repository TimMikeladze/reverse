use reverse_cli::*;
use reverse_vm::Limits;
use std::process::ExitCode;

const USAGE: &str = "\
reverse: an independent toolchain for the Verse language (not affiliated with Epic Games)

USAGE:
    reverse check <file.verse>... [--json]    parse and type-check; exit 1 on errors
    reverse run <file.verse>... [options]     check, then run `Main()`
    reverse test <file.verse>... [--json]     check, then run every `Test...()` function
    reverse explain <CODE>                    explain a diagnostic code, with a fixed example
    reverse guide                             print the Verse guide written for AI agents
    reverse mcp                               serve the tools over MCP (stdio)

OPTIONS:
    --json               print one JSON report instead of text
    --entry <Name>       function to run (default: Main)
    --max-steps <N>      stop after N evaluation steps (default: 5000000)
    --seed <N>           seed for GetRandomInt and friends (default: fixed)
    -                    read the source from stdin instead of a file

Several files are checked together as one program, for example a device
and a separate file of tests: reverse test door.verse door_test.verse
Files that import /Fortnite.com modules get built-in stand-ins for common
devices (button_device, trigger_device, ...); see docs/devices.md.

EXIT CODES:  0 success   1 check errors, failed tests or a runtime error   2 bad usage
";

struct Opts {
    files: Vec<String>,
    json: bool,
    entry: Option<String>,
    limits: Limits,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        files: Vec::new(),
        json: false,
        entry: None,
        limits: Limits::default(),
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let mut value = |name: &str| -> Result<String, String> {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{} needs a value", name))
        };
        match a.as_str() {
            "--json" => o.json = true,
            "--entry" => o.entry = Some(value("--entry")?),
            "--max-steps" => {
                o.limits.max_steps = value("--max-steps")?
                    .parse()
                    .map_err(|_| "--max-steps needs a number".to_string())?
            }
            "--seed" => {
                o.limits.seed = value("--seed")?
                    .parse()
                    .map_err(|_| "--seed needs a number".to_string())?
            }
            s if s.starts_with("--") => return Err(format!("unknown option {}", s)),
            _ => o.files.push(a.clone()),
        }
        i += 1;
    }
    Ok(o)
}

fn read_source(file: &str) -> Result<String, String> {
    if file == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
            .map_err(|e| format!("cannot read stdin: {}", e))?;
        Ok(s)
    } else {
        std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {}", file, e))
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        print!("{}", USAGE);
        return ExitCode::from(2);
    };
    let usage_error = |msg: String| {
        eprintln!("error: {}\n\n{}", msg, USAGE);
        ExitCode::from(2)
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{}", USAGE);
            ExitCode::SUCCESS
        }
        "--version" | "-V" => {
            println!("reverse {}", VERSION);
            ExitCode::SUCCESS
        }
        "guide" => {
            print!("{}", GUIDE);
            ExitCode::SUCCESS
        }
        "mcp" => {
            mcp::serve_stdio();
            ExitCode::SUCCESS
        }
        "explain" => {
            let Some(code) = args.get(1) else {
                return usage_error("explain needs a code, for example V3002".into());
            };
            let r = explain_report(code);
            if args.iter().any(|a| a == "--json") {
                println!("{}", r);
            } else {
                print!("{}", render_explain(&r));
            }
            if r["ok"] == true {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "check" | "run" | "test" => {
            let o = match parse_opts(&args[1..]) {
                Ok(o) => o,
                Err(e) => return usage_error(e),
            };
            if o.files.is_empty() {
                return usage_error(format!("{} needs a file", cmd));
            }
            let mut files: Vec<(String, String)> = Vec::new();
            for file in &o.files {
                match read_source(file) {
                    Ok(src) => {
                        let name = if file == "-" {
                            "<stdin>"
                        } else {
                            file.as_str()
                        };
                        files.push((name.to_string(), src));
                    }
                    Err(e) => {
                        eprintln!("error: {}", e);
                        return ExitCode::from(2);
                    }
                }
            }
            let report = match cmd.as_str() {
                "check" => check_files(&files),
                "run" => run_files(&files, o.entry.as_deref(), o.limits, !o.json),
                _ => test_files(&files, o.limits),
            };
            if o.json {
                println!("{}", report);
            } else {
                let text = match cmd.as_str() {
                    "check" => render_check(&report),
                    "run" => render_run(&report, false),
                    _ => render_test(&report),
                };
                print!("{}", text);
            }
            if report["ok"] == true {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        other => usage_error(format!("unknown command {}", other)),
    }
}
