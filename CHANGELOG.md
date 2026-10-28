# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/) (pre-1.0: minor versions may break the format).

## [Unreleased]

## [0.1.0-rc.2] — first public release

The first public version of skemo, as one repository.

- **Language** (`skemo-lang`): the `.skemo` format (a Pug dialect with `diagram`, `title`, `card`, `cards`,
  `zone`, `stack`, `to`), tables and lines in cards, icons and links, timeline, frieze and mind-map modes,
  step-by-step animation markers (`@1`, `@2:up`, `@3-5:focus`), an in-house CSS engine with light, dark and print
  themes, diagnostics with stable codes in English and French, canonical formatting and surgical editing that
  keeps comments and layout.
- **Engine** (`skemo-engine`): text measurement on embedded fonts, layered layout through elk-rs, pinned positions
  that never move the rest, orthogonal routing with forks, buses and shared trunks, arrow labels placed clear of
  cards, contrast checking and enforcement, and output as standalone HTML (with a step player), SVG, PNG, WebP,
  GIF and animated PNG. Rendered files carry their source and can be opened again.
- **WebAssembly** (`skemo-engine-wasm`): the same engine in the browser, the same `layout.json` as native, an
  editing API and a `<skemo-view>` element.
- **Router** (`skemo-router`): a generic orthogonal connector router with shapes, traversable regions, waypoints,
  forced sides, free ends and incremental updates; deterministic and panic-free.
- **CLI** (`skemo`): `render` (default), `check`, `fmt`, `watch`, `layout`, `fit`, `extract`, `init`, `theme`,
  `icons`, `licences`; messages in the system language; release binaries for Linux, macOS and Windows.
- **Agent skill**: `cli/skill/skemo/`, for AI agents that write diagrams.

[Unreleased]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.2...HEAD
[0.1.0-rc.2]: https://github.com/skemonet/skemo/releases/tag/v0.1.0-rc.2
