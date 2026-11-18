# Requirements — CLI binary (`cli`)

> **A single binary**, with no runtime dependency (no Node, no Chromium, no Graphviz), that exposes the engine on the command line.
> Binary name: `skemo` (decision D-009).

## 1. Commands

| Command | Effect | Prio |
|---|---|---|
| `skemo render <file> -o <output>.{html,svg,png}` | rendering; format deduced from the extension | M |
| `  --css <more.css>` (repeatable) | CSS applied after the default theme and the stylesheets the diagram includes (`diagram(css=…)`, D-082) | M |
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
| `  --motion` (SVG) | plays the arrows in motion (`.flow`, `.pulse`) with CSS keyframes; without it the SVG is a still image, moving arrows drawn solid (D-079). Other formats: a warning (HTML, GIF, APNG, animated WebP always play them; PNG never) | S |
| `  --board` (PNG) | contact sheet of the steps (one state per row) | S |
| `skemo lint --contrast <file> [--css …]` | checks the contrast in the 3 themes and lists the faulty types | S |
| `skemo lint <file>…` | validates without rendering; diagnostics; non-zero exit code on error | M |
| `skemo layout <file> --json` | full layout in the oracle's `layout.json` format (on stdout) | M |
| `skemo render <file> --watch [-o …]` | re-render on every save + local server with automatic reload (`--port`) | S |
| `skemo lint --fix <file>… [--dry]` | canonical formatting (`lang` F-PARSE-8) | S |
| `skemo --licences` | prints the licences and copyright notices of what the binary embeds: the fonts (OFL-1.1), the Lucide icons (ISC), the Rust crates compiled in (`src/crates.txt`, written by `tools/gen-licences.py`, checked against `Cargo.lock`) | S |
| `skemo capabilities [--format json]` | version, version of the SVG conventions, features (§2.4) | M |
| `  --diagnostics json` (every command) | messages as JSON lines on stderr (§2.2) | M |
| `  --allow <code>` (every command) | the warnings of that code are neither shown nor counted by `--deny-warnings` (§2.8, D-098) | S |
| `  --root <dir>` (render) | the folder the diagram's own files are read in (§2.3) | M |
| `skemo render --batch` | many diagrams in one process (§2.5) | S |
| `skemo site <dir> -o <out>` | a website from a folder of diagrams, the links between `.skemo` files followed by a click (§2.7, D-097) | S |
| `skemo init [file] [--css [name]]` | creates a sample `.skemo` (default `diagram.skemo`); `--css` also a commented stylesheet (default `skemo_style.css`) that it includes with `diagram(css=…)` (D-082) | C |

Generic options: `--help`, `--version`, `--quiet`, and `--licences` alone (no `--lang`, no `--color`: colours on a terminal unless `NO_COLOR`; D-082, D-084). **Language** (D-038): help, diagnostics and messages follow the system language (`SKEMO_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`), English by default.

## 2. Behaviour

- Read from stdin if the file is `-`; write to stdout if `-o -` (format through `--format`).
- **Exit codes**: `0` rendered (warnings allowed, unless `--deny-warnings`) · `1` the diagram is refused:
  errors in it, a file it names missing or refused (§2.3), over the limits (§2.6), or warnings with
  `--deny-warnings`; nothing is written · `2` misuse of the command line (an unknown option, a
  `--css` file that cannot be read) · `3` internal error; any other code (a panic: 101) is a crash, to
  report.
- Diagnostics in `lang`'s format (line, column, excerpt, suggestion), in colour if a terminal.
- Warnings (unknown tag, unreachable waypoint…) on stderr, without making the command fail, except with `--deny-warnings`.

### 2.1 Contract for the tools that run skemo

Asked by glitu (its requirements SK-0 to SK-11), kept for any tool: `skemo render - -o - --format svg
[options]`

1. reads the source on the standard input only and writes the SVG on the standard output only;
   writes no file, opens no window, browser or network connection (only `--watch` does, and `-o FILE`
   writes); works in a read-only folder;
2. never asks a question; `-` on a terminal is refused (exit 2) rather than waited on;
3. no colour nor escape sequence on the standard error unless it is a terminal (and never with
   `--diagnostics json`, nor with `NO_COLOR`);
4. the exit codes above; with `1`, nothing on the standard output;
5. the messages' language: `SKEMO_LANG` (`en`, `fr`) wins over the system's;
6. the same bytes as the library: `skemo_engine::from_source` with the stylesheets given and the rule
   `.stage { --font-scale: k; }`, `svg::render`, `source::embed_text` with the bundle (source, the
   stylesheets under their file names, `font_scale`, images); `--transparent` for a page's
   background. The same source, options, files and version give the same bytes (`engine/SVG.md` §4).

Tests: `tests/contract.rs`.

### 2.2 Diagnostics as JSON lines (`--diagnostics json`)

On the standard error, one JSON object per line, nothing else (the notes such as `a.skemo → a.png` are
left out):

```json
{"severity":"error","code":"unknown-target","message":"unknown target `db`","help":"did you mean `da`?","file":"-","line":4,"column":9,"end_line":4,"end_column":11}
```

- `severity`: `error` or `warning`; `code`: stable. The language's codes (`skemo_lang::Diagnostic`), the
  steps' (`no-step-after`, `step-gap`, `focus-outside`, `highlight-outside`, `step-before-parent`,
  `until-after-parent`), the files' (`file-missing`, `file-outside-root`, `file-remote`, `file-special`,
  `file-too-large`, `file-not-text`, `image-type`), `too-large` (§2.6), `bad-request` (§2.5); the
  engine's (`unknown-icon`, `image-missing`, `font-missing`, `font-unreadable`, `unknown-layout`,
  `curve-ignored`, `overlap`, `wrap-hides-cards`, `mindmap-unlinked`, `fit`, and the router's
  `unroutable`, `waypoint-unreachable`, `side-ignored`, `route-input`), with the line of the element
  they are about when there is one; the stylesheets' (`css-syntax`: a `{` never closed, a `}` without `{`, a
  declaration outside a rule or without `:`; `css-property`: a typo of a property skemo or CSS knows,
  `colr`, `--lable`), at their place in the stylesheet (`file` = its name as given); a failure without its own code carries `refused`,
  `usage` or `internal` after its exit code; a panic still prints a last line with `internal`.
- `message`, `help` (optional): in the language of §2.1.5. Names are quoted with backticks in English
  (`` unknown target `db` ``), with « » in French (D-094).
- `file`: `-` for the source on the standard input, otherwise the source's path as given; the name
  written in the diagram for a stylesheet it includes or an image.
- `line`, `column`, `end_line`, `end_column`: from 1, columns in Unicode characters; the end is just
  after the last character. Absent for a problem without a place (a file).

### 2.3 Files read inside a root (`--root`)

The files a diagram makes skemo read, its images and the stylesheets of `diagram(css=…)`, are read in
the **root folder** only: `--root DIR`, by default the source's folder (the current folder for `-`).
Refused, with a diagnostic and exit 1: an absolute path, a `..`, a URL; a symbolic link whose target is
outside the root (checked once resolved); a folder, a device or another special file; an image larger
than 1 MiB (a stylesheet: 1 MiB) or whose content is not what its extension says (a `.svg` holding a
PNG). Never the network. The files of the command line (`--css`, `--base-theme`, `--font`) are the
user's and are read wherever they are.

### 2.4 Version and capabilities

`skemo capabilities --format json`:

```json
{"version":"0.1.0","svg_conventions":1,"features":["diagnostics-json","highlights","root","batch","limits","allow"]}
```

`version` is the crate's (semantic); `svg_conventions` the version of `engine/SVG.md`; `features` what
this binary supports among §2.2 to §2.8 and the highlight markers (FORMAT §3.8). `skemo --version` keeps
printing `skemo X.Y.Z`.

### 2.5 Many diagrams in one process (`--batch`)

`skemo render --batch [options]`: requests on the standard input, one JSON object per line; one answer
per request, in the same order, on the standard output, flushed after each:

```json
{"id":"slide4-1","source":"diagram\n  card#a A\n","css":["themes/crystal/skemo.css"],"font_scale":1.35}
{"id":"slide4-1","svg":"<svg …>","diagnostics":[]}
```

Keys of a request: `id` (any JSON value, copied into the answer), `source` (required), `css` (paths,
replacing `--css`), `font_scale`, `theme`, `transparent`, `no_source`; an unknown key is refused. The
other options come from the command line and apply to every request (`--root`, `--base-theme`,
`--contrast`, `--embed-fonts`, `--motion`, `--deny-warnings`). `diagnostics` holds the objects of §2.2;
a refused diagram answers `"svg": null` and the process goes on; a line that is not a request answers
with a `bad-request` diagnostic. The process ends at the end of the input. The answers are the SVG
`render - -o - --format svg` writes with the same options.

### 2.6 Bounded work

A diagram is refused (exit 1, code `too-large`) before any layout beyond: 512 KiB of source, 1000 cards
and zones, 1000 arrows, 5000 lines. These bound the size, not every case: the layout time grows with
the arrows and their crossings (measured on one core: a tree of 600 cards, 1.3 s; 200 cards with 400
arrows drawn at random, 20 s; 400 with 800, 3 min). A tool should keep its own timeout.

### 2.7 A website from a folder of diagrams (`site`)

`skemo site <dir> -o <out> [--title …] [--theme …] [--css …] [--base-theme …] [--deny-warnings]` (D-097):
to split a system too large for one drawing into views, each `.skemo` file of `<dir>` and its subfolders
(hidden folders, links and `<out>` left out) becomes a page at the same place (`shop/api.skemo` →
`shop/api.html`) showing its drawing, with pan (drag, arrow keys), zoom (wheel, pinch, `+` `-` `0`)
and, around it:

- the links between diagrams (`href="api.skemo"`, `href="../context.skemo#web"`, FORMAT §3.2.1) become
  links between the pages; a click zooms into the element, then opens its page; a fragment centres and
  flashes that element;
- a breadcrumb: the shortest way from `index.skemo` (the home page), or, without one, from the diagrams
  no other opens, to the current one;
- a side panel: every diagram by folder, with a filter; what the view opens and what it is opened from;
  its edge is dragged (or moved with the arrow keys) to set its width, kept from page to page in the
  reader's browser, a double-click going back to the default;
- a search box (`/`): the cards of every diagram (legends left out), found by every word typed in
  their title, category (`label=`), tag, zone, lines or diagram, accents and case ignored, titles
  first; a note is also found by the card it annotates. A result shows its diagram (with the one above
  it in the breadcrumb), its zone and its lines, and opens the diagram's page centred on the card
  (`page.html#id`). The index (`assets/search.js`) is a script loaded on first use, so that `file://`
  works;
- `index.html`: `index.skemo`'s page, or, without one, a thumbnail per diagram (PNG), the ones nothing
  opens first.

A diagram's title is its `title`, else its file name. Every diagram is read and drawn before anything is
written: one refused diagram refuses the site (exit 1), with the diagnostics of each. A link to a
diagram that is not in the folder (`site-missing-diagram`) or to a file that is not in it
(`site-missing-file`) is a warning at its line; other relative links are copied beside the pages. The
sources are copied beside their pages (a "Source" button). By default every diagram is drawn light and
dark (`--theme auto`) and the pages follow the reader's system, with a switch; `--theme light`, `dark` or
`print` draws one. `--css`
applies to every diagram: a diagram cannot include a stylesheet out of its own folder
(`diagram(css=…)`, §2.3), so a shared style is given there. An animated diagram shows its last step,
arrows in motion played. Pages need no server (`file://` works) and no network: fonts are written once
in `assets/fonts/`. The pages' own text is in English whatever the system language, so two machines
build the same site. An example: `cli/examples/site/` (`--css cli/examples/site/c4.css`).

### 2.8 Warnings left out (`--allow`)

`--allow <code>` (every command, repeatable, or `--allow a,b`; D-098): the warnings of that code
(§2.2) are not shown, in text or JSON, and do not count for `--deny-warnings`. Errors are never left
out; an unknown code is accepted and changes nothing, so that a tool can pass a code that an older or
newer skemo does not have. Feature `allow` in `capabilities`.

For a tool whose context makes a warning meaningless. glitu's case: a slide's text and its diagram
share one series of clicks, so a diagram's steps may skip the clicks of the text (`@1`, then `@8`);
glitu passes `--allow step-gap` and checks the steps against the slide's clicks itself.

Tests: `tests/contract.rs` (`allowed_codes_are_left_out`).

## 3. Non-functional requirements

| ID | Requirement |
|---|---|
| C-NF-1 | **Single binary**, static if possible (musl on Linux). Targets: Linux x86_64/arm64, macOS arm64/x86_64, Windows x86_64; the same file names at every release (`skemo-<target>`), with `SHA256SUMS` |
| C-NF-2 | Default theme and **fonts embedded** in the binary |
| C-NF-3 | Size < 25 MB (indicative); start-up < 50 ms; rendering of the examples < 300 ms |
| C-NF-4 | Automated releases: one binary per platform, the SHA-256 of each in the release's description (done: `.gitlab-ci.yml` and `tools/release.sh`; archives and `.sha256` files until `v0.1.0-rc.2`); installation by `cli/install.sh`, served at `https://skemo.net/install.sh` (`curl … | sh`, checksum verified; done) and by `cargo install` (to come, after publication) |
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
