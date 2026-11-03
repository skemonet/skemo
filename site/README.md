# skemo.net — the public site

Static site that presents skemo, in English (`/`) and French (`/fr/`), with the documentation under `/docs/` and
`/fr/docs/` (built from `lang/site/`). Its arguments: a diagram is text you keep, review and rerun (the same image on every
machine), with every kind of diagram skemo draws shown on its own documentation page; and an AI agent that writes a
`.skemo` file emits about 12 times fewer tokens than one that draws the same picture as SVG.

```bash
python3 site/build.py            # → site/dist/ (needs target/release/skemo, or SKEMO=/path/to/skemo)
python3 -m http.server -d site/dist 8000
```

| Path | Role |
|---|---|
| `src/index.html`, `src/fr/index.html` | the landing page in each language, with placeholders filled at build time (see `build.py`) |
| `src/brand.css`, `BRAND.md` | the brand shared by every page (fonts, colours, header, buttons, footer) and its charter |
| `src/style.css`, `src/favicon.svg`, `src/brand/` | the landing page's layout (light and dark), icon, logo |
| `examples/`, `examples/fr/` | diagrams that only belong to this page (the hero) |
| `gallery/` | the measured gallery examples |
| `tools/measure_tokens.py` | the token measurement behind every figure on the page |
| `build.py` | renders each diagram with skemo (inline SVG, light and dark), runs the measurement, builds the documentation, writes `sitemap.xml`, `robots.txt`, `llms.txt`, `404.html` and the social card `og.png`, copies the fonts and the agent skill |

Every number on the page comes from `tools/measure_tokens.py`, re-run at each build on `gallery/`: none is typed by hand.
The site has no script, no tracker and no cookie; the diagram/source tabs are radio buttons and CSS. The agent skill
(`cli/skill/`) is published under `/skill/` so that agents can download it with `curl`, and `llms.txt` lists the
documentation for them.

**Search engines**: every page has its own title and description, a canonical URL, `hreflang` alternates between English
and French, an Open Graph card (a PNG of its first diagram) and JSON-LD (`SoftwareApplication` on the landing page,
`TechArticle` and `BreadcrumbList` on the documentation); `sitemap.xml` lists every page with its alternates.

**Deployment**: `.github/workflows/pages.yml` builds skemo and the site on every push to `main` that touches `site/`,
`lang/site/` or the skill (or by hand), and publishes it on GitHub Pages at <https://skemo.net>. It is the repository's
only GitHub workflow.
