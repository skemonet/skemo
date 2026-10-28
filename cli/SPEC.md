# Requirements — CLI binary (`cli`)

> **A single binary**, with no runtime dependency (no Node, no Chromium, no Graphviz), that exposes the engine on the command line.
> Binary name: `skemo` (decision D-009).

## 1. Commands

| Command | Effect | Prio |
|---|---|---|
| `skemo render <file> -o <output>.{html,svg,png}` | rendering; format deduced from the extension | M |
| `  --css <custom.css>` (repeatable) | CSS applied after the default theme | M |
| `  --scale <n>` | PNG scale (default 2) | M |
| `  --step <n>` | rendering of the state at step n (value as written: `2.5`) | S |
| `  --animated` (PNG) | **one PNG per state** (`name.00.png`…) + `name.states.json`; without effects, with focus | M |
| `-o <output>.gif` | **animated GIF** with the effects; options `--hold 1.6s`, `--transition 0.45s`, `--fps 20`, `--loop/--no-loop`, `--final-hold 3s` | M |
| `-o <output>.apng` | **animated PNG**: exact colours, about 2× lighter than the GIF | S |
| `-o <output>.webp` | **WebP** (static and animated), planned (D-034) | S |
| `  --animated` (SVG) | meaningless: the SVG is **always** marked (`data-step`…) and never animated by the engine; option refused with an explicit message | M |
| `  --theme light\|dark\|auto\|print` | rendering theme (replaces `diagram(theme=…)`); `auto` = HTML only | M |
| `  --base-theme <file.css>` | replaces the default theme | S |
| `  --contrast high` | enhanced contrast: texts below the WCAG threshold are darkened or lightened, layout unchanged (D-026) | S |
| `  --transparent` | transparent background (SVG, PNG, APNG, contact sheet); no effect in HTML and GIF (D-029) | S |
| `  --board` (PNG) | contact sheet of the steps (one state per row) | S |
| `skemo check --contrast <file> [--css …]` | checks the contrast in the 3 themes and lists the faulty types | S |
| `skemo check <file>…` | validates without rendering; diagnostics; non-zero exit code on error | M |
| `skemo layout <file> --json` | full layout in the oracle's `layout.json` format (on stdout) | M |
| `skemo watch <file> [-o …]` | re-render on every save + local server with automatic reload (`--port`) | S |
| `skemo fmt <file>… [--check]` | canonical formatting (`lang` F-PARSE-8) | S |
| `skemo theme --print` | writes the default theme (starting point of a custom theme) | S |
| `skemo licences` | prints the licences and copyright notices of what the binary embeds: the fonts (OFL-1.1), the Lucide icons (ISC), the Rust crates compiled in (`src/crates.txt`, written by `tools/gen-licences.py`, checked against `Cargo.lock`) | S |
| `skemo init [folder]` | creates a sample `.skemo` + a commented `custom.css` | C |

Global options: `--quiet`, `--color auto|always|never`, `--lang en|fr`, `--version`, `--help`. **Language** (D-038): help, diagnostics and messages follow the system language (`SKEMO_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`), English by default.

## 2. Behaviour

- Read from stdin if the file is `-`; write to stdout if `-o -` (format through `--format`).
- **Exit codes**: `0` OK · `1` errors in the file (diagnostics) · `2` wrong usage · `3` internal error.
- Diagnostics in `lang`'s format (line, column, excerpt, suggestion), in colour if a terminal.
- Warnings (unknown tag, unreachable waypoint…) on stderr, without making the command fail, except with `--deny-warnings`.

## 3. Non-functional requirements

| ID | Requirement |
|---|---|
| C-NF-1 | **Single binary**, static if possible (musl on Linux). Targets: Linux x86_64/arm64, macOS arm64, Windows x86_64 |
| C-NF-2 | Default theme and **fonts embedded** in the binary |
| C-NF-3 | Size < 25 MB (indicative); start-up < 50 ms; rendering of the examples < 300 ms |
| C-NF-4 | Automated releases: one binary per platform, the SHA-256 of each in the release's description (done: `release.yml`; archives and `.sha256` files until `v0.1.0-rc.2`); installation by the `install.sh` script (`curl … | sh`, checksum verified; done) and by `cargo install` (to come, after publication) |
| C-NF-5 | `watch` mode only embeds a mini HTTP server (`tiny_http`) and polls the files every 300 ms (no `notify` dependency) |

## 4. Tests

- End-to-end tests (`assert_cmd`) on the examples of the `engine/fixtures` repository: `render` produces a non-empty PNG, `layout --json` = `engine`'s parity, `check` detects each error of `lang`'s error files; `watch` serves the page; `init` creates a renderable sample.
- Snapshots of `--help`.

## 5. Milestones

| Milestone | Contents | Depends on |
|---|---|---|
| **C1** | `check`, `layout --json` | lang L1, engine E1 |
| **C2** | `render` html/svg/png | engine E3 |
| **C3** | `--step`, `watch`, `fmt`, releases | engine E4, lang L3 |

## 6. Licence

The engine's (constrained by elk-rs's EPL dependency); include the dependencies' notices in the releases (`cargo-about` or equivalent).
