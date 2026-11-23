// Run the conformance corpus and the stress cases through the WebAssembly
// build, to confirm it behaves like the native one.
//
//   node playground/test-wasm.mjs [path/to/reverse.wasm]
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath = process.argv[2] ?? join(root, "target/wasm32-unknown-unknown/wasm/reverse_wasm.wasm");
const module = await WebAssembly.compile(readFileSync(wasmPath));

let instance = await WebAssembly.instantiate(module, {});
async function call(tool, args) {
  const x = instance.exports;
  const bytes = new TextEncoder().encode(JSON.stringify({ tool, arguments: args }));
  try {
    const ptr = x.q_alloc(bytes.length);
    new Uint8Array(x.memory.buffer, ptr, bytes.length).set(bytes);
    const out = x.q_call(ptr, bytes.length);
    x.q_free(ptr, bytes.length);
    const len = new DataView(x.memory.buffer).getUint32(out, true);
    const text = new TextDecoder().decode(new Uint8Array(x.memory.buffer, out + 4, len));
    x.q_free(out, len + 4);
    return JSON.parse(text);
  } catch (e) {
    // A trap leaves the instance unusable; start a fresh one.
    instance = await WebAssembly.instantiate(module, {});
    return { ok: false, trapped: true, text: String(e) };
  }
}

let failures = 0;
const fail = (msg) => { failures++; console.log("FAIL " + msg); };

// 1. Every run-corpus file must print what its `#>` lines say.
const dir = join(root, "tests/conformance/run");
let ran = 0;
for (const name of readdirSync(dir).filter((f) => f.endsWith(".verse")).sort()) {
  const source = readFileSync(join(dir, name), "utf8");
  const want = source.split("\n").map((l) => l.trimStart()).filter((l) => l === "#>" || l.startsWith("#> ")).map((l) => l.slice(3));
  const wantStatus = (source.match(/^#= status: (.+)$/m) ?? [null, "returned"])[1].trim();
  // The call-depth limit is lower in WebAssembly; that file tests the native limit.
  const r = await call("verse_run", { source, max_steps: 2000000 });
  if (r.trapped) { fail(`${name}: trapped: ${r.text}`); continue; }
  const rep = r.report;
  const got = rep.output ?? [];
  const status = rep.status === "error" || !rep.status ? "error:" + (rep.error?.kind ?? "?") : rep.status;
  if (JSON.stringify(got) !== JSON.stringify(want)) fail(`${name}: output\n  want ${JSON.stringify(want)}\n  got  ${JSON.stringify(got)}`);
  if (status !== wantStatus) fail(`${name}: status want ${wantStatus}, got ${status}`);
  ran++;
}

// 2. Check-corpus files: every `#! CODE` marker must be reported.
for (const sub of ["parse", "check"]) {
  const d = join(root, "tests/conformance", sub);
  for (const name of readdirSync(d).filter((f) => f.endsWith(".verse")).sort()) {
    const source = readFileSync(join(d, name), "utf8");
    const r = await call("verse_check", { source });
    if (r.trapped) { fail(`${sub}/${name}: trapped`); continue; }
    source.split("\n").forEach((line, i) => {
      const m = line.match(/#! (.+)$/);
      if (!m || line.trim().startsWith("#!-")) return;
      for (const code of m[1].trim().split(/\s+/)) {
        if (!r.report.diagnostics.some((x) => x.line === i + 1 && x.code === code)) fail(`${sub}/${name}:${i + 1}: expected ${code}`);
      }
    });
    ran++;
  }
}

// 3. Hostile input must come back as an error, never as a trap.
const n = 50000;
const stress = {
  "nested parens": `F() : int = ${"(".repeat(n)}1${")".repeat(n)}`,
  "long chain": `F() : int = 1${" + 1".repeat(n)}`,
  "nested ifs": `F() : int = ${"if (1 = 1) then ".repeat(n)}1${" else 0".repeat(n)}`,
  "nested types": `F(X : ${"[]".repeat(n)}int) : void = {}`,
  "runaway recursion": "Down(N : int) : int = Down(N + 1)\nMain() : void = Print(\"{Down(0)}\")",
  "runaway loop": "Main() : void =\n    loop:\n        X := 1\n",
  "runaway suspending recursion": "Down(N : int)<suspends> : int =\n    Sleep(0.0)\n    Down(N + 1)\nMain()<suspends> : void = Print(\"{Down(0)}\")",
};
for (const [label, source] of Object.entries(stress)) {
  const r = await call("verse_run", { source, max_steps: 300000 });
  if (r.trapped) fail(`${label}: trapped: ${r.text}`);
  else if (r.report.ok) fail(`${label}: unexpectedly succeeded`);
}

const guide = await call("verse_guide", {});
if (!guide.ok || !guide.text.includes("Verse")) fail("guide");

console.log(`${ran} corpus files and ${Object.keys(stress).length} stress cases through WebAssembly; ${failures} failure(s)`);
process.exit(failures ? 1 : 0);
