# docs.skemo.net — the documentation site

User documentation of the skemo language and tools, in English: 16 pages from getting started to the
command line, with every example rendered by skemo itself next to its source.

```bash
python3 site/build.py                    # → site/dist/
python3 site/build.py --skemo ~/bin/skemo   # or SKEMO=…; default: ../cli/target/release/skemo, then the PATH
python3 -m http.server -d site/dist 8001
```

| Path | Role |
|---|---|
| `pages/*.html` | the pages, as HTML fragments; the order and titles are in `NAV` (`build.py`) |
| `examples/*.skemo`, `*.css` | every diagram shown, as real files |
| `template.html`, `assets/` | layout (sidebar, table of contents, pager), styles, icon |
| `build.py` | renders `<skemo name="…">` tags as source + inline SVG (light and dark), animation steps as WebP |

**Rules**

- `FORMAT.md` stays the contract; these pages explain it for users. A format change updates both.
- A new example is a file in `examples/`; `cargo test` (`tests/site_examples.rs`) checks that every example
  parses without any diagnostic, and the build fails on any warning (`--deny-warnings`).
- Features that are implemented but not validated yet are marked *preview* (icons and links today); proposals
  that are not implemented (concepts, v0.7) are not documented here.

Needs only Python 3 and a `skemo` binary; no dependency, no JavaScript in the output.
