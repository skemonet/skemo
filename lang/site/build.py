#!/usr/bin/env python3
"""Build the skemo documentation (skemo.net/docs/ and skemo.net/fr/docs/) into site/dist/.

No dependency besides Python 3 and a `skemo` binary. The landing page's build (site/build.py at the
root of the repository) calls `build()` to put the documentation under its own dist folder; run alone,
this script writes a tree with the same paths (docs/, fr/docs/, fonts/) into lang/site/dist/.

Pages are HTML fragments in pages/<lang>/, listed in NAV below. Each one starts with a header comment:

  <!--
  title: Architecture diagrams as code      (the <title> and the social cards)
  nav: Architecture diagrams                (the sidebar, the pager, the breadcrumb)
  description: One or two sentences for search engines and link previews.
  -->

Every diagram is a real file in examples/ (checked by `cargo test`, tests/site_examples.rs) and is
rendered by skemo at build time as inline SVG, with its source in a second tab. A French page shows
examples/fr/NAME.skemo when it exists (and examples/fr/NAME.css, else examples/NAME.css); engine labels
(months, centuries) follow the page's language.

In a page:
  <skemo name="NAME"></skemo>           the rendering of examples/NAME.skemo (+ NAME.css), its source in a tab
  attributes: theme="dark|print"        render in that theme only (default: light and dark, following the reader)
              steps="1 2 3"             one image per animation step (lossless WebP) instead of the final state
              figure                    the rendering only, without the source
  <skemo-source name="NAME"></skemo-source>   the source only
  href="arrows.html#sides"              a link to another page: rewritten to /docs/arrows/#sides (or /fr/docs/…)

The first diagram of a page is also rendered as a PNG for its social card (og:image).

Usage: python3 site/build.py [--skemo PATH] [--out DIR]
       (default: $SKEMO, then the release build of ../cli or of the enclosing workspace, then `skemo` on
       the PATH)
"""
import argparse
import html
import json
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
ORIGIN = "https://skemo.net"
LANGS = ("en", "fr")

# (file in pages/<lang>/, section)
NAV = [
    ("index", "start"),
    ("getting-started", "start"),
    ("architecture-diagrams", "draw"),
    ("cloud-and-network-diagrams", "draw"),
    ("process-diagrams", "draw"),
    ("org-charts", "draw"),
    ("mind-maps", "draw"),
    ("timelines", "draw"),
    ("chronological-friezes", "draw"),
    ("step-by-step-animations", "draw"),
    ("animated-flows", "draw"),
    ("ai-agents", "use"),
    ("diagrams-as-code", "use"),
    ("syntax", "language"),
    ("cards-and-zones", "language"),
    ("arrows", "language"),
    ("rich-cards", "language"),
    ("layout", "language"),
    ("icons-and-links", "language"),
    ("styling", "style"),
    ("themes", "style"),
    ("cli", "tools"),
    ("diagnostics", "tools"),
]
SLUGS = {slug for slug, _ in NAV}

TEXT = {
    "en": {
        "start": "Start", "draw": "What you can draw", "use": "Where it fits", "language": "Language reference",
        "style": "Style", "tools": "Tools", "docs": "Documentation", "menu": "Menu", "skip": "Skip to content",
        "toc": "On this page", "pages": "Pages", "step": "Step", "home": "Home", "other": "Français",
        "other_label": "Lire cette page en français",
        "footer": "Every diagram on these pages is rendered by skemo from the source shown next to it.",
        "alt": "Diagram rendered by skemo from {name}.skemo", "diagram": "Diagram", "source": "Source",
    },
    "fr": {
        "start": "Commencer", "draw": "Ce que vous pouvez dessiner", "use": "Où l’utiliser",
        "language": "Référence du langage", "style": "Style", "tools": "Outils", "docs": "Documentation",
        "menu": "Menu", "skip": "Aller au contenu", "toc": "Sur cette page", "pages": "Pages", "step": "Étape",
        "home": "Accueil", "other": "English", "other_label": "Read this page in English",
        "footer": "Chaque diagramme de ces pages est rendu par skemo à partir de la source affichée à côté.",
        "alt": "Diagramme rendu par skemo à partir de {name}.skemo", "diagram": "Diagramme", "source": "Source",
    },
}

CHROME = {
    "en": {"main": "Main", "docs": "Documentation", "draw": "What you can draw", "agents": "For AI agents",
           "other": "FR", "other_label": "Lire cette page en français", "product": "Product", "learn": "Learn",
           "project": "Project", "start": "Getting started", "syntax": "Syntax", "cli": "Command line",
           "releases": "Releases", "skill": "Agent skill", "tagline": "Diagrams as code: write the structure, skemo draws the diagram.",
           "fine": "Open source, MIT OR Apache-2.0 · Fonts: Inter, JetBrains Mono (SIL OFL 1.1) · Icons: Lucide (ISC) · No tracker, no cookie."},
    "fr": {"main": "Principal", "docs": "Documentation", "draw": "Ce que vous pouvez dessiner", "agents": "Pour les agents IA",
           "other": "EN", "other_label": "Read this page in English", "product": "Produit", "learn": "Apprendre",
           "project": "Projet", "start": "Premiers pas", "syntax": "Syntaxe", "cli": "Ligne de commande",
           "releases": "Versions", "skill": "Compétence pour agents", "tagline": "Des diagrammes en code : écrivez la structure, skemo dessine le diagramme.",
           "fine": "Libre, MIT OU Apache-2.0 · Polices : Inter, JetBrains Mono (SIL OFL 1.1) · Icônes : Lucide (ISC) · Ni traceur, ni cookie."},
}


def logo(lang: str) -> str:
    return (f'<a class="logo" href="{prefix(lang)}/"><picture><source srcset="/brand/logo-dark.png" '
            f'media="(prefers-color-scheme: dark)"><img src="/brand/logo-light.png" width="126" height="26" alt="skemo"></picture></a>')


def header(lang: str, other_url: str, landing: bool = False) -> str:
    """The header of every page: logo, the main links, the other language."""
    t, other = CHROME[lang], [x for x in LANGS if x != lang][0]
    links = ""
    if landing:
        links += f'<a class="wide-only" href="#draw">{t["draw"]}</a><a class="wide-only" href="#agents">{t["agents"]}</a>'
    current = "" if landing else ' aria-current="page"'
    links += (f'<a href="{page_path(lang, "index")}"{current}>{t["docs"]}</a>'
              f'<a href="https://github.com/skemonet/skemo">GitHub</a>'
              f'<a class="lang" href="{other_url}" hreflang="{other}" lang="{other}" title="{t["other_label"]}">{t["other"]}</a>')
    return (f'<header class="top"><div class="top-inner">{logo(lang)}'
            f'<nav aria-label="{t["main"]}">{links}</nav></div></header>')


def footer(lang: str, version: str) -> str:
    """The footer of every page: what skemo is, links by kind, licences."""
    t = CHROME[lang]
    d = lambda slug: page_path(lang, slug)  # noqa: E731
    return (f'<footer class="site-footer"><div class="cols">'
            f'<div>{logo(lang)}<p>{t["tagline"]}</p><p>skemo {version}</p></div>'
            f'<div><h3>{t["product"]}</h3><ul>'
            f'<li><a href="{d("architecture-diagrams")}">{TITLES[lang]["architecture-diagrams"]}</a></li>'
            f'<li><a href="{d("cloud-and-network-diagrams")}">{TITLES[lang]["cloud-and-network-diagrams"]}</a></li>'
            f'<li><a href="{d("timelines")}">{TITLES[lang]["timelines"]}</a></li>'
            f'<li><a href="{d("step-by-step-animations")}">{TITLES[lang]["step-by-step-animations"]}</a></li></ul></div>'
            f'<div><h3>{t["learn"]}</h3><ul>'
            f'<li><a href="{d("getting-started")}">{t["start"]}</a></li><li><a href="{d("syntax")}">{t["syntax"]}</a></li>'
            f'<li><a href="{d("cli")}">{t["cli"]}</a></li><li><a href="{d("ai-agents")}">{TITLES[lang]["ai-agents"]}</a></li></ul></div>'
            f'<div><h3>{t["project"]}</h3><ul>'
            f'<li><a href="https://github.com/skemonet/skemo">GitHub</a></li>'
            f'<li><a href="https://github.com/skemonet/skemo/releases">{t["releases"]}</a></li>'
            f'<li><a href="/skill/skemo/SKILL.md">{t["skill"]}</a></li><li><a href="/llms.txt">llms.txt</a></li></ul></div>'
            f'</div><div class="fine">{t["fine"]}</div></footer>')


# Sidebar titles of the pages, per language, filled by build() from the pages' headers.
TITLES: dict[str, dict[str, str]] = {lang: {} for lang in LANGS}


TAG = re.compile(r'<skemo(-source)?((?:\s+[\w-]+(?:="[^"]*")?)*)\s*>\s*</skemo(?:-source)?>')
ATTR = re.compile(r'([\w-]+)(?:="([^"]*)")?')
HEADER = re.compile(r"\A\s*<!--(.*?)-->\s*", re.S)
LINK = re.compile(r'href="([a-z0-9-]+)\.html(#[^"]*)?"')


def prefix(lang: str) -> str:
    """Path of a language's root: '' for English, '/fr' for French."""
    return "" if lang == "en" else f"/{lang}"


def page_path(lang: str, slug: str) -> str:
    """Site path of a documentation page, with a trailing slash (GitHub Pages serves its index.html)."""
    return f"{prefix(lang)}/docs/" + ("" if slug == "index" else f"{slug}/")


def fluid(svg: str, label: str) -> str:
    """Fixed pixel size in the file, fluid on the page; wide diagrams are marked to scroll on phones."""
    def attrs(m: re.Match) -> str:
        wide = " wide" if int(m[3]) > 600 else ""
        return f'{m[1]}{wide}"{m[2]} style="max-width:{m[3]}px" role="img" aria-label="{html.escape(label)}"'
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


def parse_header(text: str, where: str) -> tuple[dict, str]:
    m = HEADER.match(text)
    if not m:
        raise SystemExit(f"{where}: missing header comment (title, nav, description)")
    meta = {}
    for line in m[1].strip().splitlines():
        key, _, value = line.partition(":")
        meta[key.strip()] = value.strip()
    for key in ("title", "nav", "description"):
        if not meta.get(key):
            raise SystemExit(f"{where}: header without {key}")
    return meta, text[m.end():]


class Renderer:
    def __init__(self, skemo: str, out: pathlib.Path, lang: str):
        self.skemo = skemo
        self.out = out
        self.lang = lang
        self.count = 0
        self.first = None  # first diagram of the page being built, for its social card
        self.tabs = 0

    def files(self, name: str) -> tuple[pathlib.Path, pathlib.Path | None]:
        src = EXAMPLES / f"{name}.skemo"
        if not src.exists():
            raise SystemExit(f"unknown example: {name}")
        css = src.with_suffix(".css")
        if self.lang != "en":
            local = EXAMPLES / self.lang / f"{name}.skemo"
            if local.exists():
                src = local
            if local.with_suffix(".css").exists():
                css = local.with_suffix(".css")
        return src, css if css.exists() else None

    def command(self, name: str) -> list[str]:
        src, css = self.files(name)
        cmd = ["env", f"SKEMO_LANG={self.lang}", self.skemo, "render", str(src), "-q", "--deny-warnings"]
        return cmd + (["--css", str(css)] if css else [])

    def img_folder(self) -> pathlib.Path:
        folder = self.out / page_path(self.lang, "index").strip("/") / "img"
        folder.mkdir(parents=True, exist_ok=True)
        return folder

    def svg(self, name: str, theme: str) -> str:
        cmd = self.command(name) + ["--format", "svg", "-o", "-", "--theme", theme, "--motion", "--transparent"]
        out = subprocess.run(cmd, capture_output=True, text=True)
        if out.returncode:
            raise SystemExit(f"{name}: {out.stderr}")
        self.count += 1
        return fluid(out.stdout, TEXT[self.lang]["alt"].format(name=name))

    def image(self, name: str, theme: str, step: str) -> str:
        """One animation state as a lossless WebP: the SVG output holds every state at once."""
        file = f"{name}-{theme}-{step}.webp"
        cmd = self.command(name) + ["--scale", "2", "--step", step, "--theme", theme,
                                    "-o", str(self.img_folder() / file)]
        subprocess.run(cmd, check=True)
        self.count += 1
        alt = f'{TEXT[self.lang]["alt"].format(name=name)}, {TEXT[self.lang]["step"].lower()} {step}'
        return f'<img src="{page_path(self.lang, "index")}img/{file}" alt="{html.escape(alt)}" loading="lazy">'

    def card(self, name: str, slug: str) -> str:
        """The page's social card: its first diagram as a PNG (absolute URL)."""
        file = f"og-{slug}.png"
        subprocess.run(self.command(name) + ["--scale", "2", "--theme", "light", "--no-source",
                                             "-o", str(self.img_folder() / file)], check=True)
        return f"{ORIGIN}{page_path(self.lang, 'index')}img/{file}"

    def figure(self, name: str, attrs: dict) -> str:
        theme = attrs.get("theme")
        steps = attrs.get("steps", "").split() or [None]
        draw = (lambda t, s: self.image(name, t, s)) if steps[0] else (lambda t, s: self.svg(name, t))
        parts = []
        for step in steps:
            caption = f'<figcaption>{TEXT[self.lang]["step"]} {step}</figcaption>' if step else ""
            if theme:
                body = draw(theme, step)
            else:
                body = (f'<div class="only-light">{draw("light", step)}</div>'
                        f'<div class="only-dark">{draw("dark", step)}</div>')
            parts.append(f'<figure class="diagram">{body}{caption}</figure>')
        cls = "figures steps" if len(steps) > 1 else "figures"
        return f'<div class="{cls}">{"".join(parts)}</div>'

    def source(self, name: str) -> str:
        src, css = self.files(name)
        out = f'<div class="code-block"><div class="code-title">{name}.skemo</div>' \
              f'<pre><code>{html.escape(src.read_text().rstrip())}</code></pre></div>'
        if css:
            out += f'<div class="code-block"><div class="code-title">{name}.css</div>' \
                   f'<pre><code>{html.escape(css.read_text().rstrip())}</code></pre></div>'
        return out

    def expand(self, match: re.Match) -> str:
        attrs = {k: v or "" for k, v in ATTR.findall(match[2])}
        name = attrs["name"]
        self.files(name)
        if match[1]:
            return self.source(name)
        self.first = self.first or name
        if "figure" in attrs:
            return self.figure(name, attrs)
        # The diagram and its source as two tabs (radio buttons and CSS, no script): both stay in the HTML.
        self.tabs += 1
        tab = f"ex{self.tabs}"
        text = TEXT[self.lang]
        return (f'<div class="example">'
                f'<input type="radio" class="t-fig" name="{tab}" id="{tab}-fig" checked>'
                f'<label for="{tab}-fig">{text["diagram"]}</label>'
                f'<input type="radio" class="t-src" name="{tab}" id="{tab}-src">'
                f'<label for="{tab}-src">{text["source"]}</label>'
                f'<div class="p-fig">{self.figure(name, attrs)}</div>'
                f'<div class="p-src">{self.source(name)}</div></div>')


def sidebar(lang: str, current: str, titles: dict) -> str:
    out, section = [], None
    for slug, sec in NAV:
        if sec != section:
            if section:
                out.append("</ul>")
            out.append(f'<p class="nav-section">{TEXT[lang][sec]}</p><ul>')
            section = sec
        here = ' aria-current="page"' if slug == current else ""
        out.append(f'<li><a href="{page_path(lang, slug)}"{here}>{html.escape(titles[slug])}</a></li>')
    out.append("</ul>")
    return "".join(out)


def pager(lang: str, i: int, titles: dict) -> str:
    def link(j: int, cls: str, text: str) -> str:
        return f'<a class="{cls}" href="{page_path(lang, NAV[j][0])}">{text}</a>'
    prev = link(i - 1, "prev", f"← {html.escape(titles[NAV[i - 1][0]])}") if i else "<span></span>"
    nxt = link(i + 1, "next", f"{html.escape(titles[NAV[i + 1][0]])} →") if i + 1 < len(NAV) else ""
    return f'<nav class="pager" aria-label="{TEXT[lang]["pages"]}">{prev}{nxt}</nav>'


def toc(lang: str, body: str) -> tuple[str, str]:
    """Anchors on h2 headings, and a short table of contents."""
    items = []

    def anchor(m: re.Match) -> str:
        text = re.sub("<[^>]+>", "", m[1])
        slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
        items.append(f'<li><a href="#{slug}">{text}</a></li>')
        return f'<h2 id="{slug}"><a class="anchor" href="#{slug}">{m[1]}</a></h2>'

    body = re.sub(r"<h2>(.*?)</h2>", anchor, body)
    label = TEXT[lang]["toc"]
    return body, (f'<nav class="toc" aria-label="{label}"><p>{label}</p><ul>{"".join(items)}</ul></nav>'
                  if len(items) > 1 else "")


def links(lang: str, body: str, where: str) -> str:
    def fix(m: re.Match) -> str:
        if m[1] not in SLUGS:
            raise SystemExit(f"{where}: link to an unknown page {m[1]}.html")
        return f'href="{page_path(lang, m[1])}{m[2] or ""}"'
    return LINK.sub(fix, body)


def structured(lang: str, slug: str, meta: dict, image: str) -> str:
    """JSON-LD: the page as an article of the documentation, and its breadcrumb."""
    url = ORIGIN + page_path(lang, slug)
    crumbs = [(TEXT[lang]["home"], ORIGIN + prefix(lang) + "/"),
              (TEXT[lang]["docs"], ORIGIN + page_path(lang, "index"))]
    if slug != "index":
        crumbs.append((meta["nav"], url))
    data = [
        {"@context": "https://schema.org", "@type": "TechArticle", "headline": meta["title"],
         "description": meta["description"], "inLanguage": lang, "url": url, "image": image,
         "isPartOf": {"@type": "WebSite", "name": "skemo", "url": ORIGIN + "/"},
         "about": {"@type": "SoftwareApplication", "name": "skemo", "applicationCategory": "DeveloperApplication"}},
        {"@context": "https://schema.org", "@type": "BreadcrumbList", "itemListElement": [
            {"@type": "ListItem", "position": i + 1, "name": name, "item": item}
            for i, (name, item) in enumerate(crumbs)]},
    ]
    return "".join(f'<script type="application/ld+json">{json.dumps(d, ensure_ascii=False)}</script>' for d in data)


def head(lang: str, slug: str, meta: dict, image: str) -> str:
    url = ORIGIN + page_path(lang, slug)
    title = html.escape(meta["title"])
    desc = html.escape(meta["description"])
    alternates = "".join(f'<link rel="alternate" hreflang="{other}" href="{ORIGIN}{page_path(other, slug)}">'
                         for other in LANGS)
    return (f'<title>{title} | skemo</title>\n'
            f'<meta name="description" content="{desc}">\n'
            f'<link rel="canonical" href="{url}">\n'
            f'{alternates}<link rel="alternate" hreflang="x-default" href="{ORIGIN}{page_path("en", slug)}">\n'
            f'<meta property="og:type" content="article"><meta property="og:site_name" content="skemo">'
            f'<meta property="og:locale" content="{"fr_FR" if lang == "fr" else "en_GB"}">\n'
            f'<meta property="og:title" content="{title}"><meta property="og:description" content="{desc}">'
            f'<meta property="og:url" content="{url}"><meta property="og:image" content="{image}">\n'
            f'<meta name="twitter:card" content="summary_large_image">\n'
            f'{structured(lang, slug, meta, image)}')


def build(skemo: str, out: pathlib.Path) -> list[dict]:
    """Write every documentation page of every language under `out`; return the pages for the sitemap."""
    version = subprocess.run([skemo, "--version"], check=True, capture_output=True, text=True).stdout.split()[-1]
    template = (SITE / "template.html").read_text()
    pages, count = [], 0
    for lang in LANGS:
        sources, titles = {}, {}
        for slug, _ in NAV:
            where = f"pages/{lang}/{slug}.html"
            path = SITE / "pages" / lang / f"{slug}.html"
            if not path.exists():
                raise SystemExit(f"{where}: missing (every page exists in every language)")
            sources[slug] = parse_header(path.read_text(), where)
            titles[slug] = sources[slug][0]["nav"]
        TITLES[lang].update(titles)
        other = LANGS[1] if lang == LANGS[0] else LANGS[0]
        for i, (slug, _) in enumerate(NAV):
            meta, body = sources[slug]
            renderer = Renderer(skemo, out, lang)
            body = TAG.sub(renderer.expand, body).replace("{{version}}", version)
            body = links(lang, body, f"pages/{lang}/{slug}.html")
            body, contents = toc(lang, body)
            image = renderer.card(renderer.first, slug) if renderer.first else f"{ORIGIN}/og.png"
            count += renderer.count
            crumb = "" if slug == "index" else (
                f'<nav class="crumbs" aria-label="Breadcrumb"><a href="{page_path(lang, "index")}">'
                f'{TEXT[lang]["docs"]}</a> / <span>{html.escape(meta["nav"])}</span></nav>')
            values = {
                "lang": lang, "head": head(lang, slug, meta, image), "nav": sidebar(lang, slug, titles),
                "toc": contents, "body": crumb + body, "pager": pager(lang, i, titles), "version": version,
                "header": header(lang, page_path(other, slug)), "footer": footer(lang, version),
                **{f"t:{k}": v for k, v in TEXT[lang].items()},
            }
            page = re.sub(r"\{\{([\w:]+)\}\}", lambda m: values[m[1]], template)
            target = out / page_path(lang, slug).strip("/") / "index.html"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(page)
            pages.append({"path": page_path(lang, slug), "lang": lang, "slug": slug,
                          "title": meta["title"], "description": meta["description"]})
    shutil.copy(SITE / "assets" / "style.css", out / "docs" / "style.css")
    print(f"documentation: {len(pages)} pages, {count} renderings (skemo {version})")
    return pages


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skemo")
    parser.add_argument("--out", default=str(SITE / "dist"))
    args = parser.parse_args()
    out = pathlib.Path(args.out)
    shutil.rmtree(out, ignore_errors=True)
    (out / "fonts").mkdir(parents=True)
    build(skemo_binary(args.skemo), out)
    for font in FONT_FILES:
        if (FONTS / font).exists():
            shutil.copy(FONTS / font, out / "fonts" / font)
    # the brand (logo, colours, header and footer styles) belongs to the site at the repository's root
    brand = SITE.parent.parent / "site" / "src"
    shutil.copy(brand / "brand.css", out / "brand.css")
    shutil.copy(brand / "favicon.svg", out / "favicon.svg")
    shutil.copytree(brand / "brand", out / "brand")
    print(f"written to {out} (serve it as a site root: python3 -m http.server -d {out})")


if __name__ == "__main__":
    main()
