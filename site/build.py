#!/usr/bin/env python3
"""Build skemo.net into site/dist/: the landing page in English and French, the documentation under
/docs/ and /fr/docs/ (lang/site/build.py), and the files search engines and agents read (sitemap.xml,
robots.txt, llms.txt, a 404 page, the social card og.png).

No dependency besides Python 3 and a `skemo` binary. Every diagram on the site is rendered by skemo
itself at build time, as inline SVG, once in the light theme and once in the dark theme (the page
shows the one matching the reader's colour scheme). Token figures come from tools/measure_tokens.py,
run on the gallery examples of site/gallery/.

Placeholders in src/index.html (English) and src/fr/index.html (French):
  {{seo}}           canonical URL, language alternates, social card and structured data
  {{diagram:NAME}}  rendered diagram of examples/NAME.skemo (+ NAME.css), or gallery/NAME; a French page
                    takes examples/fr/NAME.skemo when it exists
  {{tabbed:NAME}}   the same, with its source in a second tab (radio buttons, no script)
  {{source:NAME}}   escaped source of that diagram
  {{tokens:KEY}}    a figure from the token measurement (ratio, saving_percent, skemo_tokens…)
  {{icon:NAME}}     a Lucide icon from skemo's built-in set, as inline SVG
  {{example:NAME:KEY}}  a figure of one measured example (skemo_tokens, svg_tokens…)
  {{task:KEY}}      a figure of the whole-task measurement (data/whole-task.json: saving_percent,
                    speedup, runs and its own keys)
  {{chart}}         per-example token chart
  {{version}}       version printed by `skemo --version`
Numbers follow the page's language (1,234 and 12.3 in English, 1 234 and 12,3 in French).

Usage: python3 site/build.py [--skemo PATH]   (default: $SKEMO, then target/release/skemo)
"""
import argparse
import html
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

SITE = pathlib.Path(__file__).resolve().parent
ROOT = SITE.parent
GALLERY = SITE / "gallery"
FONTS = ROOT / "engine" / "crates" / "engine" / "assets" / "fonts"
FONT_FILES = ["Inter-Regular.otf", "Inter-Medium.otf", "Inter-Bold.otf", "JetBrainsMono-Regular.ttf",
              "LICENSE-Inter.txt", "LICENSE-JetBrainsMono.txt"]
ORIGIN = "https://skemo.net"
LANGS = {"en": "src/index.html", "fr": "src/fr/index.html"}

sys.path.insert(0, str(SITE / "tools"))
import measure_tokens  # noqa: E402

_spec = importlib.util.spec_from_file_location("docs_build", ROOT / "lang" / "site" / "build.py")
docs = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(docs)

TEXT = {
    "en": {"alt": "Diagram rendered by skemo from {name}.skemo", "diagram": "Diagram", "source": "Source",
           "chart": "Tokens written per diagram", "skemo": "skemo source (+ CSS)", "svg": "the same picture as SVG",
           "table": "Show as a table", "example": "Example", "skemo_tokens": "skemo tokens", "svg_tokens": "SVG tokens",
           "ratio": "Ratio", "total": "Total", "more": "more", "of": "tokens of",
           "desc": "Write cards, zones and arrows as text; skemo lays them out and renders PNG, SVG or animated HTML.",
           "title": "skemo: diagrams as code, laid out and rendered for you"},
    "fr": {"alt": "Diagramme rendu par skemo à partir de {name}.skemo", "diagram": "Diagramme", "source": "Source",
           "chart": "Tokens écrits par diagramme", "skemo": "source skemo (+ CSS)", "svg": "la même image en SVG",
           "table": "Voir sous forme de tableau", "example": "Exemple", "skemo_tokens": "tokens skemo",
           "svg_tokens": "tokens SVG", "ratio": "Rapport", "total": "Total", "more": "de plus", "of": "tokens de",
           "desc": "Écrivez cartes, zones et flèches en texte ; skemo les place et rend du PNG, du SVG ou du HTML animé.",
           "title": "skemo : des diagrammes en texte, placés et rendus pour vous"},
}


def number(value, lang: str) -> str:
    """1,234 and 12.3 in English; 1 234 (narrow no-break space) and 12,3 in French."""
    if isinstance(value, float) and value.is_integer():
        value = int(value)
    text = f"{value:,}"
    if lang == "fr":
        text = text.replace(",", " ").replace(".", ",")
    return text


def fluid(svg: str, label: str) -> str:
    """Fixed pixel size in the file, fluid on the page; wide diagrams are marked to scroll on phones."""
    def attrs(m: re.Match) -> str:
        wide = " wide" if int(m[3]) > 600 else ""
        return f'{m[1]}{wide}"{m[2]} style="max-width:{m[3]}px" role="img" aria-label="{html.escape(label)}"'
    return re.sub(r'(<svg[^>]*?class="[^"]*)"([^>]*?) width="(\d+)" height="\d+"', attrs, svg, count=1)


def find_source(name: str, lang: str = "en") -> tuple[pathlib.Path, pathlib.Path | None]:
    folders = ([SITE / "examples" / lang] if lang != "en" else []) + [SITE / "examples", GALLERY]
    for folder in folders:
        src = folder / f"{name}.skemo"
        if src.exists():
            css = src.with_suffix(".css")
            return src, css if css.exists() else None
    raise SystemExit(f"unknown diagram: {name}")


def command(skemo: str, name: str, lang: str) -> list[str]:
    src, css = find_source(name, lang)
    cmd = ["env", f"SKEMO_LANG={lang}", skemo, "render", str(src), "-q", "--deny-warnings"]
    return cmd + (["--css", str(css)] if css else [])


def render(skemo: str, name: str, theme: str, lang: str) -> str:
    cmd = command(skemo, name, lang) + ["--format", "svg", "-o", "-", "--theme", theme, "--transparent"]
    svg = subprocess.run(cmd, check=True, capture_output=True, text=True).stdout
    return fluid(svg, TEXT[lang]["alt"].format(name=name))


def figure(skemo: str, name: str, lang: str) -> str:
    return (f'<figure class="diagram">'
            f'<div class="only-light">{render(skemo, name, "light", lang)}</div>'
            f'<div class="only-dark">{render(skemo, name, "dark", lang)}</div></figure>')


def source(name: str, lang: str) -> str:
    src, _ = find_source(name, lang)
    return f'<pre class="code"><code>{html.escape(src.read_text().rstrip())}</code></pre>'


class Tabs:
    """A diagram and its source as two tabs; ids are unique within one page."""
    def __init__(self, skemo: str, lang: str):
        self.skemo, self.lang, self.count = skemo, lang, 0

    def __call__(self, m: re.Match) -> str:
        self.count += 1
        tab, text = f"g{self.count}", TEXT[self.lang]
        return (f'<div class="tabbed">'
                f'<input type="radio" class="t-fig" name="{tab}" id="{tab}-fig" checked>'
                f'<label for="{tab}-fig">{text["diagram"]}</label>'
                f'<input type="radio" class="t-src" name="{tab}" id="{tab}-src">'
                f'<label for="{tab}-src">{text["source"]}</label>'
                f'<div class="p-fig">{figure(self.skemo, m[1], self.lang)}</div>'
                f'<div class="p-src">{source(m[1], self.lang)}</div></div>')


def icon(skemo: str, name: str) -> str:
    """Take the icon out of a one-card diagram, so the page uses skemo's own icon set."""
    svg = subprocess.run([skemo, "render", "-", "--format", "svg", "-o", "-", "--deny-warnings"],
                         input=f'card#a(icon="{name}") A\n', check=True, capture_output=True, text=True).stdout
    paths = re.search(r'<g class="icon"[^>]*>(.*?)</g>', svg)[1]
    return (f'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
            f'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{paths}</svg>')


def chart(data: dict, lang: str) -> str:
    """Paired horizontal bars, one row per example; scale shared by every row."""
    t = TEXT[lang]
    n = lambda v: number(v, lang)  # noqa: E731
    top = max(r["svg_tokens"] for r in data["examples"])
    rows = []
    for r in sorted(data["examples"], key=lambda r: r["svg_tokens"]):
        ratio = r["svg_tokens"] / r["skemo_tokens"]
        rows.append(
            f'<div class="bar-row" tabindex="0" title="{r["example"]}: {n(r["skemo_tokens"])} {t["of"]} skemo, '
            f'{n(r["svg_tokens"])} {t["of"]} SVG (×{ratio:.0f})">'
            f'<span class="bar-name">{r["example"]}</span>'
            f'<span class="bar-track">'
            f'<span class="bar skemo" style="width:{100 * r["skemo_tokens"] / top:.2f}%"></span>'
            f'<span class="bar svg" style="width:{100 * r["svg_tokens"] / top:.2f}%"></span></span>'
            f'<span class="bar-value"><b>{n(r["skemo_tokens"])}</b> vs {n(r["svg_tokens"])} '
            f'<span class="muted">×{ratio:.0f}</span></span></div>')
    table = "".join(
        f'<tr><td>{r["example"]}</td><td>{n(r["skemo_tokens"])}</td><td>{n(r["svg_tokens"])}</td>'
        f'<td>×{n(round(r["svg_tokens"] / r["skemo_tokens"], 1))}</td></tr>' for r in data["examples"])
    return (
        f'<div class="chart" role="group" aria-label="{t["chart"]}">'
        f'<div class="legend"><span><i class="swatch skemo"></i>{t["skemo"]}</span>'
        f'<span><i class="swatch svg"></i>{t["svg"]}</span></div>'
        + "".join(rows) +
        f'<details class="table-view"><summary>{t["table"]}</summary><table>'
        f'<thead><tr><th>{t["example"]}</th><th>{t["skemo_tokens"]}</th><th>{t["svg_tokens"]}</th><th>{t["ratio"]}</th></tr></thead>'
        f'<tbody>{table}</tbody><tfoot><tr><td>{t["total"]}</td><td>{n(data["skemo_tokens"])}</td>'
        f'<td>{n(data["svg_tokens"])}</td><td>×{n(data["ratio"])}</td></tr></tfoot></table></details></div>')


def whole_task() -> dict:
    """The measurement of a whole task with and without skemo (data/whole-task.json), and the figures the page says from it."""
    t = json.loads((SITE / "data" / "whole-task.json").read_text())
    t["saving_percent"] = round(100 * (1 - t["tokens_with"] / t["tokens_without"]))
    t["speedup"] = round(t["seconds_without"] / t["seconds_with"])
    return t


def seo(lang: str, version: str) -> str:
    """Canonical URL, language alternates, social card and structured data of a landing page."""
    url = f"{ORIGIN}{docs.prefix(lang)}/"
    alternates = "".join(f'<link rel="alternate" hreflang="{other}" href="{ORIGIN}{docs.prefix(other)}/">'
                         for other in LANGS)
    t = TEXT[lang]
    data = [
        {"@context": "https://schema.org", "@type": "WebSite", "name": "skemo", "url": f"{ORIGIN}/",
         "inLanguage": list(LANGS)},
        {"@context": "https://schema.org", "@type": "SoftwareApplication", "name": "skemo", "url": url,
         "description": t["desc"], "applicationCategory": "DeveloperApplication",
         "operatingSystem": "Linux, macOS, Windows", "softwareVersion": version, "license":
         "https://github.com/skemonet/skemo/blob/main/LICENSE-MIT", "downloadUrl":
         "https://github.com/skemonet/skemo/releases", "codeRepository": "https://github.com/skemonet/skemo",
         "image": f"{ORIGIN}/og.png", "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"}},
    ]
    return (f'<link rel="canonical" href="{url}">\n{alternates}'
            f'<link rel="alternate" hreflang="x-default" href="{ORIGIN}/">\n'
            f'<meta property="og:type" content="website"><meta property="og:site_name" content="skemo">'
            f'<meta property="og:locale" content="{"fr_FR" if lang == "fr" else "en_GB"}">\n'
            f'<meta property="og:title" content="{html.escape(t["title"])}">'
            f'<meta property="og:description" content="{html.escape(t["desc"])}">'
            f'<meta property="og:url" content="{url}"><meta property="og:image" content="{ORIGIN}/og.png">\n'
            f'<meta name="twitter:card" content="summary_large_image">\n'
            + "".join(f'<script type="application/ld+json">{json.dumps(d, ensure_ascii=False)}</script>'
                      for d in data))


def landing(skemo: str, lang: str, data: dict, version: str) -> str:
    n = lambda v: number(v, lang)  # noqa: E731
    page = (SITE / LANGS[lang]).read_text()
    page = page.replace("{{seo}}", seo(lang, version))
    other = [x for x in LANGS if x != lang][0]
    page = page.replace("{{header}}", docs.header(lang, f"{docs.prefix(other)}/", landing=True))
    page = page.replace("{{footer}}", docs.footer(lang, version))
    page = re.sub(r"\{\{tabbed:([\w-]+)\}\}", Tabs(skemo, lang), page)
    page = re.sub(r"\{\{diagram:([\w-]+)\}\}", lambda m: figure(skemo, m[1], lang), page)
    page = re.sub(r"\{\{icon:([\w-]+)\}\}", lambda m: icon(skemo, m[1]), page)
    page = re.sub(r"\{\{source:([\w-]+)\}\}", lambda m: source(m[1], lang), page)
    page = re.sub(r"\{\{tokens:(\w+)\}\}", lambda m: n(data[m[1]]), page)
    by_name = {r["example"]: r for r in data["examples"]}
    page = re.sub(r"\{\{example:([\w-]+):(\w+)\}\}", lambda m: n(by_name[m[1]][m[2]]), page)
    task = whole_task()
    page = re.sub(r"\{\{task:(\w+)\}\}", lambda m: n(task[m[1]]), page)
    page = page.replace("{{chart}}", chart(data, lang)).replace("{{version}}", version)
    if "{{" in page:
        raise SystemExit("unresolved placeholder: " + page[page.index("{{"):][:40])
    return page


def sitemap(pages: list[dict]) -> str:
    """Every page with its language alternates (the landing pages, then the documentation)."""
    groups: dict[str, dict[str, str]] = {}
    for p in pages:
        groups.setdefault(p["slug"], {})[p["lang"]] = ORIGIN + p["path"]
    entries = []
    for urls in groups.values():
        links = "".join(f'<xhtml:link rel="alternate" hreflang="{lang}" href="{url}"/>' for lang, url in urls.items())
        links += f'<xhtml:link rel="alternate" hreflang="x-default" href="{urls["en"]}"/>'
        entries += [f"<url><loc>{url}</loc>{links}</url>" for url in urls.values()]
    return ('<?xml version="1.0" encoding="UTF-8"?>\n'
            '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n'
            + "\n".join(entries) + "\n</urlset>\n")


def llms(pages: list[dict], version: str) -> str:
    """llms.txt (llmstxt.org): what skemo is and where an agent finds the documentation and the skill."""
    lines = [
        "# skemo", "",
        "> skemo is a text language for diagrams (a Pug dialect styled with CSS) and a single offline binary that lays "
        "them out, routes the arrows and renders PNG, SVG, WebP, GIF, APNG or animated HTML. "
        f"Open source (MIT OR Apache-2.0), version {version}.", "",
        "An AI agent writes a few lines of `.skemo` text instead of drawing SVG; `skemo lint` reports mistakes as text "
        "with line numbers; `skemo render` draws the picture.", "",
        "## Agent skill", "",
        f"- [SKILL.md]({ORIGIN}/skill/skemo/SKILL.md): the workflow, the syntax and the rules, for coding agents",
        f"- [format.md]({ORIGIN}/skill/skemo/references/format.md): the language reference read by the skill", "",
        "## Documentation", "",
    ]
    lines += [f"- [{p['title']}]({ORIGIN}{p['path']}): {p['description']}" for p in pages if p["lang"] == "en"]
    lines += ["", "## Optional", "",
              f"- [Documentation in French]({ORIGIN}/fr/docs/)",
              "- [Source code](https://github.com/skemonet/skemo)", ""]
    return "\n".join(lines)


NOT_FOUND = """<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Page not found | skemo</title><meta name="robots" content="noindex">
<link rel="icon" href="/favicon.svg" type="image/svg+xml"><link rel="stylesheet" href="/brand.css"><link rel="stylesheet" href="/style.css"></head>
<body><main><section class="hero"><div class="wrap">
<span class="eyebrow">404</span>
<h1>Page not found.</h1>
<p class="lead">This page does not exist, or it moved when the documentation came to skemo.net/docs/.
  · Cette page n’existe pas, ou elle a changé d’adresse.</p>
<div class="actions"><a class="btn primary" href="/">skemo.net</a><a class="btn ghost" href="/docs/">Documentation</a>
<a class="btn ghost" href="/fr/docs/" lang="fr">Documentation en français</a></div>
</div></section></main></body></html>
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skemo", default=os.environ.get("SKEMO", str(ROOT / "target/release/skemo")))
    skemo = parser.parse_args().skemo
    dist = SITE / "dist"
    shutil.rmtree(dist, ignore_errors=True)
    (dist / "fonts").mkdir(parents=True)

    data = measure_tokens.measure(skemo, GALLERY)
    data["count"] = len(data["examples"])
    (dist / "tokens.json").write_text(json.dumps(data, indent=2) + "\n")
    version = subprocess.run([skemo, "--version"], check=True, capture_output=True, text=True).stdout.split()[-1]

    pages = docs.build(skemo, dist)
    for lang in LANGS:
        target = dist / docs.prefix(lang).strip("/") / "index.html"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(landing(skemo, lang, data, version))
        pages.insert(0, {"path": f"{docs.prefix(lang)}/", "lang": lang, "slug": "", "title": TEXT[lang]["title"],
                         "description": TEXT[lang]["desc"]})

    # The social card of the landing page: the hero diagram, sharp enough for a large preview.
    subprocess.run(command(skemo, "hero", "en") + ["--scale", "2", "--theme", "light", "--no-source",
                                                   "-o", str(dist / "og.png")], check=True)
    (dist / "sitemap.xml").write_text(sitemap(pages))
    (dist / "robots.txt").write_text(f"User-agent: *\nAllow: /\n\nSitemap: {ORIGIN}/sitemap.xml\n")
    (dist / "llms.txt").write_text(llms(pages, version))
    (dist / "404.html").write_text(NOT_FOUND)

    for asset in ("brand.css", "style.css", "favicon.svg"):
        shutil.copy(SITE / "src" / asset, dist / asset)
    # the product's logo and icons, light and dark
    shutil.copytree(SITE / "src" / "brand", dist / "brand")
    for font in FONT_FILES:
        shutil.copy(FONTS / font, dist / "fonts" / font)
    for skill in (ROOT / "cli/skill").rglob("*.md"):
        target = dist / "skill" / skill.relative_to(ROOT / "cli/skill")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(skill, target)
    print(f"site written to {dist} (skemo {version}, {len(pages)} pages, ×{data['ratio']} fewer tokens)")


if __name__ == "__main__":
    main()
