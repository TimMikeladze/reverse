#!/usr/bin/env python3
"""Assemble the playground from its template, the examples and the wasm module.

Writes two versions into playground/dist:
  index.html + reverse.wasm   for hosting (the page fetches the module)
  playground.html              one self-contained file with the module inlined
"""
import base64
import json
from pathlib import Path

here = Path(__file__).resolve().parent
dist = here / "dist"
dist.mkdir(exist_ok=True)

TITLES = {
    "01-hello": "Basics: values, loops, strings",
    "02-failure": "Failure and rollback",
    "03-concurrency": "Concurrency: sync, race, spawn",
    "04-device": "A device with events, tested",
    "05-mistakes": "Common mistakes, with fixes",
    "06-tests": "Tests, extension methods, generics",
}

examples = [
    {"title": TITLES.get(p.stem, p.stem), "source": p.read_text()}
    for p in sorted((here / "examples").glob("*.verse"))
]
payload = json.dumps(examples).replace("</", "<\\/")
template = (here / "index.html").read_text()
assert "/*EXAMPLES*/[]" in template and '"/*WASM_BASE64*/"' in template
page = template.replace("/*EXAMPLES*/[]", payload)

(dist / "index.html").write_text(page)
wasm = (dist / "reverse.wasm").read_bytes()
inlined = page.replace('"/*WASM_BASE64*/"', '"' + base64.b64encode(wasm).decode() + '"')
(dist / "playground.html").write_text(inlined)
print(f"index.html {len(page):,} bytes, reverse.wasm {len(wasm):,} bytes, playground.html {len(inlined):,} bytes")
