# SVG conventions — version 1

What an integration may rely on in the SVG skemo writes (`skemo render … --format svg`, and
`skemo_engine::svg::render`). glitu's engine and exports read these; so may any other player.

**Versioning.** This is version **1**, given by `skemo capabilities --format json`
(`"svg_conventions": 1`). Within a version, things are only added. A removal, a rename or a change
of meaning raises the version, announced in the changelog first. Anything not listed here (the
geometry, the class names of the parts beyond those below, the order of the elements, the
`<defs>`' contents) may change at any release.

## 1. Attributes

| Where | What | Meaning |
|---|---|---|
| root `<svg>` | `class="skemo theme-…"` | a skemo diagram; the theme (`theme-light`, `theme-dark`, `theme-print`) |
| root `<svg>` | `data-states` | JSON list of the steps, as written and in order (`["1","2.5","3"]`); absent on a still diagram |
| root `<svg>` | `viewBox`, `width`, `height` | size in CSS pixels; `width` and `height` equal the `viewBox`'s |
| node, line, arrow `<g>` | `data-id` | the element's identifier: the `#id`, a generated one otherwise (`card1`), `a->b` for an arrow. **Never an `id` attribute** |
| same | `data-step` | appearance step, as written (decimals kept); absent when the element appears with its group (FORMAT §4.4) |
| same | `data-fx` | appearance effect, with `data-step` only: `fade`, `up`, `zoom`, `draw` (arrows) |
| same | `data-until` | last step the element is visible |
| same | `data-focus` | focus ranges, space-separated: `"4 6-7"` |
| same | `data-highlight` | highlights in writing order, space-separated `effect:range`: `"halo:3-4 magnify:6"`; effects `halo`, `chase`, `magnify` (FORMAT §3.8, D-090) |
| arrow `<g>` | class `edge`; `data-from`, `data-to` | an arrow and the identifiers of its ends (`data-from`/`data-to` when it carries a step attribute) |
| node `<g>` | class `card` or `zone`, then the type's classes | the kind of element and its types (`card web`) |
| line `<g>` | class `li` | a line `|` of a card or a zone |
| arrow label | class `edge-label` with `data-for="<arrow id>"`, in a group `edge-labels` after the arrows | an arrow's label, drawn above every line (as the HTML draws it); it carries its arrow's step attributes |
| frieze, timeline | classes `fr-stem`, `fr-dot`, `tl-mark` with `data-for="<id>"` | decorations attached to an element outside its group; they carry its step attributes (`data-col` and `data-tag` are written in the HTML output only) |
| `<metadata>` | `<skemo-source xmlns="https://skemo.net/source">` | the source bundle, JSON, escaped (absent with `--no-source`); read back by `skemo extract` |
| `<defs>` | identifiers prefixed by a digest of the diagram (`skfaa3a531-ombre0`) | several diagrams in one page without collisions; referenced only as `url(#…)` or `href="#…"` inside the same SVG |

Elements nest: a zone's group holds its cards, a card's group its lines. The arrows are in a group of
their own, above the nodes, and their labels in another, above the arrows. skemo adds no script and no animation class; it animates nothing in SVG
unless asked with `--motion` (one `<style>` of keyframes).

## 2. What the SVG never holds

- no `<script>`, no `<foreignObject>`, no `on*` attribute, no `javascript:`;
- no external reference: images are embedded as `data:image/svg+xml|png|jpeg|webp` in an `<image>`
  (an SVG image is never inlined); `href` points to `#id` inside the same SVG, except below;
- **one exception, written by the author**: a `href="…"` written in the source (FORMAT §3.2.1) gives an
  `<a href="…">` around the element, to the address written (`https:`, `mailto:`, a relative path;
  never `javascript:`). An integration that does not want links drops the `<a>` and keeps its content.

## 3. Elements

`svg g path rect circle ellipse line polyline polygon text tspan defs marker linearGradient
radialGradient stop clipPath mask pattern symbol use filter fe* title desc metadata image` and
`skemo-source`; also `a` (only for a `href` written in the source) and `style` (only with `--motion`,
or `--embed-fonts`: `@font-face` rules with `data:` fonts). A new element is announced here first,
in a new version if an integration's allowlist would have to change.

Test: `cli/tests/contract.rs` renders every example of the repository and checks it against this list.

## 4. Determinism

The same source, options, files and version give the same bytes, on every platform: no date, no
random number, no order taken from a hash map; the fonts used for measuring are embedded in the
binary. Tested by rendering the examples twice in two processes (`cli/tests/contract.rs`) and in CI.
