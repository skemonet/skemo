# skemo format reference

Complete reference of the `.skemo` language as implemented by `skemo` 0.1. The specification of record is
`FORMAT.md` in the `skemo-lang` repository; the documentation site is https://docs.skemo.net/.

## Contents

1. Syntax
2. `diagram` options
3. Cards and zones
4. Arrows
5. Positions and adjustments
6. Styling with CSS
7. Themes and contrast
8. Animation markers
9. Timeline mode
10. Frieze mode
11. Mindmap mode
12. Icons and links
13. Diagnostics
14. Command line

## 1. Syntax

A file is a valid Pug document restricted to the tags `diagram`, `title`, `card`, `cards`, `zone`, `stack`
and `to`, and lines written with Pug's text marker `|`, whose text follows Markdown (emphasis, tables).

```
tag        := name ("#" id | "." class)* attributes? (" " text)?
attributes := "(" attr ((","|" ")+ attr)* ")"
attr       := ident "=" value | ident          ; a bare ident is an arrow target
value      := "double" | 'single' | number
comment    := "//-" text | "//" text           ; ignored, with their indented block
piped      := "| " text                        ; appended to the parent's title
```

- Nesting is by indentation: 2 spaces recommended, tabs allowed, never both in one file.
- The text after the tag is the title (`card`, `zone`) or the label (`to`); a line `| text` is a line of the
  card, zone or title it is indented under. Classes stick to the bar: `|.warn text`.
- **One attribute group per tag**: `to(core side="right-top").dashed`, not `to(core).dashed(side=…)`.
- Unknown tags are ignored with a warning.
- File extension `.skemo`; any extension is accepted.

## 2. `diagram` options

| Attribute | Values | Default | Effect |
|---|---|---|---|
| `dir` | `down` `right` `up` `left` | `down` | main direction of the layout |
| `layout` | `graph` `timeline` `frieze` `mindmap` | `graph` | layout mode |
| `look` | `band` `header` `classic` `eyebrow` | `band` | card look for the whole diagram |
| `theme` | `light` `dark` `print` `auto` | `light` | `auto` follows the system (HTML only); the CLI `--theme` overrides it |
| `lang` | `en` `fr` | the user's language | language of labels the engine draws (months, centuries, "BC") |
| `elk` | JSON of raw ELK options | | fine tuning; avoid in shared files |
| `fit` | `16:9`, `4:3`, `1.5`, `1280x720` | | target box (pre-release): a warning names the `dir`/`wrap` setting that shows the text largest once fitted; `skemo fit --write` applies it; when nothing helps and the proportions are over 15 % off, it says so. Never changes the layout by itself |

`diagram(…)` is optional; when present, everything else is indented below it.

## 3. Cards and zones

```pug
zone#lan.lan Internal LAN
  card#core.t1(tag="Cisco") Core switch
    | 10.10.0.0/16 VLAN 10
    | 10.20.0.0/16 VLAN 20
  zone#servers Servers
    card#srv.t2 File server
```

| Element | Meaning |
|---|---|
| `#id` | unique identifier, needed to be the target of an arrow |
| `.class` | type(s), styled in CSS; several allowed (`.t1.critical`) |
| text | title |
| `\| text` | content line of a card (description line under a zone's title); a leading IPv4 or CIDR is set in monospace; `\|.pill 8080` (pre-release): a pill tinted with the card's colour |
| `tag="…"` | text of the card's chip/badge, replaces the type's `--label`; on a zone (pre-release), a pill after its title |
| `icon="name"` | built-in icon before the title (section 12) |
| `href="…"` | the card or zone is a link (section 12) |
| `at="x,y"` | pinned top-left position, in px (section 5) |
| `size="w,h"` | zone only, with `at`: imposed size |

A zone may contain lines, then zones and cards. A card contains only lines `|` and `to`. A zone passes its accent colour
on to the untyped cards it contains. An empty zone keeps a visible frame.

Predefined classes:

| Class | On | Rendering |
|---|---|---|
| `.note` | card | annotation: dashed border, tinted background, centred text, no badge (amber) |
| `.chip` | card | one-line pill with a coloured outline (purple) |
| `.info` | card | small discreet box (amber) |
| `.bare` | zone | no frame; the title becomes a caption below, centred and italic; without a title, an invisible group |
| `.wide` | card | (pre-release) spans the width of its group, like a network bar; its arrows drop straight to it |
| `.wide` | zone | (pre-release) spans the width of its group; its cards, on one row, share that width, evenly spread. Put it on the one row that carries the columns (the machines), not on every row: a row of pods marked `.wide` is spread evenly and its services can no longer be centred under their pods |
| `.tag-left`, `.tag-right` | card, zone | (pre-release) the badge before or after the name |
| `.col-name` | card, zone | (pre-release) nodes of one `.col-…` class line up across `dir`, from zone to zone (pods over their machine) |

Default theme types: `.t0` (red, "T0"), `.t1` (orange, "T1"), `.t2` (blue, "T2"), `.t3` (grey, "T3"),
zones `.cloud`, `.dmz`, `.lan`.

**Piles** (pre-release). `cards` instead of `card` draws a pile of cards (several instances: the tasks of a
service, the machines of a group); it takes everything a card takes. `count="4"` draws up to three cards and a
`×4` badge on the bottom-right corner; without it, three cards and no badge.

```pug
diagram(dir="right")
  card#lb Load balancer
    to(api)
  cards#api.t2(count="4" tag="ECS") api
    | 4 tasks
```

**Title and stacks** (pre-release). `title` (once, directly under `diagram`) draws a title above the
drawing; its `|` lines are subtitle lines under it, its `|.pill` lines pills on the right. `stack` is an invisible group whose children (zones or
cards) are laid one after the other in writing order along `dir`, whatever the arrows between them;
arrows still align what they link, and one going back up the stack leaves by the top, enters by the
bottom; its zones share one width. Use it to order layers that are not linked (application above infrastructure).

```pug
diagram(dir="down")
  title Kubernetes application
    |.pill namespace demo
  stack
    zone#pods(tag="172.30.0.0/16") Pods
      card#fe1 frontend-1
      card#fe2 frontend-2
    zone#svcs(tag="172.21.0.0/16") Services
      card#svc Frontend
        to(fe1 fe2)
    zone#nodes.bare
      card#cp Control Plane
      card#w1 Worker 1
    card#net.wide Cluster network
      to(cp w1)
```

## 4. Arrows

An arrow is written **inside its source card**: `to(target).classes Label`.

| Element | Meaning |
|---|---|
| `to(a)` | arrow to card `a` |
| `to(a b c)` | one arrow per target, same classes and label |
| `to(a b c join="trunk")` | hyperedge: the arrows leave one point and share a trunk, then branch |
| `to(a join="http")` | (pre-release) arrows to `a` that carry the same name, written under different cards, meet and end on one line with one head |
| `to(a constraint="false")` | (pre-release) drawn and routed, but ignored by the layout: for cross links that would stretch the diagram into a chain |
| `.class` | styles (below) or your own classes |
| text | label |
| `side="start-end"` | attachment sides among `top` `bottom` `left` `right`: `side="right-top"`; one half may be empty: `side="-top"` |
| `via="x,y x,y"` | waypoints the route must pass through, in order |
| `href="…"` | the arrow is a link |
| `#id` | explicit id: `to#e1(core)`; otherwise `source->target` (`#2`, `#3`… for duplicates) |

Provided styles, combinable: `.dashed` `.dotted` `.thick` `.flow` (animated dashes) `.both` (head at both
ends) `.nohead` `.dot` (dot at the start) `.link` (dotted, no head: attaches a note), colour examples
`.wan` (red, thicker) and `.secure` (green).

Every arrow also gets the classes `from-<type>` and `to-<type>` of its endpoints:
`.edge.from-uplink { color: #7B5CC4; }` colours every arrow leaving an `.uplink` card.

Routing: every arrow is routed orthogonally around cards, and around zones that contain neither of its
ends; segments are centred in the free corridor.

## 5. Positions and adjustments

Layout is automatic. Pin only what the user wants fixed:

- `at="x,y"` on a card or zone: pinned top-left corner (px). Other cards keep their automatic place.
- `side=` and `via=` on an arrow: force sides or waypoints.
- Overlaps are never corrected: a card pinned on top of another gives a warning naming both.
- Stability: changing one card only moves the arrows attached to it.
- `wrap="N"` on a zone (pre-release): at most N children per row (per column with `dir="right"`), on a grid.
  Only when the arrows reach the zone from a side where every card stays visible: a card in the second
  column (with `dir="right"`) cannot be reached straight from the left, and the render warns.
  `wrap="1"` (a single file: a server and its stack) never warns: arrows enter by the free sides. The cards
  of a grid column share the widest one's width, left-aligned.
- `diagram(zones="stretch")` (pre-release): zones stacked along `dir` get the same width (layers).
- Cards of one row keep the writing order; an arrow written after its target still follows `dir`.
- `at=` has no effect in timeline, frieze and mindmap modes (warning).

## 6. Styling with CSS

Pass stylesheets with `--css file.css` (repeatable); they apply after the default theme with the normal
cascade. Print the default theme with `skemo theme --print`.

A type defines only its **accent** and its **label**; backgrounds, borders and inks are derived:

```css
.server  { --c: #5B6CFF; --label: "SERVER"; }
.lan     { --c: #3E6FC4; }                        /* a zone type */
.hr      { --z-bg: #F3F0FF; --z-border: #D0C6FF; --z-ink: #5F3DC4; }   /* explicit zone tints */
.edge.important { color: #E8590C; }               /* arrow colour: set `color` */
.segment { --look: header; }                      /* look per type */
:root    { --radius: 18px; --font: 'Inter', sans-serif; }
[data-id="fw"] { --c: #C94A44; }                  /* one element by id */
```

| Variable | Where | Role |
|---|---|---|
| `--c` | card or zone type | accent colour |
| `--label` | card type | badge or chip text; absent = no badge |
| `--look` | card type | `band` `header` `classic` `eyebrow` |
| `--icon` | type | default icon name; `icon=` overrides it |
| `--icon-size`, `--icon-color`, `--icon-gap` | type | icon tuning |
| `--link-mark` | `:root` or type | `none` hides the link glyph |
| `--on-c` | type | colour of text on the accent (default white) |
| `--z-bg`, `--z-border`, `--z-ink` | zone type | override the derived zone tints |
| `--zone-pad`, `--zone-head` | `:root` | zone margins (used by the layout) |
| `--bg`, `--surface`, `--ink`, `--ink-2`, `--muted`, `--line`, `--card-border` | `:root` | base colours, redefined by each theme |
| `--font-scale` | `.stage` | multiplies every text size of the theme (`1.4` for a slide; pre-release) |
| `--font`, `--mono`, `--radius` | `:root` | typography and corner radius |

Do not hard-code background or text colours in types: use the variables, so dark and print themes work.
Looks: `band` = coloured band with the name and a chip (default); `header` = band with the type label,
name centred below; `classic` = accent line on top, badge before the name; `eyebrow` (pre-release) = white
card, type label above the name. A card without a type is drawn light in every look. `.note`, `.chip`,
`.info` and timeline/frieze/mind-map cards always keep their own style.

Rendered selectors, for advanced CSS: `card`, `zone`, `card > .title`, `card li` (one per line `|`), `card table`, `card mark`, `.edge`, `.edge .line`,
`.edge .head`, `.edge-label`, `[data-id="…"]`.

## 7. Themes and contrast

| Theme | Use |
|---|---|
| `light` | default |
| `dark` | dark screens; tints recomputed from `--c` |
| `print` | white background, black ink, darker fills: readable in black and white; also applied when printing the HTML |
| `auto` | HTML follows the reader's system; PNG and SVG render as `light` |

`skemo check --contrast f.skemo` reports texts below WCAG 4.5:1 (3:1 on accent fills).
`skemo render --contrast high` darkens or lightens those texts without changing the layout.
Information must never be carried by colour alone: give types a `--label` and meaningful arrows a style.

## 8. Animation markers

Markers go at the **end of a line**, after a space, on `card`, `zone`, a line `|` (or a table row) or `to`:

| Marker | Meaning |
|---|---|
| `@2` | appears at step 2 (effect `fade`; `draw` for arrows) |
| `@2:up` | appears at step 2 with effect `up` (`fade` `up` `zoom` `draw`) |
| `@2-4` | visible from step 2 to step 4 included |
| `@0-2` | (pre-release) visible from the start to step 2 included: `| A @0-2` then `| B @3` is one line whose text changes at step 3 |
| `@4:focus`, `@4-6:focus` | highlighted at step 4 (or 4 to 6); does not control appearance |
| `@2.5` | steps are ordered values: 2.5 slots between 2 and 3 |

`tag="8080 @1"` animates the chip alone (pre-release; in HTML not with the `classic` look).

Rules: at most one appearance marker per node, any number of `:focus`. Only trailing tokens are read
(`meet @3 tomorrow` stays text). `\@` writes a literal `@`. A child never appears before its parent; an
arrow without marker appears with the later of its two ends.

Outputs: `-o f.html` (step reader: arrow keys, space, PageDown), `-o f.gif`, `-o f.apng`, `-o f.webp`
(animated), `--step 2` (one state as PNG/SVG), `--animated` (one PNG per state), `--board` (contact sheet
of all states). GIF/APNG timing: `--hold`, `--transition`, `--fps`, `--final-hold`, `--no-loop`.

## 9. Timeline mode

```pug
diagram(layout="timeline" start="2026-10" end="2027-06" scale="month" today="2026-10-15" lang="en")
  card#launch.milestone(date="2026-10-14") Specs delivered
  zone#engine.lane Engine
    card#e0.spike(start="2026-10-16" end="2026-11-07") Spikes
      to(valid)
  zone#deliv.lane Deliverables
    card#valid.decision(date="2026-11-09") Validation
```

| Attribute | On | Meaning |
|---|---|---|
| `layout="timeline"` | diagram | enables the mode |
| `start`, `end` | diagram | axis bounds (`end` included); default = extreme dates ± 7 days |
| `scale` | diagram | `day` `week` `month` (default) `quarter` `year` |
| `width` | diagram | imposed axis width in px |
| `today` | diagram | date of the "today" marker |
| `date` | card | milestone: left edge on the date, diamond on the axis |
| `start`, `end` | card | period: a bar measures the duration; `end` is **inclusive** |

Dates: `YYYY`, `YYYY-MM`, `YYYY-MM-DD`. Top-level zones are lanes, in writing order; cards outside any zone
form an unframed lane at the top. Overlapping cards of a lane stack on rows. Arrows are dependencies.
Every card needs `date` or `start`.

## 10. Frieze mode

One central axis for long spans, BC dates included.

```pug
diagram(layout="frieze" start="-800" end="600" scale="century" lang="en")
  card#rep.rep(start="-509" end="-27") Republic
  card#emp.emp(start="-27" end="476") Empire
  card#caesar.evt(date="-44-03-15") Death of Caesar
    | Ides of March
  card#fall.end(date="476") Fall of the West
```

| Attribute | On | Meaning |
|---|---|---|
| `layout="frieze"` | diagram | enables the mode |
| `start`, `end` | diagram | axis bounds in years; `end` is included (`end="2017"` ends on the 2018 graduation); an era's `end` too |
| `scale` | diagram | `year` `decade` `century` (default) `millennium` |
| `width` | diagram | imposed axis width |
| `date` | card | event: card above or below the axis, stem to the date |
| `start`, `end` | card | era: band on the axis; `end` is a bound (the next era starts there) |

Dates: a year, negative for BC, optionally `-MM` or `-MM-DD`. **There is no year 0** (error). (Pre-release)
a top-level zone frames its events on one side of the axis; zones do not nest. Eras stack under the axis,
longest first.

## 11. Mindmap mode

```pug
diagram(layout="mindmap")
  card#root.root skemo
    to(lang engine)
  card#lang Language
    to(parser)
  card#parser Parser
  card#engine Engine
```

The tree comes from the arrows: the targets of a card's `to(…)` are its children, in writing order.
`root="id"` picks the centre; by default the first card that no arrow targets. Root children are spread
right and left; arrows of one parent share a trunk. Zones are not allowed; `at=` has no effect; an
unreachable card is placed below with a warning.

## 12. Icons and links (pre-release)

```pug
card#web.app(icon="globe") Storefront
  to(db href="https://wiki.example.com/uplinks") uplink
card#db(icon="database" href="https://wiki.example.com/db") Orders
zone#lan(href="#core") LAN
```

- `icon="name"`: an icon of the built-in Lucide set (kebab-case). `skemo icons <word>` searches names and
  tags. (Pre-release) `icon="img/logo.svg"`: an image file (svg, png, jpg, webp), relative to the source
  and inside its folder; it keeps its colours and travels inside the rendered file. `icon="none"` removes the type's default icon. An unknown name draws nothing and warns with a
  suggestion. Icons are for cards and zones (ignored with a warning on `to`).
- `href="…"`: `http:`, `https:`, `mailto:`, relative paths and `#fragments` only; anything else
  (`javascript:`, `data:`, `file:`…) or an empty value is the error `unsafe-href`. Clickable in SVG and HTML;
  a small link glyph follows the name in every format (`--link-mark: none` hides it).

## 13. Diagnostics

`skemo check` prints errors and warnings with file, line, column, a caret and often a hint; exit code 1 on
errors. Blocking errors:

| Code / cause | Fix |
|---|---|
| `unknown-target` | the id in `to(…)` does not exist: check spelling (a suggestion is given) |
| `duplicate-id` | two cards or zones share an id |
| `to` outside a card | move the arrow under its source card |
| several attribute groups | merge into one `(…)` |
| mixed tabs and spaces | re-indent with spaces only |
| bad marker (`@0`, `@4-2`, unknown effect, two appearance markers) | fix the marker |
| malformed `at`, `size`, `via`, date, year 0 | numbers / dates expected |
| `unsafe-href` | use an allowed scheme |

Warnings never block: `unknown-icon`, `icon-ignored`, overlaps of pinned cards, markers earlier than their
parent, `at=` in a mode where it has no effect, unreachable mindmap cards.

`skemo fmt f.skemo` rewrites the file in canonical form (`--check` only reports).

## 14. Command line

| Command | Role |
|---|---|
| `skemo f.skemo` | render `f.png` next to the source (same as `skemo render f.skemo`) |
| `skemo render f.skemo -o out.ext` | format from the extension: `png` `svg` `webp` `html` `gif` `apng` |
| `skemo check f.skemo…` | diagnostics only; `--contrast` adds the legibility check |
| `skemo fmt f.skemo… [--check]` | canonical formatting |
| `skemo watch f.skemo` | live preview on `http://127.0.0.1:8710`, reloads on save |
| `skemo layout f.skemo --json` | positions and routes as JSON |
| `skemo fit f.skemo [--target 16:9\|1280x720] [--write]` | settings that show the text larger in a target box (pre-release) |
| `skemo extract image.png [-o dir]` | source and stylesheets embedded in a rendered image (`--stdout`: source only) |
| `skemo icons [query]` | list built-in icons |
| `skemo theme --print` | print the default theme CSS |
| `skemo init [folder]` | sample diagram and commented CSS |

Render options: `--css f.css` (repeatable), `--theme light|dark|print|auto`, `--scale 2` (raster
resolution), `--transparent`, `--contrast high`, `--step N`, `--animated`, `--board`, `--base-theme t.css`,
`--deny-warnings`, `--no-fonts` (HTML), `--embed-fonts` (SVG: fonts inside, ~0.8 MB per weight),
`--font-scale 1.4` (every text size; for slides). The SVG root carries `data-states` (ordered steps).
Every output embeds its source and stylesheets (PNG iTXt chunk, SVG `<metadata>`, WebP/GIF/HTML too);
`--no-source` leaves them out; any command accepts such an image instead of a `.skemo`. `-` as input reads standard input; `-o -` with `--format` writes to
standard output. Messages follow the system language; `--lang en|fr` overrides.

Exit codes: 0 OK, 1 errors in the file, 2 wrong usage, 3 internal error.
