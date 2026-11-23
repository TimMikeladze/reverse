---
name: verse
description: Write, check, run and test Verse code (Epic Games' language for UEFN and Unreal Engine 6) with the reverse toolchain. Use whenever the task involves a .verse file or Verse code, before showing any Verse to the user.
---

# Writing Verse

Verse differs from mainstream languages in ways that are easy to get wrong from memory.
Do not rely on memory: follow this loop.

## Setup, once per session

1. Read the guide: run `reverse guide`, or call the `verse_guide` tool. It is short and
   lists the ten mistakes models make most often.
2. Confirm the tool works: `reverse --version`. If it is missing, build it with
   `cargo build --release` in the reverse repository and use `target/release/reverse`.

## The loop

1. Write or edit the `.verse` file.
2. Check it: `reverse check <file> --json` (tool: `verse_check`).
   - Fix every diagnostic with `"severity": "error"`. Start with the first one.
   - When a diagnostic has a `fix`, apply it exactly: replace the text between
     `start_offset` and `end_offset` with `replacement`.
   - When the `help` text is not enough, run `reverse explain <CODE>` (tool:
     `verse_explain`) for an example that raises the code and its corrected form.
   - Check again. Repeat until `"ok": true`.
3. Test behaviour. Add functions named `Test...` that take no arguments; make them
   `<decides><transacts>` so each comparison is an assertion:

   ```verse
   TestClamp()<decides><transacts> : void =
       Clamp(5, 0, 10) = 5
       Clamp(-1, 0, 10) = 0
   ```

   Run `reverse test <file> --json` (tool: `verse_test`). Fix until every test passes.
4. To see output, give the file a `Main() : void` and run `reverse run <file>`
   (tool: `verse_run`).
5. Only then show the code to the user. Say that it passed `reverse check` and which tests
   ran.

## Limits to state honestly

- Common Fortnite devices (`creative_device`, buttons, triggers, timers, score managers and
  more; the guide lists them) are checked and run against built-in stand-ins that mirror
  the public API by name. Put device code in one file and tests in another, drive the
  devices with `Sim...` helpers (`SimAddPlayer`, `SimStart`, `Button.SimInteract`), and run
  both files together: `reverse test device.verse device_test.verse`. Only the device
  file goes to UEFN. The stand-ins are a simple model, so tell the user the logic passed
  here and still has to be played in UEFN.
- Other Fortnite and Unreal Engine APIs produce `V6002` warnings and are not checked. Tell
  the user that this part must be compiled in UEFN.
- `<suspends>` code runs here on a simulated clock: `Sleep`, `sync`, `race`, `rush`,
  `branch`, `spawn`, tasks and events all work, and a run that sleeps for an hour finishes
  at once. Write a `<suspends>` test as a function returning `logic`.
- A `V6001` error means reverse does not support a construct yet. The code may still be
  valid in UEFN; say so instead of rewriting valid code to please the checker.
- reverse has not been verified against UEFN. Passing its checks makes compile errors in
  UEFN much less likely, not impossible.
