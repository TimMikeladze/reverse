//! The toolchain must never panic, hang or overflow the stack, whatever it
//! is given: agents will feed it broken and adversarial input all day.
//!
//! This mutates every corpus file thousands of times (deletions, swaps,
//! insertions of syntax-heavy fragments, truncation) and checks and runs
//! each mutant under a small step budget.

use reverse_cli::{check_report, run_report, test_report};
use reverse_vm::Limits;
use std::path::Path;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const FRAGMENTS: &[&str] = &[
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    ":",
    ":=",
    "=",
    "\n",
    "\n    ",
    "\n        ",
    "\"",
    "'",
    "<#",
    "#>",
    "#",
    "?",
    "..",
    ",",
    ";",
    "<decides>",
    "<suspends>",
    " if ",
    " else ",
    " for ",
    " loop:",
    " not ",
    " or ",
    " and ",
    "set ",
    "var ",
    "class",
    "enum",
    "array{",
    "map{",
    "option{",
    "{}",
    "[]",
    "()",
    "/",
    "//",
    "==",
    "!",
    "\\",
    "\t",
    "é",
    "0x",
    "1.",
    "9999999999999999999999",
    "Main",
    "Self",
    "(super:)",
    "=>",
    "->",
    "@",
    "<",
    ">",
    "return ",
    "break",
    " then ",
    "case (",
    "_",
    "\"{",
    "}\"",
];

fn mutate(src: &str, rng: &mut Rng) -> String {
    let mut chars: Vec<char> = src.chars().collect();
    let edits = 1 + rng.below(4);
    for _ in 0..edits {
        if chars.is_empty() {
            break;
        }
        let at = rng.below(chars.len());
        match rng.below(6) {
            0 => {
                chars.remove(at);
            }
            1 => {
                let end = (at + 1 + rng.below(20)).min(chars.len());
                chars.drain(at..end);
            }
            2 => {
                let frag = FRAGMENTS[rng.below(FRAGMENTS.len())];
                for (i, c) in frag.chars().enumerate() {
                    chars.insert(at + i, c);
                }
            }
            3 => {
                let other = rng.below(chars.len());
                chars.swap(at, other);
            }
            4 => chars.truncate(at),
            _ => {
                let end = (at + 1 + rng.below(30)).min(chars.len());
                let piece: Vec<char> = chars[at..end].to_vec();
                let dest = rng.below(chars.len());
                for (i, c) in piece.into_iter().enumerate() {
                    chars.insert(dest + i, c);
                }
            }
        }
    }
    chars.into_iter().collect()
}

#[test]
fn mutated_programs_never_crash_the_toolchain() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let mut sources = Vec::new();
    for dir in ["parse", "check", "run"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().map(|e| e == "verse").unwrap_or(false) {
                sources.push(std::fs::read_to_string(path).unwrap());
            }
        }
    }
    sources.push(reverse_cli::GUIDE.to_string());
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let limits = Limits {
        max_steps: 20_000,
        max_output_bytes: 4096,
        ..Limits::default()
    };
    let rounds: usize = std::env::var("REVERSE_FUZZ_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let mut ran = 0usize;
    for src in &sources {
        for _ in 0..rounds {
            let m = mutate(src, &mut rng);
            let report = check_report(&m, "fuzz.verse");
            assert_ne!(report["stage"], "internal", "checker panicked on:\n{}", m);
            // Every diagnostic must carry a position inside the file.
            for d in report["diagnostics"].as_array().unwrap() {
                assert!(d["line"].as_u64().unwrap() >= 1, "bad position in {:?}", d);
            }
            if report["ok"] == true {
                let r = run_report(&m, "fuzz.verse", None, limits.clone(), false);
                assert_ne!(r["stage"], "internal", "interpreter panicked on:\n{}", m);
                let t = test_report(&m, "fuzz.verse", limits.clone());
                assert_ne!(t["stage"], "internal", "interpreter panicked on:\n{}", m);
                ran += 1;
            }
        }
    }
    assert!(
        ran > 50,
        "expected some mutants to survive checking and run, got {}",
        ran
    );
}

#[test]
fn pathological_nesting_is_an_error_not_a_crash() {
    let n = 50_000;
    let cases = vec![
        format!("F() : int = {}1{}", "(".repeat(n), ")".repeat(n)),
        format!("F() : int = {}1", "-".repeat(n)),
        format!("F() : int = 1{}", " + 1".repeat(n)),
        format!("F(X : {}int) : void = {{}}", "[]".repeat(n)),
        format!(
            "F() : int = {}1{}",
            "if (1 = 1) then ".repeat(n),
            " else 0".repeat(n)
        ),
        format!("F() : int = {}1{}", "{".repeat(n), "}".repeat(n)),
        format!(
            "F() : string = {}1{}",
            "\"{".repeat(3000),
            "}\"".repeat(3000)
        ),
        format!("F(X : int) : int = X{}", ".Y".repeat(n)),
        format!("F(X : int) : int = X{}", "()".repeat(n)),
        format!("F(X : ?int)<decides> : int = X{}", "?".repeat(n)),
    ];
    for src in cases {
        let report = check_report(&src, "deep.verse");
        assert_ne!(
            report["stage"],
            "internal",
            "crashed on input starting {:?}",
            &src[..40]
        );
        assert_eq!(report["ok"], false);
        assert!(
            report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "V0008"),
            "expected V0008 for input starting {:?}, got {}",
            &src[..40],
            report["diagnostics"]
        );
    }
}
