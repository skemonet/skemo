# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/) (pre-1.0: minor versions may break the format).

## [Unreleased]

## [0.1.0-rc.4] — 2026-10-31

- Release binaries named by system and processor, the same in every release: `skemo-linux-x86_64`,
  `skemo-linux-arm64`, `skemo-macos-arm64`, `skemo-windows-x86_64.exe` (until 0.1.0-rc.3:
  `skemo-<version>-<target>`; `install.sh` reads both). The release notes say which file to take.
- Checks, binaries and releases run on the maintainers' CI (`.gitlab-ci.yml`); GitHub only builds the site.
- skemo.net: the source of the landing page is in `site/`; the token comparison counts a lean SVG (no embedded
  source, no class or data attributes), which brings the ratio from about 17 to about 12.

## [0.1.0-rc.3] — 2026-10-29

First release from the public repository.

- CI: actionlint installed from its release; longer timeout for the quality job.
- engine: range check written for clippy 1.97 (`manual_range_contains`).

## [0.1.0-rc.2] — first public version of the code

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

[Unreleased]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.4...HEAD
[0.1.0-rc.4]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.3...v0.1.0-rc.4
[0.1.0-rc.3]: https://github.com/skemonet/skemo/releases/tag/v0.1.0-rc.3
[0.1.0-rc.2]: https://github.com/skemonet/skemo/tree/28d0a43
