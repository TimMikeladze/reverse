//! Every explanation's "bad" example must raise its code, and every "good"
//! example must be free of errors.

use reverse_check::{analyze, CODES};
use reverse_syntax::Severity;

#[test]
fn explanations_match_the_checker() {
    let mut problems = Vec::new();
    for ex in CODES {
        if !ex.bad.is_empty() {
            let (_, r) = analyze(ex.bad);
            if !r.diagnostics.iter().any(|d| d.code == ex.code) {
                let got: Vec<_> = r
                    .diagnostics
                    .iter()
                    .map(|d| format!("{} {}", d.code, d.message))
                    .collect();
                problems.push(format!(
                    "{}: bad example does not raise it; got {:?}",
                    ex.code, got
                ));
            }
        }
        if !ex.good.is_empty() {
            let (_, r) = analyze(ex.good);
            let errs: Vec<_> = r
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| format!("{} {}", d.code, d.message))
                .collect();
            if !errs.is_empty() {
                problems.push(format!("{}: good example has errors: {:?}", ex.code, errs));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn every_fix_applies_cleanly() {
    // Applying a diagnostic's fix must remove that diagnostic.
    for ex in CODES {
        if ex.bad.is_empty() {
            continue;
        }
        let (_, r) = analyze(ex.bad);
        for d in r.diagnostics.iter().filter(|d| d.code == ex.code) {
            let Some(fix) = &d.fix else { continue };
            let mut fixed = ex.bad.to_string();
            fixed.replace_range(
                fix.span.start as usize..fix.span.end as usize,
                &fix.replacement,
            );
            let (_, r2) = analyze(&fixed);
            assert!(
                !r2.diagnostics
                    .iter()
                    .any(|x| x.code == ex.code && x.span.start == d.span.start),
                "{}: fix did not resolve it.\nfixed source:\n{}\nstill: {:?}",
                ex.code,
                fixed,
                r2.diagnostics
                    .iter()
                    .map(|d| format!("{} {}", d.code, d.message))
                    .collect::<Vec<_>>()
            );
        }
    }
}
