# Instructions for AI agents working on this repository

This repository implements the Verse language. Read this before changing anything.

## Hard rules

1. **Clean room.** Use only the Book of Verse (https://verselang.github.io/book/), Epic's
   public Verse documentation and the Verse Calculus paper. Never read, search for, quote or
   paraphrase Unreal Engine or UE6 source code, including Epic's Verse compiler and VM. If a
   search result leads there, close it and say so in your summary.
2. **Never commit digest files.** `*.digest.verse` files belong to Epic and ship with UEFN.
3. **Never mark a test `uefn-verified: yes`.** Only a human who ran the snippet in UEFN may,
   and they record the UEFN version next to it.
4. **Stand-ins are original code.** `crates/check/lib/fortnite.verse` mirrors the names and
   signatures in Epic's public Verse API reference pages and nothing else. Write the bodies
   yourself. Never copy text from a digest file, and never add a member you have not seen
   on a public reference page. Everything that is not in the public API starts with `Sim`.
5. **Do not guess semantics.** If the Book does not settle a behaviour, implement the
   narrowest reasonable reading and add an entry to `docs/UNVERIFIED.md`.

## How a change is made

The conformance corpus is the specification. Work in this order:

1. Add or edit snippets in `tests/conformance/` that state the behaviour. Cite the Book
   chapter in the `# source:` header.
   - `parse/` and `check/`: mark each expected diagnostic with a trailing `#! CODE`
     (or `#!- CODE` on the next line when the line cannot hold a comment).
   - `run/`: give expected output as `#> line` comments and, if not a normal return,
     `#= status: failed` or `#= status: error:<kind>`.
2. Run `cargo test -p reverse-cli --test conformance` and watch the new snippet fail.
3. Implement: `crates/syntax` for syntax, `crates/check` for static rules, `crates/vm` for
   behaviour. Keep these three crates free of dependencies and I/O.
4. For a new diagnostic code: add an entry to `crates/check/src/explain.rs` with an example
   that raises it and a fixed version. A test compiles both.
5. If the change affects how Verse should be written, update `agents/VERSE_GUIDE.md`. Every
   `verse` block in it is compiled by the tests.
6. Run `cargo test --workspace`. All of it must pass, including the mutation test, which
   feeds thousands of broken programs through the toolchain and fails on any panic.

## Diagnostics are the product

An agent reads every message this toolchain prints, so each diagnostic must:

- have a stable code and point at the exact span;
- say what is wrong in terms of the user's code, not the checker's internals;
- carry a `help` sentence that shows the Verse way to write it;
- carry a `fix` when one replacement resolves it (a test applies each fix and re-checks);
- appear once: report the root cause and suppress the errors it would cascade into.

When you see a new mistake that language models make in Verse, turn it into a targeted
diagnostic and a conformance snippet. That is the highest-value change in this repository.

## Useful commands

```
cargo test --workspace                                   # everything
cargo test -p reverse-cli --test conformance            # corpus only
REVERSE_FUZZ_ROUNDS=1500 cargo test --release -p reverse-cli --test robustness
cargo run -q -p reverse-cli -- check path/to/file.verse
python3 evals/score.py <dir-of-solutions>                # score solutions to the eval tasks
scripts/build-wasm.sh && node playground/test-wasm.mjs   # WebAssembly build, then the corpus through it
```

## Concurrency

Suspending code runs as Rust futures in `crates/vm/src/conc.rs`; everything else stays in
the synchronous evaluator. Two rules keep this sound: never hold a borrow of the machine
across an `.await`, and give every concurrent child its own scope with `Env::child()`.
The clock is simulated, so a test may assert exact times.
