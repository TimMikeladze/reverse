# Reverse Bevy implementation plan

**Goal**  Run entity-attached Verse gameplay in Bevy 0.20, with the timed door as the first end-to-end test.

**Design**  [Reverse in Bevy](bevy-integration.md)

1. Add focused failing tests for a host function's type checking and rollback, an entry that sleeps across host ticks, and a door that loads through Bevy's asset system. Keep existing language semantics covered by the conformance corpus.
2. Add `reverse_check::analyze_with_host` for registered function signatures. Add `Program::load_with_host`, `Vm::new_with_host`, and a persistent `Session` in `reverse-vm`. The host receives nested rollback checkpoints. A session advances on supplied time, runs queued calls, and retains tasks across ticks. The CLI and WebAssembly keep their run-to-completion clock.
3. Add `reverse-bevy` with the `ReversePlugin`, `.verse` assets, a script component, and registration for typed ECS properties and incoming Bevy messages. Bind each VM to its owning entity. Snapshot inputs and properties before stepping, then apply staged writes and publish notifications in order. Keep the previous running version when a replacement script fails to check or initialize.
4. Ship a headless timed-door example with a test for closing after two fixed ticks and for a successful handler whose failed inner condition leaves ECS state and messages unchanged. Cover checker errors and invalid asset replacement.
5. Verify `cargo test --workspace`, the timed-door example, and the existing WebAssembly build. Document the actual host API and how reload treats Verse task state.
