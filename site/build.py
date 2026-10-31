#!/usr/bin/env python3
"""Build the skemo.net landing page into site/dist/.

No dependency besides Python 3 and a `skemo` binary. Every diagram on the page is rendered by skemo
itself at build time, as inline SVG, once in the light theme and once in the dark theme (the page
shows the one matching the reader's colour scheme). Token figures come from tools/measure_tokens.py,
run on the gallery examples of site/gallery/.

Placeholders in src/index.html:
  {{diagram:NAME}}  rendered diagram of examples/NAME.skemo (+ NAME.css), or gallery/NAME
  {{source:NAME}}   escaped source of that diagram
  {{tokens:KEY}}    a figure from the token measurement (ratio, saving_percent, skemo_tokens…)
  {{icon:NAME}}     a Lucide icon from skemo's built-in set, as inline SVG
  {{example:NAME:KEY}}  a figure of one measured example (skemo_tokens, svg_tokens…)
  {{task:KEY}}      a figure of the whole-task measurement (data/whole-task.json: saving_percent,
                    speedup, runs and its own keys)
  {{chart}}         per-example token chart
  {{version}}       version printed by `skemo --version`

Usage: python3 site/build.py [--skemo PATH]   (default: $SKEMO, then target/release/skemo)
"""
import argparse
import html
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

sys.path.insert(0, str(SITE / "tools"))
import measure_tokens  # noqa: E402


def fluid(svg: str) -> str:
    """Fixed pixel size in the file, fluid on the page; wide diagrams are marked to scroll on phones."""
    def attrs(m: re.Match) -> str:
        wide = " wide" if int(m[3]) > 600 else ""
        return f'{m[1]}{wide}"{m[2]} style="max-width:{m[3]}px" role="img"'
    return re.sub(r'(<svg[^>]*?class="[^"]*)"([^>]*?) width="(\d+)" height="\d+"', attrs, svg, count=1)


def find_source(name: str) -> tuple[pathlib.Path, pathlib.Path | None]:
    for folder in (SITE / "examples", GALLERY):
        src = folder / f"{name}.skemo"
        if src.exists():
            css = src.with_suffix(".css")
            return src, css if css.exists() else None
    raise SystemExit(f"unknown diagram: {name}")


def render(skemo: str, name: str, theme: str) -> str:
    src, css = find_source(name)
    cmd = [skemo, "--lang", "en", "render", str(src), "-q", "--format", "svg", "-o", "-",
           "--theme", theme, "--deny-warnings"]
    if css:
        cmd += ["--css", str(css)]
    svg = subprocess.run(cmd, check=True, capture_output=True, text=True).stdout
    return fluid(svg)


def figure(skemo: str, name: str) -> str:
    return (f'<figure class="diagram" aria-label="Diagram rendered by skemo from {name}.skemo">'
            f'<div class="only-light">{render(skemo, name, "light")}</div>'
            f'<div class="only-dark">{render(skemo, name, "dark")}</div></figure>')


def source(name: str) -> str:
    src, _ = find_source(name)
    return f'<pre class="code"><code>{html.escape(src.read_text().rstrip())}</code></pre>'


def icon(skemo: str, name: str) -> str:
    """Take the icon out of a one-card diagram, so the page uses skemo's own icon set."""
    svg = subprocess.run([skemo, "render", "-", "--format", "svg", "-o", "-", "--deny-warnings"],
                         input=f'card#a(icon="{name}") A\n', check=True, capture_output=True, text=True).stdout
    paths = re.search(r'<g class="icon"[^>]*>(.*?)</g>', svg)[1]
    return (f'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
            f'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{paths}</svg>')


def chart(data: dict) -> str:
    """Paired horizontal bars, one row per example; scale shared by every row."""
    top = max(r["svg_tokens"] for r in data["examples"])
    rows = []
    for r in sorted(data["examples"], key=lambda r: r["svg_tokens"]):
        ratio = r["svg_tokens"] / r["skemo_tokens"]
        rows.append(
            f'<div class="bar-row" tabindex="0" title="{r["example"]}: {r["skemo_tokens"]} tokens of skemo, '
            f'{r["svg_tokens"]} tokens of SVG ({ratio:.0f}× more)">'
            f'<span class="bar-name">{r["example"]}</span>'
            f'<span class="bar-track">'
            f'<span class="bar skemo" style="width:{100 * r["skemo_tokens"] / top:.2f}%"></span>'
            f'<span class="bar svg" style="width:{100 * r["svg_tokens"] / top:.2f}%"></span></span>'
            f'<span class="bar-value"><b>{r["skemo_tokens"]:,}</b> vs {r["svg_tokens"]:,} '
            f'<span class="muted">×{ratio:.0f}</span></span></div>')
    table = "".join(
        f'<tr><td>{r["example"]}</td><td>{r["skemo_tokens"]:,}</td><td>{r["svg_tokens"]:,}</td>'
        f'<td>×{r["svg_tokens"] / r["skemo_tokens"]:.1f}</td></tr>' for r in data["examples"])
    return (
        '<div class="chart" role="group" aria-label="Tokens written per diagram">'
        '<div class="legend"><span><i class="swatch skemo"></i>skemo source (+ CSS)</span>'
        '<span><i class="swatch svg"></i>the same picture as SVG</span></div>'
        + "".join(rows) +
        '<details class="table-view"><summary>Show as a table</summary><table>'
        '<thead><tr><th>Example</th><th>skemo tokens</th><th>SVG tokens</th><th>Ratio</th></tr></thead>'
        f'<tbody>{table}</tbody><tfoot><tr><td>Total</td><td>{data["skemo_tokens"]:,}</td>'
        f'<td>{data["svg_tokens"]:,}</td><td>×{data["ratio"]}</td></tr></tfoot></table></details></div>')


def whole_task() -> dict:
    """The measurement of a whole task with and without skemo (data/whole-task.json), and the figures the page says from it."""
    t = json.loads((SITE / "data" / "whole-task.json").read_text())
    t["saving_percent"] = round(100 * (1 - t["tokens_with"] / t["tokens_without"]))
    t["speedup"] = round(t["seconds_without"] / t["seconds_with"])
    return t


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

    page = (SITE / "src" / "index.html").read_text()
    page = re.sub(r"\{\{diagram:([\w-]+)\}\}", lambda m: figure(skemo, m[1]), page)
    page = re.sub(r"\{\{icon:([\w-]+)\}\}", lambda m: icon(skemo, m[1]), page)
    page = re.sub(r"\{\{source:([\w-]+)\}\}", lambda m: source(m[1]), page)
    page = re.sub(r"\{\{tokens:(\w+)\}\}", lambda m: f"{data[m[1]]:,}", page)
    by_name = {r["example"]: r for r in data["examples"]}
    page = re.sub(r"\{\{example:([\w-]+):(\w+)\}\}", lambda m: f"{by_name[m[1]][m[2]]:,}", page)
    task = whole_task()
    page = re.sub(r"\{\{task:(\w+)\}\}", lambda m: f"{task[m[1]]:,}", page)
    page = page.replace("{{chart}}", chart(data)).replace("{{version}}", version)
    if "{{" in page:
        raise SystemExit("unresolved placeholder: " + page[page.index("{{"):][:40])
    (dist / "index.html").write_text(page)

    for asset in ("style.css", "favicon.svg"):
        shutil.copy(SITE / "src" / asset, dist / asset)
    # the product's logo and icons, light and dark
    shutil.copytree(SITE / "src" / "brand", dist / "brand")
    for font in FONT_FILES:
        shutil.copy(FONTS / font, dist / "fonts" / font)
    for skill in (ROOT / "cli/skill").rglob("*.md"):
        target = dist / "skill" / skill.relative_to(ROOT / "cli/skill")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(skill, target)
    print(f"site written to {dist} (skemo {version}, ×{data['ratio']} fewer tokens)")


if __name__ == "__main__":
    main()
