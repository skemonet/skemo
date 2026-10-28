# Requirements — Rendering engine (`engine`)

> The **core** of the system: from a model (`lang` repository) and resolved styles, it **measures** the cards, **lays them out** (ELK), has the arrows **routed** (`router` repository), then **renders** as HTML, SVG or PNG, with the animation steps.
> It is compiled twice: as a **native library** (used by the CLI) and as **WebAssembly** (used by the editor and the slides).
> This repository is the "WebAssembly engine" of the requested split, but it also contains the native core shared with the CLI.

## 1. Organisation (Cargo workspace)

```
crates/
  engine/        library: measure, layout, route (orchestration), render, steps
  engine-wasm/   wasm-bindgen API + <skemo-view> custom element
fixtures/        prototype oracle (layout.json, png, html, .pug, process.css)
```

Dependencies: `lang` (model and styles), `router` (routes), `elk-rs` (layout), and text and rendering crates (§4).

## 2. Processing chain

```
Model + resolved styles (lang)
  1. measure   sizes of cards and labels (shaped text, embedded fonts)
  2. layout    ELK layered → positions (and indicative routes)
  3. pins      at= → pinned positions; zones = bounding boxes; overlaps reported (never corrected, D-033)
  4. route     scene for `router`: cards = Shape, zones = Region, pass_through = ancestor zones of the endpoints,
               side / via / hint (ELK route) → routes
  5. labels    label position (middle of the longest segment, with avoidance)
  6. render    HTML (positioned DOM + full CSS + player) · standalone SVG · PNG (resvg)
  7. steps     data-step / data-fx, inheritance, route lengths
```

## 3. Requirements

Priorities: **M** = mandatory (v1), **S** = desirable, **C** = later.

### Measurement

| ID | Requirement | Prio |
|---|---|---|
| E-MEAS-1 | Size of each card according to the resolved style (`lang`): badge + title + `li` lines, `min-width`/`max-width`, line wrapping | M |
| E-MEAS-2 | Size of arrow labels | M |
| E-MEAS-3 | **Embedded fonts** (Inter and a monospace, OFL), identical natively and in WASM; no system font | M |
| E-MEAS-4 | Gap ≤ 2 px from the oracle's `sizes` (same fonts) | M |

### Layout

| ID | Requirement | Prio |
|---|---|---|
| F-LAYOUT-1 | Hierarchical ELK layered layout, nested zones, `dir` = `down`, `right`, `up` or `left` | M |
| F-LAYOUT-2 | Prototype options: writing order respected, `LONGEST_PATH`, inline labels; cycle breaking `GREEDY_MODEL_ORDER` (D-041; the prototype's `MODEL_ORDER` reversed every arrow written after its target, kept only by the oracle parity test). Limit L3 fixed: siblings of a row are put back in writing order after ELK (D-041) | M |
| F-LAYOUT-7 | `wrap="N"` on a zone: grid of N children per row (D-041) | S |
| F-LAYOUT-8 | `diagram(fit=…)`: aspect ratio check and written suggestions (`dir`, `wrap`), never an automatic change (D-041) | S |
| F-LAYOUT-3 | Sizes from the measurement; labels taken into account | M |
| F-LAYOUT-4 | Partial positions: `at=` cards fixed, the others at the computed position; zones = bounding box, with **a single margin rule** for ELK and recomputed zones (limit L6) | M |
| F-LAYOUT-6 | **Column alignment** (D-036): within a zone, free cards whose horizontal extents overlap (one ELK column) are aligned on the side that receives most links from outside the column (tie: nothing moves). Computed on ELK's placement **before** pins, so pinning a card moves no other card; routes of moved cards are re-routed from scratch. Micro-jogs under 0.75 px are removed from routes | S |
| F-LAYOUT-5 | **Overlaps reported, never corrected** (D-033, Q7): a card that overlaps a pinned card (`at=`) stays where it is; the warning names it ("card « a » overlaps « b » (position pinned by at=)", in French for now). `layout::suggest_separation` computes, without applying it, what would need to be moved away (editor proposal, ED-10). `fixtures/reseau-conflit` case: parity with the prototype's oracle | S |

### Routing (orchestration around `router`)

| ID | Requirement | Prio |
|---|---|---|
| E-ROUTE-1 | **Single router (D-010)**: all arrows go through `router`, automatic or pinned positions. ELK routes are passed as `hint` | M |
| E-ROUTE-2 | Scene translation: cards → `Shape`, zones → `Region`, `pass_through` = ancestor zones of the start and the end, the model's `side` and `via` | M |
| E-ROUTE-3 | Incremental re-routing exposed to the editor (`Router::update`) | S |
| E-ROUTE-4 | Label placement avoiding zone titles and other arrows (limit L4) | S |

### Timeline mode *(post-v1; prototyped in the POC: `runtime.js`, `layoutTimeline`)*

| ID | Requirement | Prio |
|---|---|---|
| F-TL-1 | Placement **by date**: linear scale `from → to` (density according to `scale`, or `width`); milestone = left edge on the date; period = start at `start`, width ≥ duration (wider if the text requires), `--bar-w` bar = exact duration | C |
| F-TL-2 | **Lanes** = top-level zones, in writing order, over the full width of the axis; cards outside a zone = implicit lane at the top | C |
| F-TL-3 | **Stacking** in a lane: chronological sort, first free row (horizontal gap 12 px, vertical 22 px); lane height = sum of the rows | C |
| F-TL-4 | **Axis**: graduations according to `scale` (labels in the language of the messages, or `diagram(lang="fr"|"en")`: "janv. 2026" / "Jan 2026", "T1 2026" / "Q1 2026"…, major at year or month change), vertical grid lines, milestone diamond, `today` marker | C |
| F-TL-5 | **Dependencies** through `router`: lanes are **not** obstacles (`pass_through` = all lanes); default sides `right-left` inside a lane, `right-*` towards another lane; reduced margin (8 px) because lanes are tight | C |
| F-TL-6 | Dependencies between lanes go **through the gutters between lanes** rather than along a grid line or the `today` marker (visible defect in the POC: R0 → Validation arrow) | C |
| F-TL-7 | Steps: as elsewhere; the axis diamond inherits its milestone's step | C |
| F-TL-8 | `fixtures/timeline` case: layout parity (`nodes` ±1 px with injected `sizes`) | C |

### Frieze mode *(post-v1; prototyped in the POC: `runtime.js`, `layoutFrise`)*

| ID | Requirement | Prio |
|---|---|---|
| F-FR-1 | Scale in **astronomical years** (no year 0); density according to `scale` or `width` | C |
| F-FR-2 | **Eras** = bands under the axis (height 28 px); stacking **by decreasing duration**, touching bands on the same row (0.5 px tolerance), name to the right of the band if it does not fit (`.outside`), its room being reserved in the stacking | C |
| F-FR-3 | **Events** centred on their date, in chronological order, on the side (up or down) that asks for the lowest level, alternating on a tie; multiple levels per side (gaps 14 px); vertical **stem** on the exact date (minimum length 30 px) and dot on the axis | C |
| F-FR-4 | **Graduations**: step = the smallest of 1, 2, 5, 10, 20, 50, 100 × the unit giving ≥ 90 px; labels "500 av. J.-C.", "1", "500"; at century scale, centred centuries in Roman numerals ("Iᵉʳ s. av.") | C |
| F-FR-5 | Arrows (rare) through `router`, without zone obstacle, 8 px margin | C |
| F-FR-6 | Steps: stem and dot inherit their event's step | C |
| F-FR-7 | **Registers**: zones as parallel friezes on a single axis (for instance Rome and China) | C (after F-FR-1 to 6; not planned) |
| F-FR-8 | Prevent century labels from being crossed by the stems of the upper events (visible defect in the POC) | C |
| F-FR-9 | `fixtures/frise` case: layout parity (`nodes` ±1 px, injected `sizes`) | C |

### Mindmap mode *(D-034)*

| ID | Requirement | Prio |
|---|---|---|
| F-MM-1 | **Tree from the arrows**: root = `root` option, else the first untargeted card; children = targets of `to(…)` in writing order (breadth-first, a card already reached is not taken again); unreachable cards placed below the map with a warning | S |
| F-MM-2 | **Bilateral placement** (`mindmap::layout`): root in the middle, the children of the root alternately on the lower side (right first), sub-branches moving outwards; a node centred on the block of its children; levels 72 px apart, siblings 20 px; no overlap | S |
| F-MM-3 | **Routing**: tree arrows leave through the outward side and arrive through the centre-facing side; those of one parent on one side share a trunk (router R-18) | S |
| F-MM-4 | Layout independent of classes and theme; `@` animation markers work as elsewhere | S |

### Styles and themes

| ID | Requirement | Prio |
|---|---|---|
| E-STYLE-1 | **Header style**: if a card's resolved style is `look = header`, render `.head > span.type + span.tag`, then the centred title and the `li`; measurement takes this structure into account | M |
| E-STYLE-2 | Secondary cards `.note`, `.chip`, `.info` and zone `.bare` (legend under the group, no frame) | M |
| E-STYLE-3 | Arrows: add `from-<type>` and `to-<type>` for each class of the endpoints (HTML and SVG) | M |
| E-STYLE-4 | **Theme** passed to the rendering (`light`, `dark`, `print`, `auto`): `theme-<name>` class on `.stage`; **measurement and layout are done for this theme**, since dimensions may vary (print theme borders) | M |
| E-STYLE-5 | SVG and PNG: concrete colours from the resolution (`color-mix` computed by `lang`); `auto` → `light` with a warning | M |
| E-STYLE-6 | Exported HTML: theme CSS + `@media print` (legible black-and-white printing, whatever the theme); page background = the theme's `--bg` | M |
| E-STYLE-7 | Contrast test (`lang`'s F-STYLE-15) on the renderings of all test sets, in the 3 themes; and **black-and-white test**: greyscale conversion of the `print` PNG, then check that badges and bands stay legible (text/background luminance gap ≥ threshold). **Enhanced contrast option** (D-026): `Options::high_contrast` | M |
| E-STYLE-8 | `side=` always triggers the router (ELK would ignore it); with the single router (D-010), it is automatic | M |

### Rendering and export

| ID | Requirement | Prio |
|---|---|---|
| F-RENDER-1 | **Standalone** HTML: CSS (theme + custom), fonts as `@font-face` (data URI), positioned DOM with **imposed sizes**, step player; no embedded engine | M |
| F-RENDER-2 | Standalone SVG: translated decorative subset (backgrounds, borders, radii, colours, simple shadow, badges, arrows and their styles); what is not translated is documented | M |
| F-RENDER-3 | PNG without a browser (SVG → resvg), scale 2 by default; **transparent background** option (D-029) | M |
| F-RENDER-4 | `layout.json` output in the oracle's format (`sizes`, `nodes`, `edges`) | M |
| F-RENDER-9 | **Source embedded** in every output (D-042): PNG/APNG `iTXt` chunk `skemo`, GIF comment, WebP `SKMO` chunk, SVG `<metadata>`, HTML `<script type="application/json">`; bundle = source, stylesheets, options; `source::extract` gives it back; viewers unaffected | S |
| F-RENDER-5 | Prototype framing (28 px margin); the file's coordinates are never rewritten | M |
| F-RENDER-6 | DOM and SVG structure of `FORMAT.md` §4.2 (`lang` repository) (stable selectors: `g.edge`, `path.line`, `.edge-label`…) | M |
| F-RENDER-7 | PDF export | C (not planned) |
| F-RENDER-8 | **WebP** output, static and animated (D-034) | S |

### Animation

Principle: **the engine resolves and marks**; it only animates when it produces images itself (PNG per state, GIF).

| ID | Requirement | Prio |
|---|---|---|
| F-ANIM-1 | **Resolution** of `@` markers (`FORMAT.md` §3.8): inheritance `appearance = max(parent, own)`, `end = min(parent, own)`; links = bounds of their endpoints and `draw` effect by default; marker too early (or end too late) → warning, inherited value. The result (per element: appearance, end, effect, focus ranges) appears in `layout.json` (`anim` key) | M |
| F-ANIM-2 | **List of states** = 0 (only if it contains at least one visible element), then the written values, sorted and de-duplicated (starts, ends, focus). State `s`: visible if `a ≤ s ≤ b`, focused if `f0 ≤ s ≤ f1`; disappearance at the first step defined after `b` (warning if there is none) | M |
| F-ANIM-3 | **Marked SVG** (`FORMAT.md` §4.4): `<g>` per node with nested children; links group above; `data-id` (never `id`), `data-step` (original text), `data-fx`, `data-until`, `data-focus`, `data-from`/`data-to`, `pathLength="1"` on the stroke. **No** attribute on a non-animated element; **no** class, style or script related to animation | M |
| F-ANIM-4 | **HTML**: same attributes; built-in player (→, space, PageDown, click = next; ←, PageUp = previous; `?all` = everything; `?step=N`) that plays effects and focus in CSS | M |
| F-ANIM-5 | **PNG per state** (`--animated`): one file per state of F-ANIM-2, rendered **without transition effect** (final state of each step) but **with focus** (it is a state, not an effect); names `name.00.png`, `name.01.png`… + `name.states.json` (index → step value, visible elements, focused elements). A **still** PNG or WebP of an animated diagram is its **last state** (D-069) | M |
| F-ANIM-6 | **Animated GIF**: chains the states **with their effects**. Effects: `fade` (opacity 0 → 1), `up` (+ 12 px upward translation), `zoom` (+ scale 0.92 → 1), `draw` (stroke from 0 to 100 % through `pathLength`, head at the end); disappearance = reverse effect; focus = dimming transition. Parameters: hold time per state (default 1.6 s), transition time (0.45 s; 0.6 s for `draw`), frames per second (default 20), loop (default yes), last frame held longer (default 3 s) | M |
| F-ANIM-7 | **Focus rendering** (HTML, PNG, GIF), defined by the theme: as long as at least one element is focused, the others are **dimmed** (`--dim-opacity`, default 0.28) and the focused element gets an accent outline (`--focus-ring`). Legible in print theme (dimming through opacity, not colour) | M |
| F-ANIM-8 | **Determinism** of images: same input → same bytes (PNG and GIF), hence a deterministically computed GIF palette. Quantisation: a permissively licensed crate (**not** `imagequant`, which is GPL-3) | M |
| F-ANIM-9 | WebAssembly API: `states()`, `goto(step)`, `stepchange` event | S |
| F-ANIM-10 | **APNG** variant (done: exact colours, lighter than GIF) and animated **WebP** (planned, D-034) | S |

### WebAssembly

| ID | Requirement | Prio |
|---|---|---|
| F-WASM-1 | `render(source, css?, options?) → { html, svg, layout, diagnostics }` | M |
| F-WASM-2 | **Identical** result to native on all references (same `layout.json`) | M |
| F-WASM-3 | `<skemo-view src=… css=… step=…>` custom element | S |
| F-WASM-4 | Editing API for the editor: `move_card`, `set_via`, `set_side` → new source (surgical editing from `lang`) + new layout | S |
| F-WASM-5 | Size: < 4 MB gzipped (to measure in E0; document if exceeded) | S |

## 4. Building blocks to evaluate (milestone E0)

| Need | Candidate | Note |
|---|---|---|
| Layout | **elk-rs** (github.com/openedges/elk-rs, EPL-2.0) | Complete port of ELK, aims at parity (≈ 2,000 models), WASM. Very young: **it is risk no. 1** |
| Box model | **taffy** | Mature *(not needed: D-021)* |
| Text | **parley** or **cosmic-text** | Mature *(replaced by harfrust: D-021, D-032)* |
| SVG → PNG | **resvg / tiny-skia** | Mature |
| Rendering alternative | **Blitz** (through `hyper-render`, Stylo CSS engine) | Young; evaluate fidelity against size |
| WASM | **wasm-bindgen**, `wasm-pack` or `trunk` | — |

## 5. Test sets (`fixtures/`)

Complete prototype oracle: for each case, `*.layout.json` (`{model, layout: {engine, router, sizes, nodes, edges}}`), `*.png`, `*.html`; the `*.pug` sources and `process.css`; `manuel-via.steps.png` (contact sheet of the steps). *(The oracle keeps its French labels.)*

Parity tests, **isolating each stage**:
1. **Layout**: inject the oracle's `layout.sizes` (no measurement), then compare `nodes` to ±1 px (`auto-elk`, `process`, `deux-pins` cases).
2. **Measurement**: compare the measured sizes with `layout.sizes` (±2 px).
3. **Routing**: delegated to the `router` repository (its own sets); here, check E-ROUTE-2 (correct scene translation) and do a visual review.
4. **Rendering**: perceptual diff of the PNG against `*.png`, plus human review on a contact sheet; `steps` against `manuel-via.steps.png`. Plus `tests/golden.rs`: fingerprints of the outputs of every example (SVG in 3 themes, contrast, HTML, PNG).

The `auto-graphviz`, `auto-dagre`, `manuel-neato` and `manuel-straight` cases only serve as **illustration** (engines not retained).

Animation: `animation.pug` + **`animation.expected.json`** (texts after marker removal, warnings, list of states, expected SVG attributes per element, visible and focused elements per state). It is the reference test of F-ANIM-1 to 5.

Themes: `design`, `design-sombre`, `design-print`, `entete`, `entete-sombre`, `timeline-sombre`, `frise-print`. For a given source, the light and dark cases have the **same layout**; the print case may differ by a few pixels (borders).

## 6. Milestones

| Milestone | Contents | Criterion |
|---|---|---|
| **E0** — spikes | elk-rs on `auto-elk` and `process` (same `sizes` and options); one measured card (taffy + parley); the same card as SVG→PNG (resvg) and through Blitz; WASM size | Numbered report + contact sheets; **choices validated by the user** (D-005, D-007) |
| **E1** — measurement + layout | E-MEAS-*, F-LAYOUT-1 to 4, F-RENDER-4 | layout and measurement parity |
| **E2** — routing | E-ROUTE-1 and 2 (requires `router` R1) | visual review of the `manuel-*`, `deux-pins` cases |
| **E3** — rendering | F-RENDER-1 to 3, 5, 6; E-STYLE-1 to 8 | contact sheet validated by the user (light, dark, print, and print converted to grey), without Chromium |
| **E4** — animation | F-ANIM-1 to 8 | marked SVG validated by attribute tests (animation test sets); PNG per state identical to the prototype's contact sheet; GIF reviewed by the user |
| **E5** — WebAssembly | F-WASM-1 to 3, 5 | native `layout.json` = WASM; static demo page |
| **E6** — towards the editor | F-LAYOUT-5, E-ROUTE-3 and 4, F-WASM-4, F-ANIM-4 | — |
| **E7** — timeline *(post-v1)* | F-TL-1 to 8 (requires lang L4) | `fixtures/timeline` parity; step contact sheet identical to `timeline.steps.png` |
| **E8** — frieze *(post-v1)* | F-FR-1 to 6, 8, 9 (then F-FR-7) | `fixtures/frise` parity; step contact sheet identical to `frise.steps.png` |

## 7. Licence

Constrained by **elk-rs (EPL-2.0)**: distributing a binary is possible, but the source of the EPL files must remain available. Our code is under MIT OR Apache-2.0, with the EPL-2.0 dependency declared: its notice accompanies distributions and its source is available upstream.
