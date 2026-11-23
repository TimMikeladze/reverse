# Behaviour not yet verified against UEFN

Nothing in this repository has been run in UEFN. This file lists every place where the Book
of Verse is silent, ambiguous or self-contradictory and this toolchain had to choose. Each
entry is a candidate for a differential test: run the snippet in UEFN, record the result and
the UEFN version in the conformance file, then delete the entry.

An existing open-source project, [OpenVerse](https://github.com/rudi-cilibrasi/openverse),
records Epic's Verse VM output for a growing set of programs. If its licence permits,
those recorded outputs can settle many of these without a Windows machine.

## Runtime

| # | Behaviour | Choice made here | Why it is uncertain |
| --- | --- | --- | --- |
| 1 | Text of a float in interpolation and `ToString` | Six decimal places (`1.500000`); `Inf`, `-Inf`, `NaN` | The Book does not give the format |
| 2 | `Mod[A, B]` and `Quotient[A, B]` with negative operands | Floored: the remainder takes the divisor's sign, and `Quotient * B + Mod = A` | The Book does not cover these functions |
| 3 | `Print` inside a failure context that later fails | The line stays printed; only `var` and field writes roll back | The Book gives no effect signature for `Print` |
| 4 | Writes made by a `for` header clause when a later clause of the same iteration fails | Only the failing clause's own writes are undone | The Book says a failed iteration is skipped; it does not say how much is rolled back |
| 5 | `set A[F()] += 1` | `F()` is evaluated twice (once to read, once to write) | The Book says once; this is a known bug here, harmless unless `F` has effects |
| 6 | `set Map[Key] = Value` when the key is absent | Inserts the key | The Book shows this working and also says that modifying an absent key fails |
| 7 | Integer range | 64-bit, with a runtime error on overflow | The Book: arbitrary precision on the Verse VM, 64-bit with an error on the older VM. This is the portable subset |
| 8 | String indexing and iteration | By Unicode scalar value, while `.Length` counts UTF-8 code units | The Book defines `string` as `[]char` with `char` a UTF-8 code unit. The two agree for ASCII only |
| 9 | Default field values | Evaluated without access to `Self` or other fields | Not covered |
| 10 | Order of `block` clauses across a class hierarchy | Parent blocks first, then the subclass | The Book says the order differs between VMs and should not be relied on |
| 11 | Value of a `void` function | A distinct `void` value | The Book says it evaluates to `false` |
| 12 | Random numbers | A fixed-seed xorshift generator | By design: runs must be reproducible. The distribution matches nothing in UEFN |

## Checker

| # | Rule | Choice made here | Why it is uncertain |
| --- | --- | --- | --- |
| 13 | `not` binds tighter than comparison, so `not A = B` is `(not A) = B` | Follows the Book's precedence table | Surprising enough to confirm; write `not (A = B)` |
| 14 | Local `var` in a `<computes>` function | Declaring, reading and writing all need heap effects, so it is rejected | The Book says `<computes>` means no heap access but does not show a local `var` |
| 15 | Shadowing | A local name cannot be redefined anywhere in the same function, including inner blocks | The Book forbids redefinition; the exact scope of the rule is not spelled out |
| 16 | Comparing values of different types | Accepted with warning `V2008` | The Book: compiles and always fails |
| 17 | `if` without `else` used as a value | Type `void`; using it where a value is needed is `V2006` | The Book implies it cannot be a value |
| 18 | Module-level `var` | Accepted with warning `V6004` | UEFN only allows module-scoped `weak_map` variables |
| 19 | Effects of an override | May not add `<decides>` or `<suspends>`; heap effects are not compared | The Book: effects may not widen |
| 20 | Equality on class instances | Allowed for any class, by identity | The Book requires `<unique>`; the check is not implemented |
| 21 | Access specifiers (`<private>`, `<protected>`, ...) | Parsed and validated, not enforced | Not implemented |
| 22 | `Print` in a `<computes>` function | Rejected: `Print` is treated as `<transacts>` | The Book gives no effect signature for `Print` |
| 23 | `X?` when `X` is neither an option nor a `logic` | Accepted with warning `V3005`; at run time it yields `X` | The Book says `?` on any other value succeeds with that value; whether the compiler accepts it is not stated |
| 24 | Field defaults that call functions | Accepted | The Book says field defaults cannot call functions; the check is not implemented |
| 25 | `<concrete>` classes | The requirement that every field has a default is not checked | Not implemented |
| 26 | Diagnostic codes | This project's own (`V....`) | They are not Epic's compiler error numbers and are not meant to be |

## Concurrency

The scheduler is deterministic and single-threaded, and its clock is simulated. The Book
fixes what each construct means; it leaves the ordering details below open.

An embedded session advances only when its host supplies a tick. In that mode `Sleep(0.0)`
and negative durations resume no sooner than the next host tick. Positive sleeps use
nanosecond deadlines based on the clock supplied by the host. These are choices for
embedding the runner in an engine with a fixed timestep. An embedded failure context
discards deferred spawns it queued before failing, along with its staged host effects.
The Book describes the lifetime of a started task but leaves this speculative scheduling
case open.

| # | Behaviour | Choice made here | Why it is uncertain |
| --- | --- | --- | --- |
| 27 | Tasks that become ready at the same instant | They run in a fixed order: the entry function first, then tasks in the order they were created; inside a call, its body before its background tasks | The Book does not state an order |
| 28 | `Sleep(0.0)` and negative durations | A yield: every other task gets a turn first. If nothing else can run, the clock advances by one tick of 1/30 s | The Book says it may resume in the same tick; negative values are not covered |
| 29 | `Sleep` with an infinite or NaN duration | Sleeps until cancelled | Not covered |
| 30 | `Signal` | The awaiter is marked ready and runs at the scheduler's next round; the signalling code continues first | The Book does not say which runs first |
| 31 | Several tasks awaiting one event | The longest-waiting one is woken | The Book says one awaiter is woken, not which |
| 32 | `rush` arms after the first finishes | They keep running until the function that contains the `rush` returns | The Book says "enclosing async context" without defining it |
| 33 | `rush` when the first arm finishes without suspending | Later arms still start, in the background | Not covered |
| 34 | `branch` body | Runs at once up to its first suspension, like `spawn` | The Book says it starts immediately but not how far it runs |
| 35 | A suspending call in a failure context | Rejected with `V4006` | The Book shows `if (X := AsyncOp()?)` as invalid; the general rule is this project's reading |
| 36 | End of a run | The run ends when the entry function returns; tasks still running are cancelled, and their `defer` blocks run | A property of this runner, not of Verse |
| 37 | `spawn` reached inside a failure context | The task starts at the scheduler's next round instead of immediately | Not covered |
| 38 | `GetSimulationElapsedTime()` | Returns the simulated clock | Recalled from the UEFN API, not from the Book |
| 39 | Overload resolution at run time | By the run-time types of the arguments, first match in source order | The Book describes distinctness rules, not a resolution order |
| 40 | Members with no access specifier | Treated as `<internal>`; since one file is one module here, that never restricts anything | Module boundaries are not modelled |
| 41 | `Subscribe` handlers | Called at once, inside `Signal`, in subscription order, after the waiting task has been marked ready. A handler cancelled by an earlier handler of the same signal is skipped | The public reference says a callback is invoked each time `Signal` is called, not when or in what order |
| 42 | `Subscribe` inside a failure context that later fails | The subscription stays | Not covered; the public signature is `<transacts>`, which suggests it should roll back |
| 43 | One `Signal`, several awaiters, on a device event | One awaiter is woken (entry 31) and every subscriber is called | In UEFN every task awaiting a device event may resume; not confirmed either way |
| 44 | Overloaded methods | Chosen by the argument types, first match in declaration order, child class before parent | As entry 39 |

## Device stand-ins

The stand-ins in `crates/check/lib/fortnite.verse` take names and signatures from Epic's
public API reference. Their behaviour is this project's model. All of it is unverified; the
choices most likely to differ from UEFN are listed here.

| # | Behaviour | Choice made here | Why it is uncertain |
| --- | --- | --- | --- |
| 45 | Parameter names and effect specifiers of API methods | Taken from the public reference where the page shows them; otherwise the obvious name (`Agent`, `Time`) and no effect beyond the default | Many reference pages list members without full signatures |
| 46 | `OnBegin` under `SimStart` | Runs up to its first suspension at once; `SimEnd` calls `OnEnd` but does not cancel tasks `OnBegin` started | The reference says coroutines spawned in `OnBegin` are cancelled when the experience ends |
| 47 | `button_device` trigger limit | `0` means unlimited; once the limit is reached presses are ignored | The editor option decides this in UEFN |
| 48 | `trigger_device.Trigger` | Fires `TriggeredEvent` at once; reset and transmit delays are stored but not applied | Delays are editor options |
| 49 | `timer_device` | Counts down from `SimDuration` (default 60 s) on the simulated clock, then fires `SuccessEvent`, or `FailureEvent` when `SimSuccessOnEnd` is false; one shared state, the per-agent overloads act on it; `StartUrgencyModeEvent` never fires | Direction, duration, per-player state and urgency are editor options |
| 50 | `score_manager_device.Activate` | Adds the award to one total per agent shared by all score managers, then fires `ScoreOutputEvent`; `Increment` and `Decrement` change the award by one; `MaxTriggersEvent` never fires | Editor options decide the scoring mode |
| 51 | `fort_character` health | Damage drains shield first, then health; health reaching zero eliminates; `SetHealth` clamps to 1..max; `Heal` stops at max | Recalled from the public reference for `healthful` and `damageable`, not checked in UEFN |
| 52 | `player.IsActive[]` | Succeeds while the player is in the playspace | Not confirmed |
| 53 | `end_game_device.Activate` | Records the first activating agent as the winner; later calls are ignored | The device's team and winner options are not modelled |
| 54 | `switch_device` | One shared on/off state; the per-agent overload reads it | Per-player state is an editor option |
| 55 | `teleporter_device` | `Activate` and `Teleport` record the agent and fire `EnterEvent` or `TeleportedEvent`; nothing moves | Positions are not modelled |
| 56 | `hud_message_device.Show` | Records the text; display time and per-agent targeting are not modelled | Display is not modelled |
| 57 | `log.Print` | Prints the message as `Print` does; levels and channels are ignored | Log routing is not modelled |
