#!/usr/bin/env python3
"""Build the landing page and documentation into one static file.

    python3 site/build.py            # writes site/dist/index.html

Everything on the site comes from Markdown already in the repository: the
README supplies the landing page and "Get started"; the other pages are the
files named in site/site.json. There is nothing to edit here to change the
words on the site: edit the Markdown and build again.

The output is a single HTML file with its styles and a few lines of script
inline, so it can be opened from disk or served by any static host. Pages
are switched with a plain `#name` in the address.

Needs Python 3.9+ and the `markdown` package (pip install markdown).
"""
import html
import json
import os
import re
import sys
from pathlib import Path

try:
    import markdown
    from markdown.extensions.toc import slugify as default_slugify
except ImportError:
    sys.exit("the site builder needs the `markdown` package: pip install markdown")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CONFIG = json.loads((HERE / "site.json").read_text())
# The hosted site serves the playground beside it; other copies of the page
# (for example one published elsewhere on its own) can point at another URL.
if os.environ.get("SITE_PLAYGROUND_URL"):
    CONFIG["playground_url"] = os.environ["SITE_PLAYGROUND_URL"]


def external_link(url, label, cls=None):
    """A link with an arrow when it leaves the site."""
    attr = f' class="{cls}"' if cls else ""
    if re.match(r"^[a-z]+://", url):
        return f'<a{attr} href="{url}" rel="noopener">{label} <span aria-hidden="true">↗</span></a>'
    return f'<a{attr} href="{url}">{label}</a>'

KEYWORDS = (
    "if|then|else|for|loop|break|return|var|set|class|struct|enum|interface|module|using|"
    "case|block|sync|race|rush|branch|spawn|defer|not|and|or|where|do|array|map|option|"
    "logic|true|false|Self|super|type|event"
)
VERSE_TOKEN = re.compile(
    r'(?P<str>"(?:[^"\\\n]|\\.)*")'
    r"|(?P<com>#.*$)"
    r"|(?P<spec><[a-z_]+>)"
    r"|(?P<kw>\b(?:" + KEYWORDS + r")\b)",
    re.M,
)


def highlight_verse(code):
    out, pos = [], 0
    for m in VERSE_TOKEN.finditer(code):
        out.append(html.escape(code[pos : m.start()]))
        out.append(f'<span class="t-{m.lastgroup}">{html.escape(m.group())}</span>')
        pos = m.end()
    out.append(html.escape(code[pos:]))
    return "".join(out)


def highlight_terminal(code):
    lines = []
    for line in code.split("\n"):
        esc = html.escape(line)
        if line.startswith("$ "):
            esc = f'<span class="t-prompt">$</span>{html.escape(line[1:])}'
        elif re.match(r"error(\[|\b)", line):
            esc = re.sub(r"^(error(?:\[V\d+\])?)", r'<span class="t-err">\1</span>', esc)
        elif re.match(r"warning(\[|\b)", line):
            esc = re.sub(r"^(warning(?:\[V\d+\])?)", r'<span class="t-warn">\1</span>', esc)
        elif re.match(r"\s*=\s*help:", line):
            esc = f'<span class="t-help">{esc}</span>'
        elif re.match(r"\s*\|\s*\^+\s*$", line):
            esc = esc.replace("^", '<span class="t-err">^</span>')
        lines.append(esc)
    return "\n".join(lines)


FENCE = re.compile(r"^```([a-z]*)\n(.*?)^```[ \t]*$", re.M | re.S)


def render(text, slug, source):
    """Markdown to HTML. Returns (html, [(id, heading text)] for the h2s)."""
    blocks = []

    def stash(m):
        lang, code = m.group(1), m.group(2).rstrip("\n")
        if lang == "verse":
            body = highlight_verse(code)
        elif lang == "":
            body = highlight_terminal(code)
        else:
            body = html.escape(code)
        blocks.append(f'<pre class="code {lang or "plain"}"><code>{body}</code></pre>')
        return f"\n\nCODEBLOCK{len(blocks) - 1}END\n\n"

    text = FENCE.sub(stash, text)

    def slugify(value, sep):
        return f"{slug}-{default_slugify(value, sep)}".replace("_", "-")

    md = markdown.Markdown(
        extensions=["tables", "toc", "sane_lists", "fenced_code"],
        extension_configs={"toc": {"slugify": slugify}},
    )
    out = md.convert(text)
    out = re.sub(r"<p>CODEBLOCK(\d+)END</p>", lambda m: blocks[int(m.group(1))], out)
    out = out.replace("<table>", '<div class="table"><table>').replace("</table>", "</table></div>")
    out = re.sub(r'<a href="([^"]+)">(.*?)</a>', lambda m: link(m, slug, source), out, flags=re.S)
    toc = []
    for top in md.toc_tokens:
        for tok in [top] + top.get("children", []):
            if tok["level"] == 2:
                toc.append((tok["id"], tok["name"]))
    return out, toc


def link(m, slug, source):
    href, label = m.group(1), m.group(2)
    if re.match(r"^[a-z]+:", href):
        return f'<a href="{href}" rel="noopener">{label}</a>'
    if href.startswith("#"):
        # A heading of the same file; the landing page borrows the README's.
        owner = next((p["slug"] for p in CONFIG["pages"] if p["source"] == source), slug)
        return f'<a href="#{owner}-{href[1:]}">{label}</a>'
    target = (ROOT / source).parent.joinpath(href.split("#")[0]).resolve()
    try:
        rel = target.relative_to(ROOT).as_posix()
    except ValueError:
        rel = href
    for page in CONFIG["pages"]:
        if page["source"] == rel:
            return f'<a href="#{page["slug"]}">{label}</a>'
    if CONFIG.get("repo_url"):
        kind = "tree" if target.is_dir() else "blob"
        return f'<a href="{CONFIG["repo_url"]}/{kind}/main/{rel}" rel="noopener">{label}</a>'
    # No public repository yet: name the file instead of linking nowhere.
    return f'<span class="file" title="{html.escape(rel)} in the repository">{label}</span>'


def sections(text):
    """Split Markdown into (intro, [(heading, body)]) at level-2 headings."""
    parts = re.split(r"^## +(.+)$", text, flags=re.M)
    intro, rest = parts[0], parts[1:]
    return intro, [(rest[i].strip(), rest[i + 1]) for i in range(0, len(rest), 2)]


def build_home():
    cfg = CONFIG["home"]
    text = (ROOT / cfg["source"]).read_text()
    intro, secs = sections(text)
    hero = re.search(r"<!-- hero -->\n(.*?)<!-- /hero -->", intro, re.S)
    lines = [l.strip() for l in hero.group(1).strip().split("\n")] if hero else []
    intro = re.sub(r"<!-- hero -->.*?<!-- /hero -->", "", intro, flags=re.S)
    intro = re.sub(r"^# .*$", "", intro, count=1, flags=re.M)
    body, _ = render(intro, "home", cfg["source"])
    # The first paragraph is the lead; the sample and the notice follow it.
    lead = re.search(r"<p>.*?</p>", body, re.S)
    lead_html = lead.group(0).replace("<p>", '<p class="lead">', 1) if lead else ""
    rest = body.replace(lead.group(0), "", 1) if lead else body

    features = ""
    for heading, sec in secs:
        if heading == cfg.get("features"):
            features, _ = render(sec, "home", cfg["source"])
    actions = []
    for i, a in enumerate(cfg["actions"]):
        cls = "button primary" if i == 0 else "button"
        if "page" in a:
            actions.append(f'<a class="{cls}" href="#{a["page"]}">{html.escape(a["label"])}</a>')
        elif CONFIG.get(a["url"] + "_url"):
            url = CONFIG[a["url"] + "_url"]
            actions.append(external_link(url, html.escape(a["label"]), cls))
    stanza = "".join(
        f'<span class="line"><span class="no" aria-hidden="true">{i + 1}</span>{html.escape(l)}</span>'
        for i, l in enumerate(lines)
    )
    return f"""
<section class="page home" id="home" data-title="{html.escape(CONFIG['name'])}">
  <div class="hero">
    <h1 class="stanza">{stanza}</h1>
    {lead_html}
    <p class="actions">{''.join(actions)}</p>
  </div>
  <div class="prose sample">{rest}</div>
  <div class="prose features"><h2>{html.escape(cfg.get('features', ''))}</h2>{features}</div>
</section>"""


def build_page(page):
    text = (ROOT / page["source"]).read_text()
    skip = page.get("skip_sections")
    if skip is not None:
        _, secs = sections(text)
        kept = [f"## {h}\n{b}" for h, b in secs if h not in skip]
        text = f"# {page['title']}\n\n" + "\n".join(kept)
    body, toc = render(text, page["slug"], page["source"])
    title = page.get("title") or re.search(r"^# +(.+)$", text, re.M).group(1)
    nav = ""
    if len(toc) >= 4:
        items = "".join(f'<li><a href="#{i}">{n}</a></li>' for i, n in toc)
        nav = f'<details class="toc"><summary>On this page</summary><ol>{items}</ol></details>'
    # Put the contents list right after the page title.
    body = re.sub(r"(</h1>)", r"\1" + nav.replace("\\", "\\\\"), body, count=1)
    source = f'<p class="source">Source: <code>{html.escape(page["source"])}</code></p>'
    return f"""
<section class="page prose" id="{page['slug']}" data-title="{html.escape(title)}">
{body}
{source}
</section>"""


def build():
    template = (HERE / "template.html").read_text()
    nav = ['<a href="#home" data-page="home">Home</a>']
    nav += [f'<a href="#{p["slug"]}" data-page="{p["slug"]}">{html.escape(p["nav"])}</a>' for p in CONFIG["pages"]]
    if CONFIG.get("playground_url"):
        nav.append(external_link(CONFIG["playground_url"], "Playground"))
    pages = build_home() + "".join(build_page(p) for p in CONFIG["pages"])
    out = (
        template.replace("{{title}}", html.escape(CONFIG["title"]))
        .replace("{{description}}", html.escape(CONFIG["description"]))
        .replace("{{name}}", html.escape(CONFIG["name"]))
        .replace("{{nav}}", "\n      ".join(nav))
        .replace("{{footer}}", html.escape(CONFIG["footer"]))
        .replace("{{pages}}", pages)
    )
    dist = HERE / "dist"
    dist.mkdir(exist_ok=True)
    (dist / "index.html").write_text(out)
    ids = re.findall(r'\bid="([^"]+)"', out)
    dupes = sorted({i for i in ids if ids.count(i) > 1})
    if dupes:
        sys.exit(f"duplicate ids in the site: {dupes}")
    broken = sorted({h for h in re.findall(r'href="#([^"]+)"', out) if h not in ids})
    if broken:
        sys.exit(f"links to missing anchors: {broken}")
    print(f"site/dist/index.html: {len(CONFIG['pages']) + 1} pages, {len(out):,} bytes")


if __name__ == "__main__":
    build()
