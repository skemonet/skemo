# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/) (pre-1.0: minor versions may break the format).

## [Unreleased]

## [0.1.0-rc.11] — 2026-11-19

Layout by borders and alignment in the slide engine (D-117): `block`, `dir` and `align` on a
container, `at="n|s|e|w"` inside a group, docked elements that keep their arrows. Also `tunnel`
(D-116), `nodes`, a zone's title beside its cards, layout settings given by a type in CSS, and the
slide engine about twice as fast.

### Added

- `block` (D-117): a group drawn as nothing, no frame nor title, that the other lines go round; it lays out what it holds. `block(dir="right" align="center")`.
- `dir="right|down|left|up"` (or `horizontal`, `vertical`) on a zone, a stack or a block: its cards and children one after the other that way, in writing order; `align="start|center|end"` across it.
- `at="n|s|e|w"` on a child of a zone or a block docks it on that side of its group, as Java's `BorderLayout`. A docked element linked to the rest stands on its side of the drawing and its arrows are drawn (they were left out with `docked-arrow`). Slide layout.
- `nodes`: a node standing for several, one icon with its `×N` badge, no pile: `nodes#c(icon="cable" count="3")`.
- `head="left"` on a zone: its title and description in a column left of its cards, rather than above them. Slide layout.
- Compass points for sides: `side="n-s"` (leaves by the top, reaches by the bottom) besides `top-bottom`; `at="top"`, `at="bottom-right"`… besides `n`, `se`….
- Layout settings given by a type in CSS: `--head: left`, `--dir`, `--align` on a zone's type, `--side: n-s` on an arrow's class; an attribute written on the element wins.
- `tunnel`: a tube between two cards that arrows pass through (D-116). `tunnel#vpn(gw-b) IPsec` written in one card joins it to `gw-b`; `to(db tunnel="vpn")` runs into the near end, along the tunnel on a lane of its own, and out of the far end: one line, hidden inside the two cards, the tube widening with its lanes. Slide and graph layouts.

### Changed

- An empty zone is drawn as a card (a `zones` pile as a `cards` pile) until something is written inside it.
- In the slide layout, the cards of a row share the tallest one's height, no longer the drawing's tallest card's; each level of frames opening on a row takes the header its own titles need, no longer the drawing's longest description.
- The slide engine is about twice as fast: tilings and attempts drawn four at once (the same drawing).
- The spaces after the `|` of a line, past the first, are kept and drawn, as in Pug: `|     *via* gw-a` is indented by four spaces (they were trimmed).

### Release process

- A release's Linux x86_64 binary is published as soon as it is built, a few minutes after the tag; the four others and `SHA256SUMS` follow.
- The `history/*` branches, which keep the detailed history of a squashed release, no longer run the checks.

### Fixed

- A `.bare` zone with a title tripled the header of every framed zone of the drawing (its caption below its cards was counted as a header).
- A `//` comment in column 0 inside the diagram (an editor's comment toggle) hid everything after it, silently: it closed the diagram as a block comment. It now hides its own line only; a comment at its own level still hides its indented block, as in Pug.
- A tab after a tag starts its text as a space does: `to(b)<TAB>HTTPS` and `card#b<TAB>B` were refused with "unexpected character after the tag".

## [0.1.0-rc.10] — 2026-11-19

The slide engine reads what ELK read: zones inside zones, `stack`, `.wide`, `.bare`, pinned cards,
`via=`, `side=`, empty zones, notes, `join=`, a written `dir`, `wrap`, `zones="fit"`, zones placed
by hand, arrows to zones and columns. Only an invisible card or more than 120 arrows still send a
diagram to ELK. Also: `zones`, a zone drawn as a pile of frames; arrows from a zone; skemo's own
infrastructure icons (`skemo-*`); Lucide 1.73.

- **`zones`** (D-114): a zone drawn as a pile of frames, for several instances of one group (the
  clusters of a fleet); what it holds is drawn once, in the frame on top; `count="N"` adds the
  `×N` badge, as on `cards`. The slide engine keeps the frames behind clear of the rest.
- **Arrows from a zone**: `to(…)` written in a zone leaves its frame, to a card or a zone (one
  link between two sites rather than one per card); drawn by the slide engine.
- **Icons**: 36 icons of skemo's own for infrastructure, `skemo-vm`, `skemo-hypervisor`,
  `skemo-rack`, `skemo-load-balancer`, `skemo-pod`… (D-115), drawn like Lucide's; Lucide 1.73.0
  (2,055 icons, 1,866 before: `graph-ring`…); the names Lucide changed (`triangle-alert` is
  `triangle-exclamation-point`) still work.
- **Slide engine, stack and wide layers**: a stack's layer and a wide zone's row share their width
  evenly, each card in the middle of its share (D-092); a card alone in its layer stands over the
  card it links to when it can, else in the middle.
- **Slide engine, arrows to zones and columns**: an arrow may leave or enter a zone (or a stack, a
  bare zone): its line ends on the frame, from outside; the members of a column (`.col-<name>`, a
  card or a zone) share an axis across `dir`, the tiling and the cells chosen for it.
- **Slide engine, `wrap`, `zones="fit"` and zones placed by hand**: a `wrap="N"` zone lays its
  children in writing order, N a row (a column with `dir` right or left); `zones="fit"` fits each
  frame to its cards; a zone with `at=` (and `size=`) stands where written with what it holds, at
  the size written, its arrows routed again. Two blocks of one zone keep the zone's own gap.
- **Slide engine, notes, `join` and `dir`**: a note leans to the place beside its card (P16, a
  preference: it gives way to the lines' rules) and stays out of the writing order; arrows joined
  by `join=` share their trunk (out of one card for `trunk`, into one for a name); a written `dir`
  lays the flow that way, an arrow set aside across it or back against it paying (without `dir`,
  down or right as before). A frame takes the padding of its level, so that frames of one level
  line up.
- **Slide engine, written ways and empty zones**: `via=` (the route runs through the waypoints in
  order, a waypoint off the lattice taken to its nearest free node) and `side=` (the sides an arrow
  leaves and enters by) are kept, in the editing session too; an empty zone keeps a frame.
- **Slide engine, zones inside zones**: a zone holds cards and zones at any depth; each level is
  tiled like the slide, a gutter holds a padding for each frame it crosses and a title for each
  frame opening below it. Such diagrams no longer go to ELK.
- **Slide engine, `stack`, `.wide` and `.bare`**: a stack lays its children one after the other in
  writing order along `dir`, a card alone in its layer in the middle; a `.wide` card is a bar across
  its group that arrows reach straight; a `.wide` zone is a row of its own, its cards on one row; a
  `.bare` zone groups without a frame. Columns (`.col-<name>`) still go to ELK.
- **Slide engine, pinned cards** (D-031): the slide is drawn without its pins; a pin where the
  slide put the card changes nothing; a card pinned elsewhere stands there, the zones holding it
  grow round it, the arrows are routed again, and a zone grown over a card not its own is
  reported (`overlap`). The editing session's drag preview is the drawing the file gives once
  `at=` is written.

## [0.1.0-rc.9] — 2026-11-18

The slide engine becomes the default. `node` draws a thing by its icon; in the slide engine its
arrows end on the icon, or start after its text level with it.

- **Added**: `node` (D-087), a thing drawn by its icon rather than by a box: the icon on the left, its name and
  lines beside it, no frame. Nodes mix with cards whatever the look; every node's icon has the same size (32 px),
  centred on its text, and arrows end on the icon. A node takes what a card takes, except `tag` and `label`.
- **Changed**: a built-in icon of 30 px or more is drawn with a thinner line (2.5 px).
- **Changed**: a short address for the installer on Linux and macOS, `curl -fsSL https://skemo.net/install.sh | sh`:
  the site publishes `cli/install.sh` there (the GitHub address keeps working).
- **The slide engine is the default**: a diagram that names no layout is drawn by the slide engine
  when it reads all of it; one it does not read yet (nested zones, a pinned card, an empty zone, a
  zone placed or sized by hand, `wrap`, `stack`, `.wide`, an arrow to a zone) keeps ELK, as does
  `layout="graph"`. `SKEMO_LAYOUT=graph` or `grid` gives another engine to the diagrams that name
  none. In the slide engine, sibling groups and the cards of a group keep their writing order (a
  card outside every zone where it is written), `constraint="false"` arrows play no part in the
  placement, and a pile takes the room of its cards behind. On the twenty diagrams of the bench:
  16 hard findings (19 before), 13 drawings without any.
- **A character the font does not draw** is reported (`missing-glyph`, by the render and by
  `skemo lint`) instead of left blank in silence.
- A zone's description lines keep as much room from the frame's right edge as from its left.
- **Slide engine (experimental)**: `diagram(layout="slide")` places, routes and labels a diagram as
  one model solved at once (`engine/src/slide`): the groups tiled like a slide layout on a lattice
  of 10 px, candidate routes for every arrow, the rules between two lines and between labels and
  lines as conflicts, and a deterministic search of its own (no solver library, the same drawing
  in WebAssembly); room is made where a line pays for the lack of it. On twenty diagrams of a
  corpus, judged by the maintainers' independent checker: 19 hard findings where rc.8 has 521, 6
  crossings where it has 114, 241 bends where it has 410, lines 41 % shorter; 2.6 s median on one
  core (0.2 to 5 s for the seventeen within a slide's limits, 13 to 16 s for the three past them).
  Labels wider than 140 px wrap on up to three lines (SVG, HTML, PNG). `SKEMO_LAYOUT=slide` gives it
  to the diagrams that name no layout; `skemo layout` reports `layout.engine` = `slide`.
- **Grid engine (experimental)**: `diagram(layout="grid")` lays a diagram out as a grid of zones and
  cards instead of a layered graph: blocks in rows, an order that shortens the links, gutters that
  grow for their lines and labels, and its own routing rules. One module (`engine/src/grid`), one
  pipeline, each rule named and switched off by `SKEMO_OFF` for measuring. `SKEMO_LAYOUT=grid` gives
  it to the diagrams that name no layout, to measure a whole corpus; the default engine is unchanged.
  `skemo layout` says which engine placed the drawing (`layout.engine`: `grid` or `elk`).
- **Grid engine, room made evenly**: what the lines lack (two lines too close, a tail cut short
  behind a head) and what the labels lack widens the gutters of the whole group that owns the place,
  the same between all its children, never one gap among the others; a corridor is read place by
  place, between the two boxes that bound it there. A line that runs through another arrow's label
  moves to a lane beside it (`grid-label-lanes`). A zone widened for its title keeps its content
  centred, and a zone's content slides as one when two linked cards line up.
- **Grid engine, labels**: two labels that lie over one another on lines side by side (two arrows
  between the same cards, one each way) part: the straight lines move apart along the sides of
  their cards until each label stands over its own line. What a gap still lacks once widened is
  asked with 16 px more, so the room is made within the rounds of the layout (labels were left
  hidden by a line or by another label when the lines moved into the room just made). A label a
  line still runs through, where every place looked wide enough, asks a lane more of its gap (the
  line could not stand aside after all), and the round of the room that hides the fewest labels is
  kept, not merely the last.
- **Lines** (both engines): the branches of a fork leave its trunk as late as their way allows
  (`Params::late_split` of the router): a branch may start anywhere along the routes of its group
  already drawn, not only at the source, so two arrows to neighbouring cards go round an obstacle
  together and part just above their targets (they parted at the source, one each way round). A
  join, routed from its target, meets the same way. A segment in line with a route's first or last
  stretch moves with it or stays (moved alone, it left the stub at the port on a slant).
- **Graph engine, room between the zones**: the zones' frames bound the corridor a line is centred
  in, as the grid engine's do (`Params::region_walls`): a line between two zones stays between them
  (it was centred into a zone holding neither of its ends). The corridors between columns widen for
  their lines and for the labels on them, as those between rows already did (D-102); a label needs
  its width there, clear of the frames. A note stays beside its card when a corridor widens.
- **Grid engine, rows across zones**: zones of cards side by side in one row of the mosaic share the
  pitch of their rows (the largest of theirs): their rows of cards line up from one frame to the
  next.
- **Quality properties**: `through-zone` counts an arrow running through a zone that holds neither of
  its ends (rank 2, the lines).
- **Lines** (both engines): lines spread apart beside a card keep the router's whole margin from it
  when the corridor holds them so (they kept half of it, and a line ran 7 px along a column of
  cards); a cluster of lines in a corridor open on one side moves away from the side that bounds it.
- **Lines** (both engines): a zone's description lines are obstacles of their own, with the full
  margin of a title (they were taken for its tag pill, and an arrow could run through them). A step
  of less than a pixel at a corner is no longer dropped as a jog (the arrow's first segment was then
  drawn on a slant), and the router no longer swaps the levels of two turns when one "turn" is only
  a stub in line with its end (the stub was left on a slant).
- **Fixed**: a label exactly as wide as the free stretch of its line could crash the layout (bounds
  of a clamp rounded apart).
- **Experiments**: `SKEMO_JITTER=px` adds pixels to every gutter of the grid engine, to measure how
  much of a change of the counts is noise.
- **Quality properties**: the manual method's line rules are counted: `staircase` (a route stepping
  along the diagonal), `short-tail` (a head right behind a bend), `port-jog` (a Z between facing
  sides one port move would straighten), `close-lines` (two parallel lines closer than 6 px), and
  `uneven-gaps` (a group whose gutters differ along one axis).
- **Grid engine, stability** (D-031): pinning a card where it stands changes nothing, a card moved by
  hand keeps the size the grid gave it, and its zones grow to hold it.
- **Lines**: a step flattened next to an end no longer leaves the arrow's last segment on a slant
  (the step stays when one of its segments is the arrow's end).
- **Editing session**: a legend, a note or a link docked around the drawing stays where it is while
  a card is dragged (it jumped to the origin until the file was written).
- **Quality properties**: `sparse`, a drawing whose cards cover less than an eighth of its area.
- **Experiments**: `SKEMO_ELK='{"elk.…": "…"}'` adds ELK options to every diagram (the layout method's trials).
- **Docking**: a `.wide` card docked above or below the drawing spans the bands' whole width, its text
  wrapped to it (a note kept its own 280 px under a wider drawing).
- **`skemo site`**: the pages are light and dark by default (`--theme auto`), following the reader's
  system with a switch (`--theme light` draws one theme); the side panel's edge is dragged, or moved with
  the arrow keys, to set its width, kept from page to page; a double-click goes back to the default.
- **Theme**: a text card (`card.text`) wraps at a reading measure (440 px, about 70 characters) instead
  of running its paragraph on one line across the image; `.wide` still spans its group.
- **Quality properties**: `long-line`, a card wider than 600 px that is neither `.wide` nor a table (a
  paragraph on one line); `sprawl`, cards outside the flow (top level, no arrow, not docked) leaving the
  image 1.3 times the area of the rest or more.
- **Layout**: two cards of one column whose rows overlapped are no longer aligned on one edge, one
  over the other (they touched and overlapped by a pixel).
- **Quality properties**: `zone-order`, a zone standing before a sibling written before it that it is
  reached from (D-041, D-086); `card-overlap` counts one pixel, cards standing on whole pixels.
- **`skemo site`**: a search box at the top of every page (`/`) finds a card in every diagram by its
  title, category, tag, zone or lines (accents and case ignored) and opens its diagram centred on it; a
  note is found by the card it annotates too. The index is `assets/search.js`, loaded on first use.
- **Legends**: the sample of a note, a chip or an information card is that card in small (a note's
  amber tint and dashes), no longer a grey chip.
- **Stability** (D-031, D-085): which stacked zones stretch is read on the layout without pins; a
  card moved by hand no longer lets a zone beside its own stretch, or stops it.
- **Notes** (D-104): a note set beside its card follows it when a later layout pass moves the card, to
  the card's nearest free side; with `dir` down or up and no free side, the row opens after its card
  to make room for it. It no longer stands beyond the row with a link across the drawing.
- **Quality properties**: `note-far`, a note farther than 48 px from the card it annotates (twice
  the 24 px FORMAT gives it), its link run across the drawing.
- **Labels**: a label wider than the gap between its short upright step and a parallel line beside it
  slides along its own line, clear of the other, instead of lying over it.
- **Room for the lines** (D-102, layout): a corridor between two rows (two columns in a drawing that
  flows right) that more arrows run along than it holds grows, and the drawing is routed again: lines
  keep 20 px apart and 14 px from the cards instead of being squeezed along them and under their
  labels. Two unrelated arrows are no longer drawn on one line to save a crossing. Pinning a card where
  it stands still changes nothing.
- **Arrow labels above every line in SVG and PNG** (SVG conventions 1, an addition): the labels follow
  the arrows in a group `edge-labels`, each a `<g class="edge-label" data-for="<arrow id>">` carrying
  its arrow's step attributes, as the frieze's decorations do; a line written later no longer runs over
  an earlier label. The HTML already drew them this way.
- **Layout**: a zone title keeps as much room on its right as on its left (its last letter touched the
  frame); two zones widening toward each other for their titles no longer overlap.
- **Quality properties** (library, `skemo_engine::quality`): twenty-two checks of a laid-out diagram
  (arrows leaving or reaching the wrong place, overlapping zones and cards, a zone's title over its
  frame, text past its card, characters the font lacks, lines through a card or a zone title, labels on
  a card, on each other or crossed by a line, slanted lines, unrelated lines drawn on one line,
  crossings that swapping ports or siblings would remove, detours, lines along a box, rows not
  aligned, writing order, markup in labels, titles wrapped needlessly) and two of stability (what moves when a card is pinned
  where it stands, or moved by hand), counted over a corpus in a ledger that no change may make worse
  (`engine/tests/ledger.rs`). `SKEMO_TRACE=id,…` prints an arrow's route after every pass and a card's
  box after every layout pass; `SKEMO_OFF=pass,…` skips named layout or routing passes (ablation).
- **Zone titles and triangles** (D-099, layout): a zone title that stands in the way of an arrow
  entering its first card wraps between its words before the zone widens; a zone widened for its title
  widens as much on the other side, its cards in its middle; sibling zones of a row keep their titles
  level. The title no longer widens the zone's first card. The layout keeps the router's margin (14 px)
  from a zone title, so a line under a long title no longer steps by a few pixels. A triangle `a → b → c`
  with `a → c` in one column draws `a → c` straight and two mirrored steps through `b` (the README's
  example).
- **`--allow <code>`** (D-098, every command): the warnings of that code are neither shown nor counted
  by `--deny-warnings` (`--allow step-gap`, `--allow a,b`); errors are never left out. For tools whose
  context makes a warning meaningless: glitu's slides share their clicks between the text and the
  diagram, so a diagram's steps may skip some (`step-gap`). Feature `allow` in `skemo capabilities`.
- **`skemo site`** (D-097, preview): a folder of diagrams becomes a website of linked views, to split a
  system too large for one drawing (context, containers, components). A card, zone or arrow linked to
  another `.skemo` (`href="shop/api.skemo"`, `href="../ctx.skemo#web"`) opens that diagram's page on a
  click, zooming into it; a breadcrumb from `index.skemo`, the list of every diagram with a filter, what a
  view opens and is opened from, pan and zoom, `--theme auto` with a switch, an overview of thumbnails
  when there is no `index.skemo`. Warnings `site-missing-diagram` and `site-missing-file`. Example in
  `cli/examples/site/`.
- A link to another diagram (`href` to a `.skemo` file) draws a magnifier instead of the external-link
  glyph, of the same size (D-097). In the library: `skemo_lang::model::diagram_link`,
  `skemo_engine::svg::render_final`, `skemo_engine::html::font_faces_with`, `skemo_engine::fonts::files`.

- **Version 0.1.0-rc.9** in the working tree: `skemo --version` and `skemo capabilities` no longer give
  the number of the published release, whose command line had none of what follows.
- **`step-gap`** (D-094): a warning when a click between 1 and the last step changes nothing
  (`@1`, `@2`, then `@9:halo`), on the marker after the gap; `--deny-warnings` refuses the diagram.
  Decimal steps (`@2.5`) are no gap.
- **`empty-diagram`** (D-094): a warning for an empty or blank source, or a bare `diagram`; the empty
  SVG is still rendered without `--deny-warnings`.
- **Fixed**: two stylesheets with the same base name (a theme's `skemo.css` and a deck's) overwrote each
  other in the embedded source and after `skemo extract`; the second is now named `skemo-2.css`, and
  `extract` never writes the same file twice.
- **Codes for every warning of the engine**: `unknown-icon`, `image-missing`, `font-missing`,
  `font-unreadable`, `unknown-layout`, `curve-ignored`, `overlap`, `wrap-hides-cards`,
  `mindmap-unlinked`, `fit`, and the router's `unroutable`, `waypoint-unreachable`, `side-ignored`,
  `route-input`, with the line of the element they are about (`--diagnostics json`); no more `engine`.
  In the library, `Diagram::warnings` holds `EngineWarning { code, element, message }`; the WebAssembly
  module keeps `warnings` as texts and adds `engine_warnings`.
- **Stylesheet diagnostics** (D-095): `css-syntax` (a `{` never closed, a `}` without `{`, a declaration
  outside a rule or without `:`) and `css-property` (a typo of a property skemo or CSS knows: `colr`,
  `--lable`), at their line in the stylesheet. An author's own variables and valid CSS are left alone.
- **Fixed**: the variables the documentation sets on `:root` had no effect (`--link-mark`, `--zone-pad`,
  `--zone-head`, `--bg`, `--surface`, `--ink`, `--muted`, `--line`, `--card-border`, `--radius`…): the
  theme set them again on `.stage`, `card` or `zone`. The theme now sets them on `:root`, where an
  author's stylesheet wins (the dark and print themes still redefine the colours). `--radius` now also
  rounds the cards of the default look and the zones (zones: 4 px more), and a `calc(…)` in
  `border-radius` is read; the default drawing is unchanged.
- **Fixed**: the same diagram twice in one page, light and dark (or with other stylesheets), shared the
  identifiers of its SVG definitions (`sk…-ombre0`), so the second drew the first's shadows; the prefix
  now also depends on the theme and the stylesheets when they are not the defaults.
- A class `.stage` on a card or zone gives the warning `part-class`: it is the class of the whole drawing,
  and the theme's rules on it kept such a zone light in the dark theme.
- **Changed**: English messages quote names with backticks (`` unknown target `zz` ``); « » stays in
  French. Diagnostic codes and positions are unchanged.
- **Highlight markers** (D-090): `@3-4:halo`, `@5:chase`, `@6:magnify` after the other markers of a card, a zone,
  a line or an arrow, written in `data-highlight="halo:3-4 magnify:6"`; their steps count; not drawn in PNG, GIF,
  APNG, WebP. Asked by glitu, which wrote them itself.
- **A contract for the tools that run skemo** (`cli/SPEC.md` §2, D-090): `--diagnostics json` (one JSON object
  per line on stderr, with stable codes and positions), `--root DIR` (the files a diagram names are read inside
  it only: no `..`, no link leading out, no special file, at most 1 MiB, the type checked from the content),
  `skemo capabilities --format json`, `skemo render --batch` (many diagrams in one process), limits on a
  diagram's size (`too-large`).
- **SVG conventions, version 1** (`engine/SVG.md`): the attributes and elements an integration may rely on.
- **Changed**: an image or a stylesheet the diagram names that is missing or refused exits with 1 (the diagram
  is refused), no longer 2; every such file is reported, not only the first.
- **Changed**: an image whose content is not what its extension says (a `.svg` holding a PNG) is refused.
- **Changed**: `-` on a terminal is refused instead of waiting for input.

- **Changed**: zones stacked one under the other take the width of the widest by default (D-085);
  `diagram(zones="fit")` keeps fitted frames, `zones="stretch"` also evens them along `dir` right or left.
- **Changed**: in the default look, a card without lines keeps its title on the left, like its neighbours.
- A card outside a zone stands straight above its only target in it (Internet above a subnet's load
  balancer), once the zones have made room for their titles.
- Sibling zones linked both ways keep their writing order (a private subnet reaching out through the
  public subnet's NAT no longer goes above it); `constraint="false"` is no longer needed there (D-086).
- A clearer help and clearer messages: headings, values and defaults in the user's language, examples,
  `demo.skemo → demo.png` after a render, a summary after `lint`, file errors in the user's language, the
  replacement of a removed command.
- A subtitle line of the diagram's title keeps its classes (`|.warn text` under `title`), as a card's lines do.
- For contributors: a `local` profile, incremental (`cargo build --profile local`).

## [0.1.0-rc.8] — 2026-11-03

The tag v0.1.0-rc.7 was not published: its changes are released here.

- **Commands, in the order of a diagram's life** (D-083, D-084): `init`, `lint`, `render`, `extract`,
  `layout`. **Changed**: `lint` replaces `check` and `fmt` (`lint --fix` rewrites in canonical form,
  `--fix --dry` only reports) and `fit` (`lint --fit [16:9]`, applied by `--fix`); `render --watch`
  replaces `watch`; `skemo --licences` replaces `skemo licences`; `init --css-full` replaces
  `theme --print`. The former names are refused.
- **A diagram names its stylesheets** (D-082): `diagram(css="style.css")`, files next to it, read before
  `--css` and embedded in the outputs. `skemo init [file] [--css [name] | --css-full [name]]` writes
  `diagram.skemo`, and a short commented stylesheet or the whole default theme that it includes.
- **No `--lang` and no `--color`**: the language follows `SKEMO_LANG`, else the system; colours show on a
  terminal unless `NO_COLOR` is set.
- **A compact help**: one line per option, `render`'s options under Output, Style and Animation, generic
  options apart. Shell completion: `eval "$(skemo completions bash)"` (also zsh, fish, elvish,
  powershell).
- Warning `part-class`: a class named like a part the engine draws (`.head`, `.title`, `.line`…) would
  style that part everywhere.
- Org charts: the teams of a manager written between two others stay under it.
- CI: the checks build once in a `ci-fast` profile and run the tests once under coverage.

## [0.1.0-rc.7] — 2026-11-03

- **Arrows in motion** (D-079): `.pulse`, a new arrow style, runs one glowing dot along the line from the source
  to the head, every dot at the same speed; `.flow` dashes now run at a set speed. Tuned in CSS with
  `--flow-speed`, `--pulse-speed`, `--pulse-size` and `--pulse-color`. They play in HTML, GIF, APNG and
  animated WebP (a diagram without steps becomes one loop), and in SVG only with the new `--motion` option.
  **Changed**: a still image (PNG, still WebP, SVG without `--motion`) draws a `.flow` arrow as a solid line,
  no longer as fixed dashes.
- GIF: each frame after the first only holds the rectangle that changed, as APNG and WebP already did.
- **Site**: the documentation moves to <https://skemo.net/docs/> (docs.skemo.net is not used), in English and
  French, with a page per kind of diagram (architecture, cloud and network, processes, org charts, mind maps,
  timelines, friezes, animations, animated flows), each example's source in a tab, and a new look from the logo.

## [0.1.0-rc.6] — 2026-10-31

The tags v0.1.0-rc.4 and v0.1.0-rc.5 have no release (their publication failed).

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

[Unreleased]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.11...HEAD
[0.1.0-rc.11]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.10...v0.1.0-rc.11
[0.1.0-rc.10]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.9...v0.1.0-rc.10
[0.1.0-rc.9]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.8...v0.1.0-rc.9
[0.1.0-rc.8]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.7...v0.1.0-rc.8
[0.1.0-rc.7]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.6...v0.1.0-rc.7
[0.1.0-rc.6]: https://github.com/skemonet/skemo/compare/v0.1.0-rc.3...v0.1.0-rc.6
[0.1.0-rc.3]: https://github.com/skemonet/skemo/releases/tag/v0.1.0-rc.3
[0.1.0-rc.2]: https://github.com/skemonet/skemo/tree/28d0a43
