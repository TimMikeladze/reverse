//! Built-in stand-ins for common Fortnite API types.
//!
//! The stand-ins are Verse source written for this project from Epic's public
//! API reference (see `lib/fortnite.verse`). When a file imports a
//! `/Fortnite.com/...` module they are appended to it, so device code checks
//! and runs without UEFN. Definitions the file writes itself take precedence.

use std::collections::{HashMap, HashSet};

pub const FORTNITE: &str = include_str!("../lib/fortnite.verse");

/// One top-level definition of the library.
pub struct Chunk {
    /// Module the definition belongs to in UEFN, or `sim` for test helpers.
    pub module: &'static str,
    pub name: &'static str,
    pub text: &'static str,
}

fn starts_definition(line: &str) -> bool {
    line.chars()
        .next()
        .is_some_and(|c| c == '(' || c == '_' || c.is_ascii_alphabetic())
}

/// The name a top-level line defines: `name := ...`, `Name(...) ...` or the
/// method name of an extension `(X : t).Name(...)`.
fn defined_name(line: &str) -> &str {
    let rest = if line.starts_with('(') {
        match line.find(").") {
            Some(i) => &line[i + 2..],
            None => return "",
        }
    } else {
        line
    };
    let end = rest
        .find(|c: char| !(c == '_' || c.is_ascii_alphanumeric()))
        .unwrap_or(rest.len());
    &rest[..end]
}

pub fn chunks() -> Vec<Chunk> {
    let src: &'static str = FORTNITE;
    let mut out: Vec<Chunk> = Vec::new();
    let mut module: &'static str = "sim";
    let mut start: Option<(usize, &'static str, &'static str)> = None;
    let mut pos = 0;
    for line in src.split_inclusive('\n') {
        let at = pos;
        pos += line.len();
        if let Some(m) = line.strip_prefix("#@ ") {
            module = m.trim();
            continue;
        }
        if starts_definition(line) {
            if let Some((s, name, m)) = start.take() {
                out.push(Chunk {
                    module: m,
                    name,
                    text: &src[s..at],
                });
            }
            start = Some((at, defined_name(line), module));
        }
    }
    if let Some((s, name, m)) = start {
        out.push(Chunk {
            module: m,
            name,
            text: &src[s..],
        });
    }
    out
}

/// Names a source file defines at the top level.
pub fn top_level_names(src: &str) -> HashSet<&str> {
    src.lines()
        .filter(|l| starts_definition(l) && !l.starts_with("using"))
        .map(defined_name)
        .filter(|n| !n.is_empty())
        .collect()
}

/// True when the file imports a module the stand-ins cover.
pub fn wanted(src: &str) -> bool {
    src.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("using") && l.contains("/Fortnite.com")
    })
}

pub struct Library {
    /// Source text to append after the user's file.
    pub text: String,
    /// Library definition name to the module it lives in.
    pub modules: HashMap<&'static str, &'static str>,
    /// Top-level function names the library defines.
    pub functions: HashSet<&'static str>,
}

/// The library for a file, leaving out anything the file defines itself.
pub fn library_for(src: &str) -> Library {
    let own = top_level_names(src);
    let mut text = String::new();
    let mut modules = HashMap::new();
    let mut functions = HashSet::new();
    for c in chunks() {
        if own.contains(c.name) {
            continue;
        }
        text.push_str(c.text);
        if !text.ends_with('\n') {
            text.push('\n');
        }
        modules.insert(c.name, c.module);
        if c.name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
            functions.insert(c.name);
        }
    }
    Library {
        text,
        modules,
        functions,
    }
}

/// True for names that exist only in this toolchain's simulation.
pub fn is_sim_name(name: &str) -> bool {
    name.strip_prefix("Sim")
        .and_then(|r| r.chars().next())
        .is_some_and(|c| c.is_ascii_uppercase())
}
