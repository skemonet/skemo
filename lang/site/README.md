# skemo.net/docs/ — the documentation

User documentation of the skemo language and tools, in English (`/docs/`) and French (`/fr/docs/`): 23 pages per
language, from getting started to the command line. Every example is rendered by skemo itself, with its source in a
second tab.

```bash
python3 site/build.py                         # the whole site, landing page included → site/dist/ (repository root)
python3 lang/site/build.py --out /tmp/docs    # the documentation alone, same paths (docs/, fr/docs/, fonts/)
python3 -m http.server -d site/dist 8000      # serve it as a site root: links are absolute (/docs/…)
```

| Path | Role |
|---|---|
| `pages/en/*.html`, `pages/fr/*.html` | the pages, as HTML fragments with a header comment (`title`, `nav`, `description`); the order and the sections are `NAV` in `build.py` |
| `examples/*.skemo`, `*.css` | every diagram shown, as real files |
| `examples/fr/` | French versions of the diagrams whose text is read (a page falls back on the English one) |
| `template.html`, `assets/` | layout (sidebar, table of contents, pager), styles; header, footer, colours and fonts come from the site's brand (`site/src/brand.css`, `site/BRAND.md`) |
| `build.py` | renders `<skemo name="…">` tags as inline SVG (light and dark) with a source tab, animation steps as WebP, a social card per page; writes canonical URLs, language alternates and structured data |

**Sections**: *What you can draw* (one page per kind of diagram, the pages search engines should bring people to),
*Where it fits* (agents, diagrams as code), then the language reference, style and tools.

**Rules**

- `FORMAT.md` stays the contract; these pages explain it for users. A format change updates both, in both languages.
- Every page exists in every language under the same file name (the build fails otherwise); links between pages are
  written `arrows.html#anchor` and become `/docs/arrows/#anchor` or `/fr/docs/arrows/#anchor`.
- A new example is a file in `examples/` (and in `examples/fr/` when it has text to translate); `cargo test`
  (`tests/site_examples.rs`) checks that every example parses without any diagnostic, and the build fails on any warning
  (`--deny-warnings`).
- Diagrams are rendered when the site is built, never in the reader's browser: the pages stay light (no WebAssembly) and
  the diagrams' text is indexed.
- Features that are implemented but not validated yet are marked *preview*.

Needs only Python 3 and a `skemo` binary; no dependency, no JavaScript in the output.
