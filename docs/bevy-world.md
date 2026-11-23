# Signal Observatory

[Play Signal Observatory in the browser](https://reverse-sepia.vercel.app/observatory/).

```sh
cargo run --locked --manifest-path examples/bevy-world/Cargo.toml
```

Move with **WASD** or the arrow keys. Press **E** next to a station or use its number
key from anywhere. The console displays actual Verse output, committed ECS writes,
outgoing Bevy messages, and source diagnostics.

1. **Press 2 before charging the relay.** The gate script stages an open, a label
   change, and a message before it checks authorization. The closed gate and unchanged
   counters show what the failure context rolled back.
2. **Press 1, then 2.** The relay adds a charge. The gate opens for three seconds on
   Bevy's fixed clock. Walk through the opening while it is open.
3. **Press 3 to scan.** The beacon advances a float progress value while two receivers
   race for a signal. The losing receiver cleans up with `defer`, the scan waits for both
   branches of `sync`, and a detached `spawn` publishes an archive message after the
   input handler returns.
4. **Press R to reload the beacon.** The first edit switches to AURORA and speeds up
   its scan. The next edit reports `V2001` and keeps AURORA running. Press **R** again
   to restore ORBIT.

Each station loads a `.verse` asset and runs in its own VM. Bevy supplies the targeted
input, owns the entity's component data, and renders the resulting state. Verse reads
and writes registered `logic`, `int`, `float`, and `string` properties and publishes
typed Bevy messages after commit. The beacon's `OnStart` updates its label after a
successful reload while keeping the scan counter on the ECS component.

The reload controls edit a temporary copy of `beacon.verse` through Bevy's file
watcher. The versioned files are under [assets](../examples/bevy-world/assets).

## Test the stations

```sh
cargo test --locked --manifest-path examples/bevy-world/Cargo.toml
```

The tests run the station scripts with `ReversePlugin` on a headless Bevy schedule.
The windowed app has its own Cargo workspace so renderer dependencies stay out of the
toolchain's workspace builds.

## Browser preview

The browser build uses the same Bevy scene, runtime, and Verse assets. It replaces the
beacon asset in memory when you press **R**. The native app uses the file watcher.

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.130
bash examples/bevy-world/build-browser.sh
python3 -m http.server --bind 127.0.0.1 --directory examples/bevy-world/target/browser-dist 8080
```

Open `http://127.0.0.1:8080` in a WebGL2 browser. Click the game canvas if the
keyboard controls need focus.

## Edit a station in the browser

Select the relay, gate, or beacon in the editor and apply the Verse draft. The demo
checks the draft with the station's Bevy bindings and activates it at a fixed tick
boundary. A rejected draft shows the diagnostic and leaves the running station in
place. Restore the bundled source from the editor when you want to start over.
