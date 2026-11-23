#!/usr/bin/env python3
"""Regenerate the generated parts of the documentation.

  docs/devices.md      the reference between the `generated:reference` markers,
                       from crates/check/lib/fortnite.verse
  docs/diagnostics.md  every diagnostic code, from `reverse explain`

Run from the repository root after changing the stand-ins or the diagnostics:

    cargo build && python3 scripts/gen-docs.py
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIB = ROOT / "crates/check/lib/fortnite.verse"
BIN = ROOT / "target/debug/reverse"

MEMBER = re.compile(r"^    (?:var )?([A-Za-z_]\w*)<public>(.*)$")


def signature(name, rest):
    """The declaration without its body or default value."""
    if rest.startswith("("):
        depth, close = 0, 0
        for i, ch in enumerate(rest):
            depth += ch == "("
            depth -= ch == ")"
            if depth == 0:
                close = i
                break
        head, tail = rest[: close + 1], rest[close + 1 :]
        rest = head + tail.split(" =")[0]
    else:
        rest = rest.split(" =")[0]
    return (name + rest).replace("<transacts>", "").rstrip()


def device_reference():
    module = "sim"
    types = []  # (module, name, kind, [(sig, is_sim, note)])
    current = None
    note = ""
    for line in LIB.read_text().splitlines():
        if line.startswith("#@ "):
            module = line[3:].strip()
            continue
        stripped = line.strip()
        if stripped.startswith("#"):
            note = stripped.lstrip("# ").strip() if line.startswith("    #") else ""
            continue
        m = re.match(r"^([a-z_]\w*)<public> := (class|struct|enum)", line)
        if m:
            current = (module, m.group(1), m.group(2), [])
            types.append(current)
            note = ""
            continue
        if line and not line.startswith(" "):
            current = None
        m = MEMBER.match(line)
        if m and current:
            name = m.group(1)
            current[3].append((signature(name, m.group(2)), name.startswith("Sim"), note))
        note = ""

    out = []
    by_module = {}
    for t in types:
        by_module.setdefault(t[0], []).append(t)
    for mod, items in by_module.items():
        out.append(f"### `{mod}`\n")
        for _, name, kind, members in items:
            api = [s for s, sim, _ in members if not sim]
            sims = [(s, n) for s, sim, n in members if sim and n]
            if kind == "enum" or (not api and not sims):
                out.append(f"**`{name}`** ({kind})\n")
                continue
            out.append(f"**`{name}`**\n")
            if api:
                out.append("```\n" + "\n".join(api) + "\n```\n")
            for s, n in sims:
                out.append(f"- *test helper* `{s}`: {n}")
            if sims:
                out.append("")
    return "\n".join(out).rstrip() + "\n"


def write_devices():
    path = ROOT / "docs/devices.md"
    text = path.read_text()
    start, end = "<!-- generated:reference -->", "<!-- /generated -->"
    a, b = text.index(start) + len(start), text.index(end)
    path.write_text(text[:a] + "\n" + device_reference() + text[b:])


def write_diagnostics():
    codes = sorted(set(re.findall(r'code: "(V\d{4})"', (ROOT / "crates/check/src/explain.rs").read_text())))
    out = [
        "# Diagnostics",
        "",
        "Every problem the checker reports has a stable code. `reverse explain <CODE>` prints",
        "the same text as this page, and the `verse_explain` tool returns it to an agent.",
        "Codes marked *warning* do not stop a program from running.",
        "",
        "| Range | Area |",
        "| --- | --- |",
        "| `V0xxx` | Syntax and layout |",
        "| `V1xxx` | Names and definitions |",
        "| `V2xxx` | Types and calls |",
        "| `V3xxx` | Failure contexts |",
        "| `V4xxx` | Effects, specifiers and concurrency |",
        "| `V5xxx` | Assignment and control flow |",
        "| `V6xxx` | Unsupported constructs and Fortnite APIs |",
        "| `V7xxx` | Inheritance and access |",
        "| `V8xxx` | `case` |",
        "| `V9xxx` | Structure and unreachable code |",
        "",
    ]
    for code in codes:
        r = json.loads(subprocess.run([str(BIN), "explain", code, "--json"], capture_output=True, text=True).stdout)
        out.append(f"## {code}: {r['title']}\n")
        out.append(r["explanation"] + "\n")
        if r.get("example_with_error"):
            out.append("Raises it:\n\n```verse\n" + r["example_with_error"].rstrip() + "\n```\n")
            out.append("Fixed:\n\n```verse\n" + r["example_fixed"].rstrip() + "\n```\n")
    (ROOT / "docs/diagnostics.md").write_text("\n".join(out).rstrip() + "\n")
    return len(codes)


if __name__ == "__main__":
    if not BIN.exists():
        sys.exit("build first: cargo build")
    write_devices()
    n = write_diagnostics()
    print(f"docs/devices.md reference and docs/diagnostics.md ({n} codes) regenerated")
