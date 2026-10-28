# Design decisions

Comments and specs refer to design decisions as `D-NNN`. The full log is kept by the maintainers; this page
summarises the decisions the published code refers to, so that a reference can be followed. New decisions are
taken in issues and pull requests.

| Id | Decision |
|---|---|
| D-005 | Layout by ELK's layered algorithm (elk-rs in Rust): orthogonal routes and nested zones handled better than Graphviz dot or dagre. |
| D-007 | Rendering without a browser: geometry computed from a subset of CSS; HTML with imposed sizes; PNG through resvg. |
| D-009 | The name: skemo, for the language and the software; file extension `.skemo`. |
| D-010 | A single router for every arrow (`skemo-router`); no second routing path. |
| D-018 | The router is rewritten from the papers rather than adapted from an existing library. |
| D-021 | Text measurement without a layout library: blocks of lines matched on Chromium's metrics, on embedded fonts. |
| D-022 | An in-house CSS engine in `lang` for the subset skemo uses (selectors, cascade, `@media`, `color-mix`, variables). |
| D-023 | Fonts embedded whole (Inter, JetBrains Mono); subsetting left for later. |
| D-026 | The default theme keeps its palette; an "enhanced contrast" rendering option fixes texts under the WCAG threshold. |
| D-029 | PNG shadows are approximated without a blur filter; a transparent-background option. |
| D-031 | Stability by construction: positions do not depend on editing history; pinning a card where it is changes nothing; only the arrows of a moved card move. |
| D-032 | One quality and security pipeline, identical locally and in CI, run on every tag. |
| D-033 | Overlaps with pinned cards are reported, never corrected by moving other cards. |
| D-034 | Optional features: WebP and animated WebP, mind maps, hyperedges (shared trunks). |
| D-035 | Route segments are centred in their free corridor. |
| D-036 | Default card look with the name in a coloured band; column alignment in zones; `render` as the default command. |
| D-038 | Messages follow the user's system language (English and French). |
| D-039 | The language describes concepts (what a thing is); the theme decides how a concept is drawn (`look`). |
| D-041 | Fixes from diagrams made for slides: writing order kept in rows, cycle breaking that keeps the written direction, router details. |
| D-042 | Rendered files embed their source and stylesheets, so that an image can be opened again (`--no-source` to leave it out). |
| D-043 | `fit` ranks settings by the text size once fitted; a lone zone is centred on the cards it links to. |
| D-044 | Readable arrows: zone titles are obstacles, straight lines between facing cards, warnings when `wrap` hides cards. |
| D-045 | Stacked cards in zones fixed; `fit` warns about proportions; arrows outside the layout handled. |
| D-046 | Image files as icons (local files only, handed to the engine as bytes) instead of embedded brand logos. |
| D-047 | Layered drawings tidied after comparison with a hand drawing: trunk direction, row spacing in zones, forks and buses. |
| D-048 | Rules drawn from 34 cases redrawn by hand: leaves lifted to their siblings' row, label room, and other gaps fixed. |
| D-049 | Second pass on the hand-drawn rules: arrow heads, label placement, style and routing details. |
| D-050 | Diagram title, zone tags, stacks (`cards`), wide cards; named joins share an arrival trunk. |
| D-051 | Line pills, named joins, badge sides, wide zones and columns. |
| D-054 | Columns that hold their place, join labels on the shared trunk, headless lines. |
| D-056 | Sources first, stacked zones at one width, corridors between them, forked cross links, aligned titles. |
| D-057 | Lines may pass closer to a zone's tag pill than to a card. |
| D-058 | Wide bars among wide zones; sides kept only for forks and named joins. |
| D-059 | Columns at an even rhythm, for symmetry, without new syntax. |
| D-060 | In a diagram drawn in steps, an arrow is routed among what is shown at the same time as it. |
| D-062 | `cards`: a card drawn as a pile, with a `×N` badge. |
| D-063 | Lines keep a fixed gap from each other and from zone frames. |
| D-064 | A zone title does not cover a card linked from above. |
| D-065 | A better writing order for generated sources, for programs that write skemo text (`reorder`). |
| D-069 | A still image of an animated diagram shows its last step. |
| D-070 | The order search of D-065 starts from a barycentre sweep and kicks out of local optima. |
