# reverse

<!-- hero -->
Check Verse in milliseconds.  
Run it with no editor open.  
Test a device on a clock that never waits.  
Let an agent fix its own mistakes.
<!-- /hero -->

An independent, open-source toolchain for the **Verse** programming language, written in
Rust. It checks and runs Verse without Unreal Editor, on any OS, in milliseconds, and it is
built to be driven by AI coding agents.

> **Not affiliated with Epic Games.** Verse, Unreal Engine, UEFN and Fortnite are trademarks
> of Epic Games, Inc. See [Naming](#naming).

```
$ reverse check hello.verse
error[V3002]: `Half` is `<decides>`, so it must be called with square brackets
  --> hello.verse:6:26
  |
6 |     Print("half of 8 is {Half(8)}")
  |                          ^^^^^^^
  = help: write `Half[8]`; the call can fail, so use it in a failure context such as `if (...)`
```

## What you get

- **A checker with useful errors.** 62 diagnostic codes, each with a help line, a longer
  explanation and often a fix a tool can apply. It knows failure contexts, effect
  specifiers and the habits people bring from other languages.
- **An interpreter.** Failure, rollback and structured concurrency run on a simulated
  clock, so a round that lasts ten minutes finishes at once and behaves the same every time.
- **Tests in plain Verse.** Functions named `Test...` are the tests; comparisons are the
  assertions.
- **Stand-ins for Fortnite devices.** Buttons, triggers, timers, score managers and more
  can be pressed, started and inspected from a test, without UEFN.
- **Tools for AI agents.** An MCP server, a guide and a skill let a model write Verse,
  check it and fix its own mistakes before a person sees the code.
- **A browser playground.** The whole toolchain compiles to WebAssembly.

## Status

Version 0.1 covers the language end to end: every feature has a parse test, a check test
and a run test, and the same tests pass in the WebAssembly build.

| Works | Not yet |
| --- | --- |
| Lexer with layout (braces, indentation, inline), nested comments, string interpolation | Lossless syntax tree, formatter |
| Parser with per-statement error recovery | Tuple-destructuring parameters, qualified names other than `(super:)` |
| Checker: names, types, failure contexts, effect specifiers, classes, interfaces, enums, structs, `case` exhaustiveness, access specifiers, `<unique>` equality | Parametric classes |
| Overloaded functions and methods, extension methods, parametric functions, constructor functions, `defer`, `rational` arithmetic | Loading UEFN digest files (APIs beyond the stand-ins are not checked) |
| 62 diagnostic codes, each with help text and a longer explanation; many carry a machine-applicable fix | Persistence, `weak_map`, live variables |
| Interpreter: failure as a value, transactional rollback of `var` and field writes, classes, value-type containers | Bytecode VM (the interpreter walks the syntax tree) |
| Concurrency on a simulated clock: `<suspends>`, `Sleep`, `sync`, `race`, `rush`, `branch`, `spawn`, tasks, events with `Await` and `Subscribe`, cancellation, deadlock detection | Language server, Bevy plugin |
| Stand-ins for 16 Fortnite device types plus players, characters and the playspace, with test helpers | Transforms, teams, UI widgets, items |
| Step, call-depth and output limits; deterministic random numbers | |
| CLI (`check`, `run`, `test`, `explain`, `guide`), an MCP server, and a browser playground built to WebAssembly | |

**Nothing here has been verified against UEFN yet.** Each of the 111 conformance files is marked
`uefn-verified: no`. The semantics follow the Book of Verse; where the Book is silent or
ambiguous the choice made is listed in [docs/UNVERIFIED.md](docs/UNVERIFIED.md).

## Install and use

```
cargo build --release          # binary at target/release/reverse
reverse check file.verse      # parse and type-check; exit code 1 on errors
reverse run file.verse        # check, then run Main()
reverse test file.verse       # check, then run every Test...() function
reverse explain V3001         # what a diagnostic means, with a failing and a fixed example
reverse guide                 # the Verse guide written for AI agents
reverse mcp                   # MCP server on stdio
```

Concurrent code runs on a simulated clock, so a program that sleeps for an hour finishes at
once and behaves the same on every run:

```verse
Main()<suspends> : void =
    Winner := race:
        block:
            Sleep(3600.0)
            "the hour passed"
        block:
            Sleep(5.0)
            "five seconds passed"
    Print("{Winner} at {GetSimulationElapsedTime()}")     # five seconds passed at 5.000000
```

Add `--json` to `check`, `run` or `test` for one machine-readable report. Pass `-` as the
file to read from stdin.

Tests are ordinary Verse, so the same file compiles in UEFN. A `<decides>` test passes when
every line succeeds, which makes comparisons work as assertions. A `<suspends>` test returns
`logic` and passes when it returns `true`:

```verse
Double(X : int) : int = X * 2

TestDouble()<decides><transacts> : void =
    Double(2) = 4
    not (Double(1) = 3)
```

## Fortnite devices

A file that imports a `/Fortnite.com/...` module is checked and run against built-in
stand-ins for the most common device types. They carry the names of Epic's public API, so
the device code is plain UEFN Verse; tests drive them with helpers whose names start with
`Sim`:

```verse
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

counter_device := class(creative_device):
    @editable
    Button : button_device = button_device{}
    var Presses : int = 0

    OnBegin<override>()<suspends> : void =
        Button.InteractedWithEvent.Subscribe(OnPressed)

    OnPressed(Agent : agent) : void =
        set Presses += 1

TestPressIsCounted()<decides><transacts> : void =
    Device := counter_device{}
    SimStart(Device)
    Device.Button.SimInteract(SimAddPlayer("Ana"))
    Device.Presses = 1
```

Keep tests in a second file and run both together, `reverse test door.verse
door_test.verse`, so the device file goes to UEFN unchanged. The stand-ins are a simple
model written from the public API reference, not Epic's code, and are not yet compared with
UEFN. The list of devices and helpers is in [docs/devices.md](docs/devices.md); a full
example is in [examples/door](examples/door/door.verse).

## Using it with AI agents

Verse is a low-resource language: models have seen little of it and carry habits from
Python, C# and Swift that do not compile. The toolchain closes that gap with a loop an agent
can run by itself: read the guide once, write, check, apply fixes, test.

- **MCP server.** `reverse mcp` exposes `verse_check`, `verse_run`, `verse_test`,
  `verse_explain` and `verse_guide`. A sample client configuration is in
  [agents/mcp.json](agents/mcp.json).
- **Guide.** [agents/VERSE_GUIDE.md](agents/VERSE_GUIDE.md) lists the mistakes models make
  most and the rules behind them. Every example in it is compiled by the test suite.
- **Skill.** [agents/skills/verse/SKILL.md](agents/skills/verse/SKILL.md) packages the loop
  for agents that load skills.
- **Evals.** [evals/](evals/) holds ten tasks with hidden tests and a scorer, to measure
  whether a model writes better Verse with the tools than without.

How this fits with Epic's own tooling: UEFN ships an MCP server inside the editor that can
compile Verse against the real Fortnite APIs and run a play session. That is the final
word on anything that touches devices, players or UI. Use `reverse` for the loop that
needs no editor: logic, data structures, failure and rollback, and device behaviour against
the stand-ins, in CI, on Linux or macOS, in seconds.

## Layout

```
crates/syntax    lexer, layout, parser, AST, diagnostics       (no dependencies)
crates/check     types, failure contexts, effects, explanations (no dependencies)
crates/vm        interpreter, transactional undo log, built-ins (no dependencies)
crates/cli       command line, JSON reports, MCP server         (serde_json)
crates/wasm      three-function C ABI for the browser          (serde_json)
playground/      the browser playground: page, examples, bundler, WebAssembly tests
tests/conformance  parse/, check/ and run/ snippets with inline expectations
agents/          guide, skill and MCP configuration for AI agents
evals/           tasks and scorer for measuring model performance
examples/        complete programs, including a device with its tests
docs/            devices, diagnostics, grammar notes, unverified behaviour
site/            builds the landing page and documentation from these Markdown files
```

The three core crates have no dependencies and do no I/O. Concurrency uses plain Rust
futures driven by a small scheduler, with no threads and no clock, so the whole toolchain
compiles to WebAssembly unchanged.

## Playground

`scripts/build-wasm.sh` builds the WebAssembly module and assembles `playground/dist/`:
`index.html` with `reverse.wasm` for hosting, and `playground.html`, one self-contained
file that works from disk. `node playground/test-wasm.mjs` runs the whole conformance corpus
and the stress cases through the WebAssembly build.

## Documentation site

`python3 site/build.py` turns this README and the Markdown files under `docs/` and
`agents/` into `site/dist/index.html`: a landing page and the documentation in one static
file with no dependencies at run time. `python3 scripts/gen-docs.py` refreshes the
generated reference sections first.

## Sources and clean-room policy

This implementation is written only from public documentation:

- the [Book of Verse](https://verselang.github.io/book/) ([repository](https://github.com/verselang/book))
- Epic's public [Verse language reference](https://dev.epicgames.com/documentation/uefn/verse-language-reference)
  and [Verse API reference](https://dev.epicgames.com/documentation/fortnite/verse-api), for the
  names and signatures the device stand-ins mirror
- [The Verse Calculus](https://doi.org/10.1145/3607845) (ICFP 2023)

It uses no Epic source code. Contributors who have read Epic's Verse implementation in the
Unreal Engine source must say so and must not work on the matching components. UEFN digest
files (`*.digest.verse`) are never committed or redistributed.

## Naming

The project is called `reverse`. The name has not yet had a trademark search, and because it
contains "verse" that search should look at Epic's marks in particular. Never shorten it to
"Verse". Describe the project as "an independent implementation of the Verse language, not
affiliated with Epic Games".

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Contributions are accepted under the same terms with a `Signed-off-by` line (DCO).
