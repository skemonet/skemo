# skemo.net — the public landing page

Static, single-page site that presents skemo. Its main argument is **sobriety**: an AI agent that writes a
`.skemo` file emits about 12 times fewer tokens than one that draws the same picture as SVG, so it spends less
compute and energy (and the layout runs on the user's CPU in milliseconds).

```bash
python3 site/build.py            # → site/dist/ (needs target/release/skemo, or SKEMO=/path/to/skemo)
python3 -m http.server -d site/dist 8000
```

| Path | Role |
|---|---|
| `src/index.html` | the page, with placeholders filled at build time (see `build.py`) |
| `src/style.css`, `src/favicon.svg` | styles (light and dark), icon |
| `examples/` | diagrams that only belong to this page (the hero) |
| `gallery/` | the measured gallery examples |
| `tools/measure_tokens.py` | the token measurement behind every figure on the page |
| `build.py` | renders each diagram with skemo (inline SVG, light and dark), runs the measurement, copies the fonts and the agent skill |

Every number on the page comes from `tools/measure_tokens.py`, re-run at each build on `gallery/`: none is
typed by hand. The page has no script, no tracker and no cookie. The agent skill (`cli/skill/`) is
published under `/skill/` so that agents can download it with `curl`.

**Deployment**: `.github/workflows/pages.yml` builds skemo and this page on every push to `main` that touches
`site/` (or by hand), and publishes it on GitHub Pages at <https://skemo.net>. It is the repository's only GitHub workflow.
