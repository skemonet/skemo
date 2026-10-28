#!/usr/bin/env python3
"""Build the skemo documentation site (docs.skemo.net) into site/dist/.

No dependency besides Python 3 and a `skemo` binary. Pages are HTML fragments in pages/, listed in
NAV below. Every diagram is a real file in examples/ (checked by `cargo test`, tests/site_examples.rs)
and is rendered by skemo at build time as inline SVG, next to its source.

In a page:
  <skemo name="NAME"></skemo>           source of examples/NAME.skemo (+ NAME.css) and its rendering
  attributes: theme="dark|print"        render in that theme only (default: light and dark, following the reader)
              steps="1 2 3"             one image per animation step (lossless WebP) instead of the final state
              figure                    the rendering only, without the source
              stacked                   source above the rendering (automatic beyond WIDE pixels)
  <skemo-source name="NAME"></skemo-source>   the source only

Usage: python3 site/build.py [--skemo PATH]
       (default: $SKEMO, then the release build of ../cli or of the enclosing workspace, then `skemo` on
       the PATH)
"""
import argparse
import html
import os
import pathlib
import re
import shutil
import subprocess

SITE = pathlib.Path(__file__).resolve().parent
EXAMPLES = SITE / "examples"
FONTS = SITE.parent.parent / "engine" / "crates" / "engine" / "assets" / "fonts"
FONT_FILES = ["Inter-Regular.otf", "Inter-Medium.otf", "Inter-Bold.otf", "JetBrainsMono-Regular.ttf",
              "LICENSE-Inter.txt", "LICENSE-JetBrainsMono.txt"]

# (file in pages/, title in the sidebar, section)
NAV = [
    ("index", "Introduction", "Start"),
    ("getting-started", "Getting started", "Start"),
    ("syntax", "Syntax", "Language"),
    ("cards-and-zones", "Cards and zones", "Language"),
    ("arrows", "Arrows", "Language"),
    ("rich-cards", "Rich cards, tables and panels", "Language"),
    ("layout", "Layout and positions", "Language"),
    ("styling", "Styling with CSS", "Style"),
    ("themes", "Themes and contrast", "Style"),
    ("icons-and-links", "Icons and links", "Style"),
    ("animation", "Step-by-step animation", "Modes"),
    ("timeline", "Timeline", "Modes"),
    ("frieze", "Chronological frieze", "Modes"),
    ("mindmap", "Mind map", "Modes"),
    ("cli", "Command line", "Tools"),
    ("ai-agents", "AI agents", "Tools"),
    ("diagnostics", "Errors and warnings", "Tools"),
]

# Diagrams wider than this (px) go under their source instead of beside it.
WIDE = 600

TAG = re.compile(r'<skemo(-source)?((?:\s+[\w-]+(?:="[^"]*")?)*)\s*>\s*</skemo(?:-source)?>')
ATTR = re.compile(r'([\w-]+)(?:="([^"]*)")?')


def fluid(svg: str) -> str:
    """Fixed pixel size in the file, fluid on the page; wide diagrams are marked to scroll on phones."""
    def attrs(m: re.Match) -> str:
        wide = " wide" if int(m[3]) > 600 else ""
        return f'{m[1]}{wide}"{m[2]} style="max-width:{m[3]}px" role="img"'
    return re.sub(r'(<svg[^>]*?class="[^"]*)"([^>]*?) width="(\d+)" height="\d+"', attrs, svg, count=1)


def skemo_binary(arg: str | None) -> str:
    builds = (SITE.parent.parent / "cli/target/release/skemo", SITE.parent.parent / "target/release/skemo")
    for candidate in (arg, os.environ.get("SKEMO"), *map(str, builds)):
        if candidate and pathlib.Path(candidate).exists():
            return candidate
    found = shutil.which("skemo")
    if not found:
        raise SystemExit("skemo binary not found: build the cli crate or pass --skemo")
    return found


class Renderer:
    def __init__(self, skemo: str, dist: pathlib.Path):
        self.skemo = skemo
        self.dist = dist
        self.count = 0

    def svg(self, name: str, theme: str) -> str:
        src = EXAMPLES / f"{name}.skemo"
        cmd = [self.skemo, "--lang", "en", "render", str(src), "-q", "--format", "svg", "-o", "-",
               "--theme", theme, "--deny-warnings"]
        css = src.with_suffix(".css")
        if css.exists():
            cmd += ["--css", str(css)]
        out = subprocess.run(cmd, capture_output=True, text=True)
        if out.returncode:
            raise SystemExit(f"{src}: {out.stderr}")
        self.count += 1
        return fluid(out.stdout)

    def image(self, name: str, theme: str, step: str) -> str:
        """One animation state as a lossless WebP: the SVG output holds every state at once."""
        src = EXAMPLES / f"{name}.skemo"
        file = f"img/{name}-{theme}-{step}.webp"
        (self.dist / "img").mkdir(exist_ok=True)
        cmd = [self.skemo, "--lang", "en", "render", str(src), "-q", "--scale", "2", "--step", step,
               "--theme", theme, "--deny-warnings", "-o", str(self.dist / file)]
        subprocess.run(cmd, check=True)
        self.count += 1
        return f'<img src="{file}" alt="{name}, step {step}" loading="lazy">'

    def figure(self, name: str, attrs: dict) -> str:
        theme = attrs.get("theme")
        steps = attrs.get("steps", "").split() or [None]
        draw = (lambda t, s: self.image(name, t, s)) if steps[0] else (lambda t, s: self.svg(name, t))
        parts = []
        for step in steps:
            caption = f'<figcaption>Step {step}</figcaption>' if step else ""
            if theme:
                body = draw(theme, step)
            else:
                body = (f'<div class="only-light">{draw("light", step)}</div>'
                        f'<div class="only-dark">{draw("dark", step)}</div>')
            parts.append(f'<figure class="diagram">{body}{caption}</figure>')
        cls = "figures steps" if len(steps) > 1 else "figures"
        return f'<div class="{cls}">{"".join(parts)}</div>'

    def source(self, name: str) -> str:
        src = EXAMPLES / f"{name}.skemo"
        out = f'<div class="code-block"><div class="code-title">{name}.skemo</div>' \
              f'<pre><code>{html.escape(src.read_text().rstrip())}</code></pre></div>'
        css = src.with_suffix(".css")
        if css.exists():
            out += f'<div class="code-block"><div class="code-title">{name}.css</div>' \
                   f'<pre><code>{html.escape(css.read_text().rstrip())}</code></pre></div>'
        return out

    def expand(self, match: re.Match) -> str:
        attrs = {k: v or "" for k, v in ATTR.findall(match[2])}
        name = attrs["name"]
        if not (EXAMPLES / f"{name}.skemo").exists():
            raise SystemExit(f"unknown example: {name}")
        if match[1]:
            return self.source(name)
        if "figure" in attrs:
            return self.figure(name, attrs)
        figure = self.figure(name, attrs)
        width = max(int(w) for w in re.findall(r"max-width:(\d+)px", figure))
        wide = width > WIDE or "stacked" in attrs or "steps" in attrs
        layout = "example stacked" if wide else "example"
        return f'<div class="{layout}"><div class="example-source">{self.source(name)}</div>' \
               f'<div class="example-figure">{figure}</div></div>'


def sidebar(current: str) -> str:
    out, section = [], None
    for slug, title, sec in NAV:
        if sec != section:
            if section:
                out.append("</ul>")
            out.append(f'<p class="nav-section">{sec}</p><ul>')
            section = sec
        here = ' aria-current="page"' if slug == current else ""
        out.append(f'<li><a href="{slug}.html"{here}>{title}</a></li>')
    out.append("</ul>")
    return "".join(out)


def pager(i: int) -> str:
    prev = f'<a class="prev" href="{NAV[i - 1][0]}.html">← {NAV[i - 1][1]}</a>' if i else "<span></span>"
    nxt = f'<a class="next" href="{NAV[i + 1][0]}.html">{NAV[i + 1][1]} →</a>' if i + 1 < len(NAV) else ""
    return f'<nav class="pager" aria-label="Pages">{prev}{nxt}</nav>'


def toc(body: str) -> tuple[str, str]:
    """Anchors on h2 headings, and a short table of contents."""
    items = []

    def anchor(m: re.Match) -> str:
        text = re.sub("<[^>]+>", "", m[1])
        slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
        items.append(f'<li><a href="#{slug}">{text}</a></li>')
        return f'<h2 id="{slug}"><a class="anchor" href="#{slug}">{m[1]}</a></h2>'

    body = re.sub(r"<h2>(.*?)</h2>", anchor, body)
    return body, (f'<nav class="toc" aria-label="On this page"><p>On this page</p><ul>{"".join(items)}</ul></nav>'
                  if len(items) > 1 else "")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skemo")
    dist = SITE / "dist"
    shutil.rmtree(dist, ignore_errors=True)
    (dist / "fonts").mkdir(parents=True)
    renderer = Renderer(skemo_binary(parser.parse_args().skemo), dist)
    version = subprocess.run([renderer.skemo, "--version"], check=True, capture_output=True,
                             text=True).stdout.split()[-1]
    template = (SITE / "template.html").read_text()

    for i, (slug, title, _) in enumerate(NAV):
        body = (SITE / "pages" / f"{slug}.html").read_text()
        body = TAG.sub(renderer.expand, body).replace("{{version}}", version)
        body, contents = toc(body)
        page = (template.replace("{{title}}", title if slug != "index" else "skemo documentation")
                .replace("{{nav}}", sidebar(slug)).replace("{{toc}}", contents)
                .replace("{{body}}", body).replace("{{pager}}", pager(i)).replace("{{version}}", version))
        (dist / f"{slug}.html").write_text(page)

    for asset in ("style.css", "favicon.svg"):
        shutil.copy(SITE / "assets" / asset, dist / asset)
    for font in FONT_FILES:
        if (FONTS / font).exists():
            shutil.copy(FONTS / font, dist / "fonts" / font)
    print(f"{len(NAV)} pages, {renderer.count} renderings written to {dist} (skemo {version})")


if __name__ == "__main__":
    main()
