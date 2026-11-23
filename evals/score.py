#!/usr/bin/env python3
"""Score a set of candidate Verse solutions against the eval tasks.

    python3 evals/score.py <candidates_dir> [--reverse path/to/reverse] [--json]

`<candidates_dir>` holds one file per task, named `<task>.verse`. Each candidate is
concatenated with the task's hidden tests and run with `reverse test`. A task counts
as solved only when the code checks without errors and every test passes.
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def find_reverse(argv):
    if "--reverse" in argv:
        return argv[argv.index("--reverse") + 1]
    for candidate in (ROOT.parent / "target/release/reverse", ROOT.parent / "target/debug/reverse"):
        if candidate.exists():
            return str(candidate)
    return "reverse"


def main(argv):
    if len(argv) < 2 or argv[1].startswith("-"):
        print(__doc__)
        return 2
    candidates = Path(argv[1])
    reverse = find_reverse(argv)
    results = []
    for task in sorted(p for p in (ROOT / "tasks").iterdir() if p.is_dir()):
        candidate = candidates / f"{task.name}.verse"
        if not candidate.exists():
            results.append({"task": task.name, "solved": False, "reason": "no solution file"})
            continue
        source = candidate.read_text() + "\n" + (task / "tests.verse").read_text()
        with tempfile.NamedTemporaryFile("w", suffix=".verse", delete=False) as f:
            f.write(source)
        proc = subprocess.run([reverse, "test", f.name, "--json"], capture_output=True, text=True, timeout=120)
        Path(f.name).unlink(missing_ok=True)
        try:
            report = json.loads(proc.stdout)
        except json.JSONDecodeError:
            results.append({"task": task.name, "solved": False, "reason": "toolchain produced no report"})
            continue
        if report.get("stage") == "check":
            codes = sorted({d["code"] for d in report["diagnostics"] if d["severity"] == "error"})
            reason = "does not compile: " + " ".join(codes)
        elif report["ok"]:
            reason = f"{report['passed']} tests passed"
        else:
            failing = [t["name"] for t in report["tests"] if t["status"] != "passed"]
            reason = "failing tests: " + ", ".join(failing)
        results.append({"task": task.name, "solved": bool(report["ok"]), "reason": reason})
    solved = sum(r["solved"] for r in results)
    if "--json" in argv:
        print(json.dumps({"solved": solved, "total": len(results), "tasks": results}, indent=2))
    else:
        for r in results:
            print(f"{'PASS' if r['solved'] else 'FAIL'}  {r['task']:<16} {r['reason']}")
        print(f"\n{solved}/{len(results)} solved")
    return 0 if solved == len(results) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
