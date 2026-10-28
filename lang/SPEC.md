# Requirements — Diagram language (`lang`)

> This repository defines **the language**: the format (`FORMAT.md`, which is the contract), its parser, its data model, its default theme and style resolution.
> It does not measure text, lay out anything or render anything: that is the engine's job (`engine`).
> Name: **skemo** (decision D-009).

## 1. Repository contents

| Element | Role |
|---|---|
| `FORMAT.md` | **format contract**. Any evolution starts here |
| `lang` crate | parser, model, validation, diagnostics, surgical editing, formatting |
| `theme/default.css` | official default theme, visually equivalent to `default.reference.css` (the prototype's theme) |
| style resolution | CSS cascade (theme + custom CSS) on a **subset useful to the engine** (§4) |
| `fixtures/` | `*.pug` → expected `*.model.json` (extracted from the prototype's oracle); `process.css` (sample custom theme) |

## 2. Data model

It must be serialisable (serde) to JSON **compatible** with `fixtures/*.model.json`, hence with the prototype. Fields may be added, but existing ones not renamed:

```
Model   { options: { dir?, engine?, router?, elk? }, nodes: [Node], edges: [Edge] }
Node    { id, kind: "card"|"zone", parent: id|null, classes: [..], title, tag: str|null,
          items: [str], at: [x,y]|null, size: [w,h]|null, step: int|null, fx: str|null,
          date: str|null, start: str|null, end: str|null,         // timeline mode (raw text, validated separately)
          anim: Anim|null }
Edge    { …, anim: Anim|null }        // same for links
Li      { text, anim: Anim|null }     // [Rust] content lines become objects (items: [str] is still exported for compatibility)
Anim    { start: Step|null, until: Step|null, fx: "fade"|"up"|"zoom"|"draw"|null, focus: [{ from: Step, to: Step|null }] }
Step    { raw: "2.5", value: 2.5 }    // raw: copied as is into data-step
Edge    { id: "from->to", from, to, classes: [..], label, via: [[x,y]..]|null,
          side: "start-end"|null, step: int|null, fx: str|null }
```

Each element also keeps its **positions in the text** (start and end byte of the tag and of each attribute), in a separate structure: it is the basis of surgical editing.

## 3. Requirements — parsing and editing

Priorities: **M** = mandatory (v1), **S** = desirable, **C** = later.

| ID | Requirement | Prio |
|---|---|---|
| F-PARSE-1 | Read any file valid according to `FORMAT.md`, and produce the model of `fixtures/*.model.json` (structural equality) | M |
| F-PARSE-2 | Pug syntax of the subset: `tag#id.c1.c2(attr="v" flag n=3) text`, indentation with spaces **or** tabs (no mixing), comments `//-` and `//` (with their block), piped text `\| …` | M |
| F-PARSE-3 | Unknown tags: ignored with a **warning** | M |
| F-PARSE-4 | Blocking errors: unknown target, duplicate id, `to` outside a card, several attribute groups, mixed indentation, malformed `at`/`size`/`via` | M |
| F-PARSE-5 | Diagnostics: file, line, column, excerpt, suggestion ("target `cor` unknown, did you mean `core`?"). Rendering in the style of `ariadne` or `miette` | M |
| F-PARSE-6 | Arrow id `from->to`, suffixed `#2`, `#3`… on duplicates; explicit id `to#e1(target)` | M |
| F-PARSE-7 | **Surgical editing**: `set_attr(element, name, value)`, `remove_attr`, `add_class`, `remove_class`, `move_card(id, x, y)` (= `set_attr at`), which modify **only** the bytes concerned (comments, order and indentation intact) | M (editor's basis) |
| F-PARSE-8 | `fmt`: canonical formatting (2-space indentation, attribute order `at size step fx side via`); idempotent | S |
| F-PARSE-9 | Parser **without panic** (fuzzing) and WebAssembly-compatible | M |
| F-PARSE-13 | **`@` animation markers** (`FORMAT.md` §3.8) on `card`, `zone`, `li` and `to`: only **final** tokens are read, removed from the displayed text, `\@` escape, at most one appearance marker, several `:focus`. The model keeps the values **as written** (text `"2.5"`) and their numeric value, and the position of the markers in the source (for the editor). Old `step=`/`fx=` notation accepted with a warning, converted by `fmt`; both notations on a single node = error | M |
| F-PARSE-10 | `at`/`via` values written by editing: integers (no `120.0000001`); rounding option to a grid (open decision no. 5) | S |
| F-PARSE-12 | **Frieze mode** (`FORMAT.md` §3.7): `layout="frise"`, `scale` = `year|decade|century|millennium`; negative years (BC), **year 0 rejected**, optional `-MM-DD`; `end` = **bound** (not an included period); error if a zone is present (registers: later). Conversion utility to astronomical years, shared with the engine | C (post-v1) |
| F-PARSE-11 | **Timeline mode** (`FORMAT.md` §3.6): attributes `layout`, `from`, `to`, `scale`, `width`, `today` on `diagram`; `date`, `start`, `end` on `card`. Validation: date in the format `YYYY[-MM[-DD]]`, `date` **or** `start` mandatory in timeline mode, `end ≥ start`, warning if `at=`. Utility function to convert to days (inclusive `end` rule), shared with the engine | C (post-v1) |

## 4. Requirements — styles

The language **owns** the type system and the theme. The engine asks it: "for this card, what are the font, the size, the margins, the badge text…?"

| ID | Requirement | Prio |
|---|---|---|
| F-STYLE-1 | Default theme **embedded** (`include_str!`), equivalent to `default.reference.css` | M |
| F-STYLE-2 | Custom CSS applied after the theme (cascade, specificity, order) | M |
| F-STYLE-3 | Types in CSS: `--c` (accent colour), `--label` (badge); `tag="…"` replaces `--label`; card without badge = `no-tag` class | M |
| F-STYLE-4 | Zones: `--z-bg`, `--z-border`, `--z-ink`; margins `--zone-pad`, `--zone-head` | M |
| F-STYLE-5 | Theme arrow styles: `.dashed .dotted .thick .flow .both .nohead .dot` (+ examples `.wan .secure`); colour through `color` | M |
| F-STYLE-6 | **Resolved style** per element, on the "geometric" subset: `padding*`, `border*-width`, `font-family`, `font-size`, `font-weight`, `line-height`, `letter-spacing`, `min-width`, `max-width`, `li` margins, CSS variables. The rest of the CSS is passed as is to the HTML rendering | M |
| F-STYLE-7 | Rule "IP at the start of an `li` → `span.mono`", extensible (IPv6, dates, amounts) | S |
| F-STYLE-8 | Dark mode (`prefers-color-scheme`) in the default theme | S |
| F-STYLE-10 | Frieze mode theme: `card.event`, `card.era[.outside]`, `.fr-axis`, `.fr-tick`, `.fr-tick-label`, `.fr-century`, `.fr-stem`, `.fr-dot` | C (post-v1) |
| F-STYLE-13 | **Looks**: `band` (default: the name sits in the coloured band), `header` and `classic` stay available (`--look`, `diagram(look=…)`; D-036). The default theme (`DEFAULT_THEME`) and the frozen prototype theme (`REFERENCE_THEME`, oracle) are separate | M |
| F-STYLE-11 | **"Header" style**: `--look: header` (per type), or `diagram(look="header")`; the resolved style exposes the `look`, the type label (band) and the `tag` (chip) separately (in plain style, `tag` replaces `--label`) | M |
| F-STYLE-12 | **Secondary cards** `.note`, `.chip`, `.info` and **zone** `.bare` (legend at the bottom) provided by the theme; they force `--look: plain` | M |
| F-STYLE-13 | **Derived tints**: implement nested `color-mix(in srgb, …)` and `var()` in style resolution, so that the engine gets **concrete colours** (essential for SVG and PNG, which have no CSS engine). Respect CSS semantics: a variable that contains `var(--c)` is resolved **where it is declared** (pitfall P27); accent fills are computed per element with `--fill-mix` | M |
| F-STYLE-14 | **Themes** `light`, `dark`, `print` (and `auto` for HTML): sets of base variables per theme; `@media print` in the exported CSS | M (`auto`: S) |
| F-STYLE-15 | **Contrast** verified automatically on all test sets and all themes: text on surface ≥ 4.5:1; text on accent fill ≥ 3:1 (`light`, `dark`) and ≥ 4.5:1 (`print`); CI failure otherwise. Report listing the faulty types of a custom CSS | M |
| F-STYLE-16 | Automatic `from-<type>` and `to-<type>` classes on arrows (provided by the engine, documented here); `.link` style (dotted without head) | M |
| F-STYLE-9 | Timeline mode theme: `card.milestone` (diamond), `card.phase` (bar `--bar-w`), `.tl-axis`, `.tl-tick`, `.tl-grid`, `.tl-mark`, `.tl-today` (see `default.reference.css`) | C (post-v1) |

CSS library: **lightningcss**, or **cssparser** + **selectors** (those of Servo and Firefox). To settle according to the WASM size. *(Settled in D-022: an in-house engine of about 900 lines.)* Only simple selectors are needed: element, class, id, descendant, child, `:not`, attribute.

## 5. Test sets (`fixtures/`)

| Source | Expected |
|---|---|
| `reseau.pug` | `reseau.model.json` |
| `reseau-manuel.pug` | `reseau-manuel.model.json` (`at` everywhere) |
| `reseau-manuel-via.pug` | `reseau-manuel-via.model.json` (`side`, `via`, `step`, `fx`, arrow styles) |
| `reseau-2pins.pug`, `reseau-conflit.pug` | partial `at` |
| `process.pug` (+ `process.css`) | `process.model.json` (`tag="FIN"`, `dir="right"`, loop) |
| `timeline.pug` (+ `timeline.css`) | `timeline.model.json` (timeline mode: milestones, periods, lanes, steps) |
| `frise.pug` (+ `frise.css`) | `frise.model.json` (frieze mode: BC years, `-44-03-15`, eras, events, steps) |
| `design.pug` (+ `design.css`) | `design.model.json` (header style, `tag` as chip, `.note`, `.chip`, `.info`, `.bare`, `.link`, `side`) |
| `reseau-entete.pug` | `reseau-entete.model.json` (`look="header"` on the classic diagram) |
| `animation.pug` | `@` markers: expected texts after removal, raw values kept (see `engine/fixtures/animation.expected.json`, `texts` keys) |

*(These files are the prototype's oracle and keep their French labels.)* Also present: error cases (one file per error of F-PARSE-4, with the expected diagnostic as an `insta` snapshot, in `fixtures/erreurs/`), editing round trips (F-PARSE-7: the diff touches a single line) and `fmt` idempotence.

## 6. Milestones

| Milestone | Contents | Criterion |
|---|---|---|
| **L1** | Parser + model + diagnostics (F-PARSE-1 to 6, 9); `check` test command | parity on all `fixtures/`; fuzzing 10 min without panic |
| **L2** | Styles (F-STYLE-1 to 6, 11 to 16) | resolved style consistent with the prototype's CSS (unit tests per property); contrast verified in the 3 themes |
| **L3** | Editing and formatting (F-PARSE-7, 8, 10) | round trips on all examples |
| **L4** *(post-v1)* | Timeline and frieze modes (F-PARSE-11 and 12, F-STYLE-9 and 10) | parity on `timeline.model.json` and `frise.model.json`; date errors (including year 0) as snapshots |

## 7. Licence

Targeted: **MIT OR Apache-2.0** (no copyleft dependency).
