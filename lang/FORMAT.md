# Diagram format — specification (v0.8 proposal: the language redefined; v0.5 is the last validated version)

> This document is the **contract** of the format. Any evolution of the parser must start with an update here.
> **v0.8** (2026-10-28, agreed with the user in conversation, to be recorded as a decision): content lines are written `|` instead of `li` (§3.2.3), with Markdown inline emphasis (§3.2.4) and Markdown tables (§3.2.5); a card's category `label=` beside its `tag=` (§3.2.6); state classes `.ok` `.warn` `.error` (§3.2.7); legends `card.legend` and plain text `card.text` (§3.2.8); elements docked around the drawing with `at="s"`, `at="ne"`… (§3.2.9); dates: `end` included in every mode, the axis bounds written `start`/`end` (§3.6, §3.7). Clean-up: the v0.7 concepts and the `shape`/`symbol` looks are set aside (`proposals/concepts-v0.7.md`); the prototype's `engine`, `router` and `elk` attributes are removed; its old spellings are only read for compatibility (§7). The language is not public yet: these changes break nothing outside the project.
> v0.6 (icons, links: §3.2.1) is implemented, awaiting the user's validation.
> Points marked **[Rust]** are not yet in the prototype and are to be implemented in the Rust version. *v0.8* marks what is new in v0.8 and not implemented yet.

## 1. Principle

A diagram is a **valid Pug** file, restricted to a subset: **7 tags**, and **lines** written with Pug's text marker `|`. Meaning comes from classes, styled in CSS; the text inside lines follows Markdown (emphasis, tables).

| Tag | Role | Content |
|---|---|---|
| `diagram(...)` | optional root, global options | everything else, indented below |
| `title` *(preview, D-050)* | the diagram's title, written under `diagram` | text = title · `\|` lines = subtitle lines · `\|.pill` = pills beside it |
| `card#id.type` | a card | text = title · `\|` lines = content lines or table rows · `to(...)` = outgoing arrows |
| `cards#id.type` *(preview, D-062)* | a card drawn as a pile: several instances of one thing | as `card`; `count="N"` adds a `×N` badge |
| `zone#id.type` | a group | text = title · `\|` lines = description lines under the title · indented cards or zones |
| `stack` *(preview, D-050)* | an invisible group that lays its children one after the other | indented cards or zones |
| `to(target).type` | an arrow, written in its source card | text = label (optional) |

| Line | Role |
|---|---|
| `\| text` | a line of text under the title of its card, zone or diagram title (§3.2.3) |
| `\|.class text` | the same line, with classes (`.warn`, `.pill`, a legend's arrow class…) |
| `\| a \| b \| c \|` | a table row: a line that ends with `\|` (§3.2.5) |

**Language of the drawing's labels.** `diagram(lang="en")` or `lang="fr"` fixes the language of the labels the engine draws itself (months, quarters, centuries, "BC"). Without it they follow the language of the messages (the user's system language for the CLI and the editor; English for the libraries), so a file meant to render identically everywhere should pin it.

```pug
//- comment, not rendered
diagram(dir="down")
  card#fw.t0(label="Edge firewall") Firewall
    | 10.0.0.1/24 · **HA pair**
    to(core).wan transit
  zone#lan.lan Internal LAN
    | 3 floors
    card#core.t1 Core switch
```

## 2. Syntax (Pug subset)

```
file        := line*
line        := indentation (comment | tag | text_line) end_of_line
indentation := spaces* | tabs*                    ; no mixing within a file
comment     := "//-" text | "//" text             ; ignored (with their indented block)
tag         := name (("#" ident) | ("." ident))* attributes? (" " text)?
name        := "diagram" | "title" | "card" | "cards" | "zone" | "stack" | "to"
attributes  := "(" attribute ((","|" ")+ attribute)* ")"
attribute   := ident "=" value | ident            ; a lone ident = flag (arrow target)
value       := string_in_"_or_' | number
text_line   := "|" ("." ident)* (" " inline)?     ; a line (§3.2.3)
             | "|" cell ("|" cell)* "|" markers?  ; a table row: ends with "|" (§3.2.5)
cell        := ("." ident)* inline? | "^^" | ""   ; "" (nothing, not even a space) = joined to the cell on its left
inline      := Markdown emphasis: **bold** *italic* `mono` ==mark== ==.class mark== (§3.2.4)
ident       := [A-Za-z_][A-Za-z0-9_-]*
```

Rules:
- **A single group of attributes per tag**: `to(core side="right-top").dashed` and not `to(core).dashed(side="right-top")`. Pug refuses it.
- Nesting is done through indentation (2 spaces recommended).
- The text after the tag, on the same line, is the **title** (`card`, `zone`, `title`) or the **label** (`to`).
- A line `|` belongs to the tag it is indented under: content line of a card, description line of a zone, subtitle or pill of the diagram's title.
- **Classes stick to the symbol that opens**: `card.db`, `|.warn text`, `|.error cell |`, `==.ok text==`. A space after the opening symbol starts plain text: `| .env file` is the text ".env file".
- Unknown tags are ignored with a warning **[Rust]** (the prototype ignores them silently). Old spellings read for compatibility: §7.

## 3. Semantics

### 3.1 `diagram`

| Attribute | Values | Default | Effect |
|---|---|---|---|
| `dir` | `down` `right` `up` `left` | `down` | main direction of the automatic layout |
| `layout` | `graph` (default) · `timeline` · `frieze` · `mindmap` | — | layout mode (§3.6, §3.7, §3.9) |
| `look` | `band` `header` `classic` `eyebrow` | `band` | look of the cards of the diagram (§4.5) |
| `zones` *(preview, D-045)* | `stretch` | — | sibling zones stacked along the direction (one above the other with `dir` down or up) take the width of the widest (the height with right or left), like layers drawn by hand. Only the frames grow; no card moves. Zones side by side and pinned zones (`at` + `size`) are left alone |
| `theme` | `light` (default) · `dark` · `auto` · `print` | `light` | theme (§4.6); `auto` follows the system (HTML only); the CLI can impose it (`--theme`) |
| `fit` *(preview, D-041, D-043)* | ratio `16:9`, `4:3`, `1.5`, or box `1280x720` | — | target box: **a check, never an automatic change** (stability). What counts is the size of the text once the diagram is fitted in the box: when a written setting (`dir`, `wrap`) would show it at least 15 % larger, the engine warns and names it (with a box in px: the text size before and after); `skemo fit --write` puts it in the file. A setting that would make the arrows harder to follow (a card hidden by `wrap`, many more bends) is never suggested (D-044). When the proportions are more than 15 % off the target and no setting helps, the engine says so too (D-045) |

### 3.2 `card` and `zone`

| Element | Meaning |
|---|---|
| `#id` | unique identifier (target of arrows). If absent, the prototype generates `card1`, `zone2`…; **[Rust]**: warning if a card without an id is targeted by an arrow (impossible) |
| `.class` (several possible) | types, styled in CSS: colour, badge, etc. |
| text | title |
| `\| text` (children) | *v0.8* lines (§3.2.3): content lines of a card, description lines under a zone's title. If a line starts with an IPv4/CIDR, that IP is rendered as `span.mono`. *(preview, D-051)* `\|.pill 8080 · http`: the line's classes reach its style; a line with a background or a border (the theme's `.pill`: a pill tinted with the card's accent) is drawn as a box around its text |
| `label="RDS PostgreSQL 16.4"` (`card`, `cards`) | *v0.8* the card's **category**, drawn above its name (§3.2.6). Default: the type's `--label`; `label=""` shows none |
| `icon="name"` (v0.6) | icon drawn before the name, from the built-in set (Lucide, kebab-case: `database`, `server`, `user`). Overrides the type's `--icon`. `icon="none"`: no icon, even if the type has one. *(Preview, D-046)* `icon="img/logo.svg"`: an **image file** (§3.2.1). Ignored with the warning `icon-ignored` on `to` and on lines |
| `href="…"` (v0.6) | the card or zone is a link in the HTML and SVG outputs (§3.2.1) |
| `tag="Owner"` (`card`; `zone` *preview, D-050*) | the card's **chip** (an owner, a module, a port), beside its category (§3.2.6); on a zone, a pill after its title (`zone#pods(tag="172.30.0.0/16") Pods`). *(preview, D-041)* `@` markers at the end of the value animate the chip alone: `tag="8080 @1"` (not in the HTML `classic` look, where the chip is a pseudo-element) |
| `wrap="N"` (`zone`, *preview, D-041*) | (D-056: when an arrow from outside enters a card beyond the first row, the rows are spread like the diagram's, the rows of sibling `wrap` zones too, and the zone keeps a corridor that other arrows may cross) at most N children per row (per column with `dir` right or left), on a grid, in writing order; the cards of one column of the grid take the width of the widest and are left-aligned, so that a stack reads as a clean column (D-045). A warning names the cards that end up behind others as seen from the node they are linked to (second column with `dir="right"`): their arrows can only go round them (D-044); not with `wrap="1"`, a single file of cards that arrows enter by their free sides (a server and its stack, D-045). *(preview, 2026-10-28, to validate)* A zone without `wrap` that holds only cards, at least four, none with an arrow (an inventory: monitors, secrets, registries) is laid as a grid of ⌈√n⌉ columns rather than one long row; write `wrap=` to choose |
| `at="x,y"` | **pinned position**, top-left corner in px, in the file's frame |
| `at="s"` (`n` `s` `e` `w` `ne` `nw` `se` `sw`; top-level `card`, `cards`, `zone`) | *v0.8* **docked**: the element leaves the drawing and goes on that side of it, or in that corner (§3.2.9) |
| `size="w,h"` | a zone (with `at="x,y"`): **imposed** size, otherwise the zone encloses its children. A card (*preview, editor*): **minimum** size, it takes at least that room and its text may make it larger; written by the editor when a card is resized |
| `@…` at the end of the line | **animation markers** (§3.8) |

A zone contains lines, then cards, zones and stacks. A card contains lines and `to`.

**`cards`** *(preview, D-062)* is a card in every respect (`#id`, classes, `label`, `tag`, `icon`, lines, `to`, markers) drawn as a **pile**: up to two cards show behind it, 5 px right and down each, like a pile of playing cards, for several instances of one thing (the tasks of a service, the machines of a group). `count="N"` (a whole number ≥ 1, `cards` only, otherwise the warning `bad-count`) says how many: `min(N, 3)` cards are drawn, three when there is no count, and `N > 1` adds a `×N` badge (`card .count` in the theme) whose right edge is the pile's, centred on the top card's bottom edge, the same place in every look. The cards behind are part of the card's box: arrows end on them, gaps and routes keep clear of them, and the top card keeps its siblings' width. Graph layout only; in timeline, frieze and mind map modes a pile is laid out like a card.

```pug
cards#api.service(count="4" tag="ECS") api
  | 4 tasks
  to(db)
```

#### 3.2.2 Title and stacks *(preview, D-050)*

```pug
diagram(dir="down")
  title Kubernetes application architecture
    | Production cluster, eu-west-3
    |.pill namespace mon_namespace
  stack
    zone#pods(tag="172.30.0.0/16") Pods
      card#fe1 frontend-1
    zone#svcs(tag="172.21.0.0/16") Services
      card#svc Frontend
        to(fe1)
    zone#nodes.bare
      card#cp Control Plane
      card#w1 Worker 1
    card#net.wide Cluster network
      to(cp w1)
```

**`title`** is written once, directly under `diagram`: its text is drawn above the drawing, on the left. *v0.8* Each line `|` is a **subtitle line** under it (smaller, muted: the account, the source, the date); each `|.pill` line is a **pill** on the right of the title row (a namespace, a version, an owner). A second `title`, or one written elsewhere, is a warning and is ignored. The title widens the image if it is wider than the drawing; it never moves a card. Selectors: `.diagram-title > .text`, `.diagram-title > .sub`, `.diagram-title li.pill`.

**`stack`** is a group without frame nor title (an `#id` is allowed, a text is ignored with a warning): its children, cards or zones, are laid one after the other **in writing order along `dir`** (the first at the top with `dir="down"`, at the bottom with `up`, on the left with `right`), 48 px apart, centred across, whatever the arrows between them. The arrows between two children of a stack still align the cards they link (a service under its pods), but never change the stack's order: one going against the order leaves by the side facing back and enters by the side facing forward, straight or as a fork, and is not a back edge. Children linked by arrows keep that alignment and are centred together; the others are centred on their own; a `.wide` child takes the whole width. The zones of a stack take one width, like `zones="stretch"` (D-056): a stack is a pile of layers. A stack can sit in a zone or in another stack. A card can belong to one group only: zones and stacks form a tree. A `.bare` zone without a title groups cards without drawing anything (no frame, no caption).

#### 3.2.1 Icons and links (v0.6)

```pug
card#web.app(icon="globe") Storefront
  to(db href="https://wiki.corp/uplinks") uplink
card#db(icon="database" href="https://wiki.corp/db") Orders
zone#lan(href="#core") LAN
```

**Icons.** The value is a name of the built-in set, or an image file. Reserved, **not supported yet**, so that the syntax stays free: icon packs (`aws:rds`); remote URLs are refused for good (rendering is deterministic and offline).

**Image files** *(preview, D-046)*: a value ending in `.svg`, `.png`, `.jpg`, `.jpeg` or `.webp` names a file, by a path **relative to the source file and inside its folder** (`img/logo.svg`; no `..`, no absolute path, no URL: the error stops the CLI). The file is recognised from its content, 1 MB at most. It is drawn in the icon's square box, scaled to fit with its proportions kept, **in its own colours**, and always as an image: an SVG is never inlined (its scripts and links have no effect; the PNG renderer resolves nothing but the image's own data). The file travels **inside every rendered file** with the source (D-042), so a rendered image reopens with its icons, and `skemo extract` writes them back at their path. The engine reads no file itself: a program using the library hands the files over (`Options::images`, `images::referenced` lists them). A file not provided draws nothing, reserves no space and warns. Logos are their owners' trademarks: use the ones you are allowed to. An unknown name draws nothing, reserves **no space** and gives the warning `unknown-icon` with a suggestion. Placement (a built-in icon takes the colour of the text it precedes):

| Look | Where | Size | Colour |
|---|---|---|---|
| `band` | in the band, left of the name | 16 px | `--on-c` |
| `header` | in the band, left of the type label | 14 px | `--on-c` |
| `classic` | before the badge, on the title line | 16 px | accent |
| `.note`, `.chip`, `.info`, timeline and frieze cards | before the title | 14 px | accent |
| zone | before the zone title | 16 px | the zone title ink |

Arrows and lines have no icon. Contrast of an icon: 3:1 minimum (non-text graphic).

**Links.** Accepted values: `http:`, `https:`, `mailto:`, a relative path (`docs/fw.md`), a fragment (`#id`). Any other scheme (`javascript:`, `data:`, `vbscript:`, `file:`…) is the blocking error `unsafe-href` (§3.5); so is an empty value. SVG: the element is wrapped in `<a href>`. HTML: `<a class="href" href rel="noopener">`, external links open in a new tab. PNG, GIF, APNG, WebP: not clickable. A linked card or zone shows a small `external-link` glyph (12 px) after its name, which **reserves its width in every output** so the layout never depends on the format; `--link-mark: none` hides it and frees the space. Arrows get no glyph.

**Stability** (D-031): adding or removing an icon, a link or a glyph changes the size of that card only; only its arrows may move.

**Predefined classes** provided by the theme (combinable with a type):

| Class | Element | Rendering |
|---|---|---|
| `.note` | card | annotation: **dotted** border, tinted background, centred title and lines, no badge (default accent: amber) |
| `.chip` | card | rounded **pill** on one line, coloured outline (default accent: purple) |
| `.info` | card | small discreet **box** (default accent: amber) |
| `.bare` | zone | group **without frame**, legend below, centred and italic (e.g. "Uplinks / VLAN" under chips); without a title, nothing is drawn and no room is kept |
| `.wide` *(preview, D-050)* | card | spans the width of its group (the drawing, its zone or its stack) across `dir`, like a network or a bus bar; each of its arrows runs straight from the other card, ending in a dot on the bar, and never forks |
| `.wide` *(preview, D-051)* | zone | spans the width of its group like a wide card; its cards, on one row, share that width (equal widths, 40 px apart) when none of them gets narrower. Cards that carry columns (`.col-<name>`) are each as wide as the widest block of any of their columns, 48 px apart, so that every block fits over its card (D-054). Its arrows are ordinary arrows |
| `.tag-left`, `.tag-right` *(preview, D-051)* | card, zone | where the badge goes: before or after the name (card: the band's tag or the inline badge of the look; zone: `.tag-right` sets the `tag=` pill at the right end of the title row). Read on the element's own classes: a zone's choice does not reach its cards |
| `.ok`, `.warn`, `.error` *(v0.8)* | card, line, cell, mark | **state** (§3.2.7): a mark before the title or the line and the state's colour (green, amber, red) |
| `.legend` *(v0.8)* | card | a **legend** (§3.2.8): each line shows a sample of the class it names |
| `.text` *(v0.8)* | card | **plain text**: no frame, no background, no badge; muted lines (a footnote, an annotation) |
| `.col-<name>` *(preview, D-051)* | card, zone | **columns**: nodes of one `.col-<name>` share an axis across `dir`, whatever zone or stack they sit in (a service and its pods over the machine that hosts them). In each row, a column's members form a block, centred on the axis of the column's block in a `.wide` zone if there is one, otherwise of its widest block; the blocks of one row keep 48 px between them, in the order of their axes; a block that would run into a card outside the columns stays where it is |

`.note`, `.chip`, `.info`, `.legend` and `.text` cards stay in plain style in every look.

#### 3.2.3 Lines *(v0.8)*

A line is Pug's text marker `|` followed by a space and its text, indented under the element it belongs to. It replaces the former `li` tag (a word that read as a list).

| Under | A line is |
|---|---|
| `card`, `cards` | a content line, under the title; a line that ends with `\|` is a table row (§3.2.5) |
| `zone` | a description line under the zone's title ("3 availability zones", "outside the VPC"), smaller and muted; it widens the zone if it is wider than its content, never moves a card |
| `title` | a subtitle line; `\|.pill` is a pill on the title row (§3.2.2) |

- Classes stick to the `|`: `|.warn db SG has no rule for the lambda SG`. `| .env file` (a space first) is the text ".env file".
- Animation markers end a line as anywhere (§3.8): `| NodePort @0-2`.
- Leading and trailing spaces are trimmed. A `|` alone is an empty line (a vertical gap of one line); it also separates two tables that follow each other.
- **Text or row**: a line that ends with `|` and holds at least two `|` (the opening one and the closing one) is a table row; any other line is text. The rule needs no context: a card may hold text, then a table, then text.
- Escapes: `\|` a literal `|` (needed only for a line that would otherwise end with `|`), `\*`, `` \` ``, `\==`, `\@`, `\\`.
- A line has no attributes.

#### 3.2.4 Emphasis inside lines *(v0.8)*

The text of lines and table cells follows Markdown's inline emphasis. Titles, tags and arrow labels are plain text.

| Effect | Writing | Output |
|---|---|---|
| bold | `**deletion protection**` | `strong` |
| italic | `*optional*` | `em` |
| monospace | `` `shop/api:2026.10.3` `` | `code` |
| mark | `==0.0.0.0/0==` | `mark`: the colour of the line's or cell's state class (`.ok`, `.warn`, `.error`), red otherwise |
| mark of a class | `==.ok passed==` | `mark.ok`: the colour of that class; any class works, styled in CSS (`mark.mine { color: … }`) |

Rules (CommonMark's, which every Markdown tool shares):
- a delimiter **opens** only if the character after it is not a space, and **closes** only if the character before it is not a space; a delimiter with no partner stays text: `cron(0 2 * * ? *)`, `/api/*`, `status == 200`, `retry if code==503` are drawn as written;
- `_` is **not** emphasis (identifiers such as `order_events` or `DATABASE_HOST` stay intact);
- emphasis does not nest across lines; inside a mark, bold and italic may nest (`==**critical**==`);
- the only trap is two delimiters on one line meant as text (`x==1 and y==2`): escape one, `x\==1`.

A line that starts with an IPv4/CIDR still renders that address in monospace without backticks (D-051).

#### 3.2.5 Tables *(v0.8)*

A table is written in a card as a **Markdown table** (MultiMarkdown's spans): consecutive lines that end with `|`. A card may hold lines, then a table, then lines; its title, category, tag, colour and arrows work as for any card.

```pug
card#sg(label="Security groups") Ingress
  | Group  | Inbound                    | Attached to   |
  |--------|:---------------------------|---------------|
  | alb    | 443, 80 from ==0.0.0.0/0== | load balancer |
  | api    | 8080 from alb              | ECS api tasks |
  | ^^     | 9090 from monitoring       | ECS api tasks |
  | worker | no inbound rule           ||
  |.error lambda | none                 | order_events  |
```

- **Row**: a line that starts and ends with `|` (at least two `|`), cells separated by `|`. Consecutive rows form one table; a text line or an empty line `|` ends it. The spaces around a cell are trimmed, so a table aligned by any Markdown formatter is valid.
- **Header**: a separator row (`|---|---|`) after the first row makes it the header. Colons set a column's alignment, as in Markdown: `:---` left (default), `:---:` centre, `---:` right. A separator anywhere else is ignored with the warning `bad-table`.
- **Cell class**: `|.error lambda |` gives the cell `.error` (the class sticks to the `|`; `| .env |` is the text ".env").
- **Column span**: a cell with nothing in it, not even a space (`||`), joins the cell on its left. An empty cell is written with a space: `| |`.
- **Row span**: a cell holding only `^^` joins the cell above it, in the same column.
- **Ragged rows**: a row with fewer cells is padded with empty cells; one with more widens the table. Warning `bad-table` for a `^^` in the first row, a `||` that starts a row, a separator with a different number of columns, or a table of a single row of a single cell between text lines (most likely a text line that ends with `|`: write `\|`).
- Columns are as wide as their widest cell; a table never wraps its text. Animation markers after the final `|` apply to the row: `| api | 8080 | @2`.
- Arrows end on the card, not on a row (ports per row, as in an entity-relationship diagram, are a separate feature, not proposed).

Output: `card > table > tr > th | td` (`colspan`, `rowspan`, the cell's classes), drawn the same in HTML, SVG and PNG.

#### 3.2.6 Category and tag *(v0.8)*

A card shows two pieces of information beside its name: its **category** (what it is: "RDS PostgreSQL 16.4", "ECS service (Fargate)") and its **tag** (a chip: an owner, a module, a port).

- The category is `label="…"`, or by default the type's `--label` (`.rds { --label: "RDS"; }`), as `icon=` overrides `--icon`. `label=""` shows none.
- The tag is `tag="…"`; it no longer replaces the category.
- Where each goes depends on the look (§4.5); in the default `band` look, the category is a small capitalised line above the name, inside the band, and the tag a chip on the right of the band.

#### 3.2.7 States: `.ok`, `.warn`, `.error` *(v0.8)*

Predefined classes for a finding, a check, a health state. Each has a mark (Lucide `circle-check`, `triangle-alert`, `circle-x`) and a colour (theme variables `--ok`, `--warn`, `--error`: green, amber, red; contrast 4.5:1 for text).

| On | Rendering |
|---|---|
| a card (`card#lambda.fn.warn`) | the mark before the name (in the icon's place: the theme sets `--icon`, so a card's own `icon=` wins) and a ring of the state's colour around the card (`outline` in the theme); the type's colour is kept |
| a line (`\|.warn db SG has no rule for the lambda SG`) | the mark before the line, the text in the state's colour |
| a cell (`\|.error lambda \|`) | the text in the state's colour on a light tint of it, no mark |
| a mark (`==.ok passed==`) | the marked text in the state's colour on a light tint of it |

Information is never carried by colour alone (§4.6): the mark carries it on cards and lines; in a cell or a mark the writer chooses the words.

#### 3.2.8 Legends and plain text *(v0.8)*

**`card.legend`** is a legend. Its title names it ("Flows", "Terraform modules"); each line `|.class text` shows a **sample** of the class it names, then the text:
- an arrow class (one used on an arrow, or styling `.edge`): a short arrow with that class's colour, stroke and heads;
- a card type (a class with `--c`): a small chip filled with the type's accent;
- a zone type (a class some zone carries and no card does): a small frame, tinted and edged like the zone;
- a class that sets an icon (`--icon`): that icon, in the class's accent;
- a state class (`.ok`, `.warn`, `.error`): its mark;
- a line without class: plain text.

A legend with no lines is **automatic**: one line per class used in the diagram that has a `--label` (arrow classes first, then card types, in order of first use), the text being the `--label`; arrow classes may therefore carry a `--label` too (`.edge.events { --label: "events / queues"; }`). A legend is a card: left in the drawing, it is placed like any card with no arrow; docked (§3.2.9), it never moves a card.

**`card.text`** is plain text: no frame, no background, no badge; its title, if any, in bold; its lines muted. A footnote of the diagram is `card.text.wide(at="s")`.

```pug
card#flows.legend(at="ne") Flows
  |.user user request
  |.events events / queues
card.text.wide(at="s")
  |.warn = finding worth reviewing
  | Omitted for clarity: route tables (12), IAM policy attachments.
```

#### 3.2.9 Docking: `at="s"`, `at="ne"`… *(v0.8)*

`at=` says where an element goes: coordinates pin it in the drawing (`at="120,40"`, §3.4); a **compass point** docks it *outside* the drawing, the way Java's `BorderLayout` and Graphviz's ports name sides: `n`, `s`, `e`, `w` for a side, `ne`, `nw`, `se`, `sw` for a corner.

```
┌──────────────────────────────────────┐
│ title                                │
├──────────────────────────────────────┤
│ n  (the whole width)                 │
├────┬────────────────────────────┬────┤
│ nw │                            │ ne │
│ w  │ the drawing                │ e  │
│ sw │                            │ se │
├────┴────────────────────────────┴────┤
│ s  (the whole width)                 │
└──────────────────────────────────────┘
```

- **Who**: a top-level `card`, `cards` or `zone`. Elsewhere, the warning `bad-at`, and the value is ignored.
- **Only what is docked moves**: the drawing is laid out and routed as if docked elements did not exist; they never move a card of it. Without any compass `at=`, the image is the same as before.
- **Bands** `n` and `s` span the whole width (side columns included); their elements go side by side in writing order, a new row when a row would pass one and a half times that width; a `.wide` element takes a row of its own.
- **Side columns** hold, beside the drawing, `nw` (or `ne`) at the top, then `w` (or `e`), and `sw` (or `se`) at the bottom, each one under the other in writing order; a column is as wide as its widest element. A legend at `at="ne"` thus stands at the top right, beside the drawing.
- **Sizes**: the middle row is as tall as the drawing or its tallest side column. Empty bands and columns take no room. Gap: the theme's `--dock-gap` (32 px). The diagram's title stays above everything.
- **Inside**: a docked zone is laid out on its own, as a small diagram with the diagram's `dir`; its arrows are routed inside it.
- **Arrows** between a docked element and anything outside it give the warning `docked-arrow` and are not drawn.
- **Modes**: docking works in every layout mode (graph, timeline, frieze, mind map): it happens around the drawing.
- **Editor**: dragging a docked element writes `at="x,y"`, which brings it back into the drawing.

### 3.3 `to` (arrow)

| Element | Meaning |
|---|---|
| `to(target)` | arrow from the parent card to the card `target`. Several targets possible: `to(a b)` creates two arrows |
| `.class` | arrow styles (`.wan`, `.dashed`, `.link`, `.both`…) |
| text | label |
| `via="x,y x,y"` | mandatory waypoints, in order, in the file's frame |
| `side="start-end"` | attachment side: `top` `bottom` `left` `right` (e.g. `left-top`). One half may stay empty: `side="-top"` **[Rust]** |
| `@…` at the end of the line | animation markers (§3.8) |
| `href="…"` (v0.6) | the arrow (line and label) is a link (§3.2.1) |
| `join="trunk"` **[Rust]** | **hyperedge**: the arrows of this `to(a b c …)` share one trunk out of the source card, then branch to their targets (organisation-chart style): `to(a b c join="trunk")` |
| `join="name"` *(preview, D-051)* | **trunk in**: the arrows to one card that carry the same name share one trunk into it, drawn once with a single head (`to(svc join="http")` written under two cards). A name no other arrow to that card carries gives the warning `bad-join` and is ignored. The label of such arrows names their common flow: it sits on the part they share, up to the card (D-054) |
| `constraint="false"` *(preview, D-045)* | the arrow is drawn and routed but plays **no part in the automatic layout** (like Graphviz's `constraint=false`): its target is not pushed a row further. For cross links (service discovery, monitoring) that would otherwise turn the diagram into a long chain |
| `#id` **[Rust]** | explicit identifier: `to#e1(core)` |

An arrow also automatically receives the **`from-<type>` and `to-<type>` classes** of its endpoints: `.edge.from-uplink { color: … }` colours all arrows leaving a `.uplink` card.

**Hyperedges.** `to(a b c join="trunk")` draws one trunk leaving the source card, which splits towards `a`, `b` and `c`. Each target still has its own arrow (own id `start->a`, own class, label, animation marker), but the arrows leave the **same attachment point** and are **not spread apart** from each other, so that their common part reads as a single line. Only arrows written in the same `to(…)` are joined. Any other name *(preview, D-051)* joins arrows **into** one card instead: they are routed from their target, like a fork out of it, so that they meet before the card and end on one line; the side a trunk leaves by, or its branches come in by, is left to it (the other arrows of those cards use the opposite side). An empty `join=""` is the warning `bad-join`.

Implicit identifier of an arrow: `start->end`. **[Rust]**: in case of a duplicate, `#2`, `#3`, and so on are appended (the prototype overwrites the first arrow).

### 3.4 Inheritance and default rules

- **Animation**: see §3.8 (`@` markers, step resolution, SVG output).
- **Positions**: a card with coordinates in `at` is fixed (a compass point docks it instead, §3.2.9). The others take the position computed by the engine. A zone without `at`/`size` = bounding box of its children + `--zone-pad` on the sides, `--zone-head` at the top.
- **Routing**: all arrows are routed by **the single router** (D-010), whether positions are automatic or pinned. An arrow goes around cards, and around the zones that contain neither its start nor its end, except through the corridor a `wrap` zone keeps between its rows (D-056). In a diagram drawn in steps, an arrow only goes around what is shown at the same time as it: a card, zone or zone title whose steps never meet the arrow's is no obstacle to it (the cards do not move from one step to the next; D-060). Routes are **centred in the free corridor** between the shapes that border them (D-035). *(In the prototype: ELK routes without `at`/`via`, then libavoid re-routing from the first `at`/`via`.)*
- **Writing order** (D-041): siblings on the same row keep their writing order, inside zones too (end of limit L3). An arrow written after its target no longer reverses the layout: only real cycles are broken (ELK `GREEDY_MODEL_ORDER`).
- **Tidy layers** *(preview, D-047)*, in graph mode:
  - rows inside a zone are as far apart as outside, except in a `wrap` grid;
  - a card is centred on its own children (at least two, on the next row, with no other parent), otherwise on the child in its column;
  - arrows leave in the drawing's direction and enter by the side facing it, unless another side is clearly shorter;
  - **forks**: the plain arrows of a card to two or more cards of the next row (no label, `side`, `via` or `join`, same classes) share one trunk that splits at one point, like `join="trunk"`;
  - **buses**: cards of one row that each fork to the same cards of the next row (every one to every one) share one bar half-way between the rows instead of crossing;
  - junctions of a trunk or a bus are sharp, with a dot; ports spread over a roomy side; two arrows never leave and arrive on one line.
- **Hand-drawn rules** *(preview, D-048)*, in graph mode:
  - **ranks from the sources**: siblings share a row; a leaf is no longer pushed to the last row;
  - **rhythm**: rows 48 px apart, more when an arrow label must fit between two rows; arrow labels do not take a row of their own; with `dir` right or left, a label too wide to sit on a short arrow goes over it *(D-049)*;
  - **centred on parents too**: a card fed only by two or more cards of one row (each of them feeding only it) sits in the middle of them; the children a card is the only parent of are shifted under it; a one-to-one link is straight, across zones if the zone frames stay clear of their neighbours;
  - **forks on a subset**: a card's plain arrows fork even when it also has a labelled arrow, which keeps its own port;
  - **back edges** (target before the source in the drawing's direction) loop around the row: out and in by the bottom with `dir="right"`, by the right with `dir="down"`; the loop runs 24 px beyond the frames of the zones holding either card *(D-049)*;
  - **notes**: a `.note` card whose only arrow is a `.link` does not shape the layout; it stands beside its card, above it with `dir` right or left, on its right with down or up, if the place is free;
  - **cross targets**: a card that only has `constraint="false"` arrows stands beside the drawing just before the cards that use it; their arrows enter it by the side facing them, and go around a card that stands in the way;
  - **cards with no arrow** at the top level are lined up in writing order (with `dir` right or left, under the rest);
  - **widths**: cards are as wide as their text (theme minimum 96 px); siblings (same container, same parents) share the widest width, chips included *(D-049)*;
  - a `.bare` zone keeps the room of its caption below its cards; its caption widens no card.
- **Arrow labels and links** *(preview, D-049)*: a label is a pill tinted with its arrow's colour (in the outputs it sits inside its arrow, so `.edge.ok { color: … }` reaches it); a `.link` is a dotted line, in the note's colour when it leaves a `.note`; arrow heads are 8 px.
- **Routing details** *(preview, D-049)*, in graph mode: a segment crossing the gap between two rows runs half-way through the free space (between what stands above it and the first card, zone frame or zone title below); an arrow that steps aside by at most 8 px is straightened by sliding one of its ends; arrows with `via=` keep their route; the arrows of a group draw their shared trunk and its dots once.
- **Zones of one row** (D-045): sibling zones placed side by side start on the same line (same top with `dir` down).
- **Zones above their targets** (D-043): a top-level zone alone on its row is centred, across the direction, on the nodes it is linked to.
- **Overlaps** (D-033): a card placed on another (`at=`) is never moved; the engine emits a warning that names both cards.

### 3.5 Blocking errors

- unknown arrow target;
- duplicate identifier;
- `to` outside a card;
- several attribute groups on a tag;
- mixing spaces / tabs **[Rust]**;
- more than one appearance marker on a node, malformed marker (`@0` alone, `@4-2`, unknown effect) **[Rust]**;
- malformed `at`, `size` or `via` (numbers expected) **[Rust]**;
- `href` with a scheme outside the whitelist, or empty (`unsafe-href`, v0.6).

Warnings (never block): `unknown-icon`, `icon-ignored` (v0.6); *v0.8* `bad-table` (§3.2.5), `bad-at` (a compass `at=` on an element that is not top-level, or an unknown compass point), `docked-arrow` (an arrow between a docked element and the rest, not drawn), `unknown-attribute` (an attribute the element does not take, among them the prototype's `engine`, `router` and `elk`, no longer read: §7).

### 3.6 Timeline mode *(post-v1 extension; prototyped in the POC)*

Enabled by `diagram(layout="timeline" …)`. The horizontal position of a card comes from **its date**, not from its links. Top-level zones become **lanes**.

```pug
diagram(layout="timeline" start="2026-10" end="2027-06" scale="month" today="2026-10-15")
  //- card outside a zone: implicit lane at the top
  card#lancement.jalon(date="2026-10-14") Specs delivered
  zone#moteur.lane Engine
    card#e0.spike(start="2026-10-16" end="2026-11-07") E0 · spikes
      to(valid)
  zone#livrables.lane Deliverables
    card#valid.decision(date="2026-11-09") Validation of the choices
```

| Element | Attribute | Meaning |
|---|---|---|
| `diagram` | `layout="timeline"` | enables the mode |
| | `start`, `end` | *v0.8* bounds of the axis, the diagram's own period (same words and same meaning as a card's: `end` included); by default, the extreme dates ± 7 days |
| | `scale` | `day` · `week` · `month` (default) · `quarter` · `year`: graduations and density (36 / 9 / 4 / 1.4 / 0.45 px per day) |
| | `width` | imposed width of the axis in px (replaces the density of `scale`) |
| | `today` | date of the "today" marker |
| `card` | `date` | **milestone**: the card's left edge is on the date, with a diamond on that edge and on the axis |
| | `start`, `end` | **period**: the card starts at `start`; a **bar** at the top of the card measures exactly the duration. The card is at least as wide as the duration, and wider if its text requires it |

Dates: `YYYY`, `YYYY-MM` or `YYYY-MM-DD`. **`end` is included, at the precision written** (the same rule in every mode, *v0.8*): `end="2027-03"` = up to 31 March included; `end="2027-03-15"` = up to 15 March included. A `start` reduced to the month = the 1st of the month.

Rules:
- a card must have `date` **or** `start` (`end` defaults to `start`);
- in a lane, cards that overlap are **stacked** on successive rows (first free row, in chronological order); *v0.8* two bars that overlap by less than 2 px, as drawn, touch and share a row;
- lanes follow the writing order; cards outside a zone form an implicit lane, at the top and without frame;
- **arrows** are dependencies. Default sides: `right-left` inside a lane; start `right` and free arrival towards another lane. Lanes are **not** obstacles;
- coordinates in `at=` have no effect in timeline mode (the position comes from the date), a compass point docks as anywhere (§3.2.9); `@` markers work as elsewhere (§3.8): the axis diamond carries the same `data-step` as its milestone.

Errors: card without `date` nor `start`, malformed date (both checked by the prototype); **[Rust]** `end` earlier than `start`, `at=` present (warning).

### 3.7 Chronological frieze mode *(post-v1 extension; prototyped in the POC)*

Enabled by `diagram(layout="frieze" …)`. **A single central axis**: eras are **bands** on the axis, events are cards **above or below**, linked to their date by a **stem**. Suited to long durations (decades, centuries, millennia) and to **BC dates**.

```pug
diagram(layout="frieze" start="-800" end="600" scale="century")
  card#republique.rep(start="-509" end="-27") Republic
  card#empire.emp(start="-27" end="476") Empire
  card#cesar.evt(date="-44-03-15") Death of Caesar
    | Ides of March
  card#chute.fin(date="476") Fall of the Western Empire
```

| Element | Attribute | Meaning |
|---|---|---|
| `diagram` | `layout="frieze"` | enables the mode |
| | `start`, `end` | *v0.8* bounds of the axis (years), the diagram's own period; by default, the extreme dates. `end` is included like any `end`: `end="2018"` runs to the end of 2018 |
| | `scale` | `year` · `decade` · `century` (default) · `millennium` (60 / 8 / 0.9 / 0.12 px per year); the graduation step adapts to keep ≥ 90 px between two labels |
| | `width` | imposed width of the axis in px |
| `card` | `date` | **event**: card centred on its date, stem down to the axis, dot on the axis |
| | `start`, `end` | **era**: band from `start` to `end` |

Dates: a **year** (negative = BC), optionally followed by `-MM` or `-MM-DD` (`-44-03-15`). **There is no year 0**: `-1` (1 BC) is followed by `1`. Positions are computed in "astronomical" years, so that the gap between 1 BC and 1 AD is one year.

**`end` is included, at the precision written**, as in the timeline (*v0.8*; it used to be a bound here). `Republic -509 → -27` then `Empire -27 → 476` share the year -27: less than 2 px at century scale, so the two bands touch and share a row (rule below).

Rules:
- **Eras**: stacked in rows under the axis, **longest first**. The main sequence therefore stays on the first row, and short episodes go below. Two bands that touch share a row; *v0.8* bands that overlap by less than 2 px, as drawn, touch. A name too long for its band goes to the right of it, and the stacking reserves its room.
- **Events**: in chronological order, each goes to the side (up or down) that asks for the **lowest level**, alternating on a tie. Several levels per side if close cards overlap. The stem stays on the exact date.
- **Axis**: graduations in years ("500 BC", "1", "500"). At century scale, **centuries in Roman numerals** ("Iᵉʳ s. av.", "IIᵉ s." in the French labels) are available but hidden by the default theme, which keeps one row of labels *(D-048; `.fr-century { display: block; }` shows them)*.
- **Dates on cards** *(preview, D-048)*: an event shows its date above its title (`15 Mar 44 BC`), followed by its category (`label=` or the type's `--label`) if any. Theme switch: `--event-date: on`.
- **Packing** *(preview, D-048)*: an event card may hang by an edge of its stem (14 px from the edge) instead of being centred on it, when that keeps it on a lower level. Theme switch: `--event-anchor: edge`.
- **Arrows** (rare, cause-and-effect links): routed by the router; no zone obstacle.
- **Zones** *(preview, D-045)*: a top-level zone **frames its events** (cards and stems) with its title on top. All its events go to the same side, above the axis unless a zone already above overlaps its dates; events outside the zone but within its dates go to the other side. A zone inside a zone is an error. Parallel "registers" (Rome and China on one frieze, each in its own band) are not done.
- `@` markers: as elsewhere (§3.8); the stem and the dot carry the same `data-step` as their event.

Errors: card without `date` nor `start`, malformed date, **year 0**, zone inside a zone (the prototype refused any zone); **[Rust]** `end` earlier than `start`.

### 3.9 Mindmap mode *(post-v1 extension; D-034)*

Enabled by `diagram(layout="mindmap" …)`. A central card, branches going **right and left**, sub-branches moving away from the centre. The tree comes from the arrows: the targets of a card's `to(…)` are its children, in writing order.

```pug
diagram(layout="mindmap")
  card#skemo.root skemo
    to(language engine router)
  card#language Language
    to(parser styles)
  card#parser Parser
  card#styles CSS styles
  card#engine Engine
  card#router Router
```

| Element | Attribute | Meaning |
|---|---|---|
| `diagram` | `layout="mindmap"` | enables the mode |
| | `root` | id of the central card; by default the first card that no arrow targets, otherwise the first card |

Rules:
- the **children of the root** are distributed between the right and the left side, balancing the height of each side (the next one goes to the lower side, right first); on each side the writing order reads from top to bottom;
- a node is vertically **centred on the block of its children**; siblings are stacked; levels are 72 px apart, siblings 20 px;
- the arrows of the tree leave through the side facing **outwards** and arrive through the side facing the centre; those of one parent on one side **share a trunk** (like `join="trunk"`, §3.3). Other arrows (a card already reached, cross-links) are ordinary arrows;
- a card that cannot be reached from the root is placed below the map, with a warning;
- sizes, classes, styles, themes and animation markers work as everywhere else (§3.8): `@` markers on cards and arrows animate the branches in.

Errors: `root` that is not the id of a card; **zones** (not handled: the structure comes from the arrows). Warning: coordinates in `at=` (no effect: the position comes from the structure); a compass point docks as anywhere (§3.2.9).

### 3.8 Animation: `@` markers

#### Syntax

Markers are placed **at the end of the line**, preceded by at least one space, on any node: `card`, `zone`, a line `|`, a table row, `to(…)`.

```
marker := "@" start ["-" end] [":" effect]        (several markers possible, separated by spaces)
start, end := number > 0, integer or decimal (2, 2.5); start may be 0 in a range (@0-3)
effect := fade | up | zoom | draw | focus
```

| Writing | Meaning |
|---|---|
| `@2` | appears at step 2 (default effect: `fade`, and `draw` for a link) |
| `@2:up` | appears at step 2 with the `up` effect |
| `@2-4` | visible from step 2 to step 4 **included**, then disappears |
| `@0-2` *(preview, D-045)* | visible **from the start** to step 2 included, then disappears (a line replaced by another: `| NodePort @0-2` then `| ClusterIP @3`) |

**Lines that replace each other** *(preview, D-045)*: consecutive lines of a card whose ranges, as written, never overlap share **one place**: `| NodePort @0-2` then `| ClusterIP @3` is one line that changes at step 3, and the card is as tall as one line (as wide as the wider). The card's size does not change from one step to the next (stability).
| `@4:focus` | **highlighted** at step 4; does not handle appearance |
| `@4-6:focus` | highlighted from 4 to 6 |

```pug
card#seg.segment(tag="DHCP") seg-k8s-01   @1:up @4-6:focus
  | 10.1.20.0/28                    @2
  to(t0)
card#ports.note Segment ports          @2.5:up
  to(seg).link
```

Writing rules:
- **at most one appearance marker** per node (otherwise an error), and as many `:focus` as wanted;
- only the **final** tokens of the line are read as markers: they are read from the end, stopping at the first token that is not one. `meet @3 tomorrow` therefore stays text;
- markers are **removed** from the displayed text (title, line, label), as well as the spaces that precede them;
- `\@` writes a literal `@`, which is never read as a marker (`see \@2`);
- an `@` not followed by a number (`admin@example.com`) is not a marker and needs no escaping;
- the `draw` effect on a node that is not a link is replaced by `fade`, with a warning;
- steps are **ordered values**, not consecutive numbers: `@2.5` slots between 2 and 3 without renumbering anything.

It is valid Pug: a standard Pug tool would display the markers as text.

Complete test set: `fixtures/animation.pug` (expected result: `engine/fixtures/animation.expected.json`).

#### Step resolution (done by the engine)

Notation: `appearance(n)` = effective appearance step, `end(n)` = last visibility step (∞ by default).

1. **Node without appearance marker**: visible from the start (step 0), or at the same time as its parent.
2. **Child** (card in a zone, line in a card, zone in a zone): `appearance = max(appearance(parent), its own)` and `end = min(end(parent), its own)`. A child does not appear before its parent, and does not outlive it.
3. **`to()` link without marker**: `appearance = max(appearance(start), appearance(end))`, `end = min(end(start), end(end))`, `draw` effect. A link with a marker follows the same bounds rule (rules 2 and 4).
4. **Marker earlier than what inheritance imposes** (or end later): **warning** on stderr, and the inherited value is kept.
5. **Focus**: it only has an effect while the node is visible. A focus range outside the visibility is reported by a warning.
6. **Steps of the diagram** = the sorted set of written values (starts, range ends, focus), preceded by the initial state 0 if it contains at least one visible element. A node `@a-b` is visible for every step `s` such that `a ≤ s ≤ b`, and hidden from the first step defined **after** `b`. If none exists, the disappearance is never visible: **warning** ("`@1-3`: no step after 3").
7. **Still image** *(D-069)*: a PNG or WebP rendered without `--step` shows the **last step** (what `--step` with that step shows). The SVG keeps every element, marked (§4.4); HTML plays the steps.

## 4. Style (CSS)

The default theme is overridden by a custom CSS, with the normal cascade.

### 4.1 Variables read by the engine

**Principle**: a type only defines its **accent** (`--c`) and its **label** (`--label`). Tinted backgrounds, borders and inks are **derived** from them by `color-mix()` with the theme's surface colours. A single type therefore works in light, dark and print, without redefining anything. A custom CSS must not hard-code background colours: it uses the variables (`var(--surface)`, `var(--ink)`…).

| Variable | Where | Role |
|---|---|---|
| `--c` | type (card or zone) | accent colour; a zone passes it on to the untyped cards it contains |
| `--label` | card type, arrow class | *v0.8* the type's **category** (`"T0"`, `"RDS"`), shown as §3.2.6 says and overridden by `label=`; also the name of a card type or an arrow class in an automatic legend (§3.2.8); absent = no category |
| `--look` | card type or `.stage` | `band` (default), `header`, `classic`, `eyebrow` (§4.5); `plain` = secondary cards |
| `--icon` (v0.6) | type | default icon name of the type (`"database"`); `icon=` overrides it |
| `--icon-size`, `--icon-color`, `--icon-gap` (v0.6) | type | icon tuning; defaults per look (§3.2.1) |
| `--link-mark` (v0.6) | `:root` or type | `none` hides the link glyph and frees its space |
| `--on-c` | type or theme | colour of the text placed on the accent (default: white) |
| `--accent-fill` | theme | fill of accent areas (default: `--c`; darkened in print theme) |
| `--z-bg`, `--z-border`, `--z-ink` | zone type | optional override of the derived tints |
| `--zone-pad`, `--zone-head` | `:root` | zone margins, **used for the layout computation** |
| `--ok`, `--warn`, `--error` *(v0.8)* | `.stage` (per theme) | colours of the states (§3.2.7) and of marks |
| `--dock-gap` *(v0.8)* | `.stage` | gap between the drawing and docked elements, and between docked elements (32 px, §3.2.9) |
| `--bg`, `--surface`, `--ink`, `--ink-2`, `--muted`, `--line`, `--card-border`, `--shadow`, `--grid`, `--grid-major` | `.stage` (per theme) | base colours, **redefined by each theme** |
| `--font`, `--mono`, `--radius` | `.stage` | typography and corner radii. Inter and JetBrains Mono are embedded; another family is drawn only if its font files are handed to the engine (`Options::fonts`, CLI `--font`), otherwise Inter, with the warning that names it |
| `--font-scale` *(preview, D-041)* | `.stage` | multiplies every text size of the theme (`1.4` for a projected slide); CLI `--font-scale 1.4`. The theme writes `calc(13px * var(--font-scale, 1))`, read by the engine and the browser alike |
| `--event-date`, `--event-anchor` *(preview, D-048)* | `.stage` | frieze: `on` writes an event's date above its title; `edge` lets an event card hang by an edge of its stem |
| `--tl-gap-x`, `--tl-gap-y` *(preview, D-048)* | `.stage` | timeline: gap between periods of one row and between rows (prototype: 12 px and 22 px) |
| `--tl-deps` *(preview, D-048)* | `.stage` | timeline: `drop` draws dependencies straight down (or up) from the end of a bar, then into the target |
| `--curve` *(preview, D-048)* | `.edge .line` | `on`: one smooth curve from end to end (mind map) |
| `--shape` on `.edge .head` *(preview, D-048)* | arrow head | `dot`: a dot where the arrow ends instead of a head (mind map leaves) |

### 4.2 Rendered structure (available selectors)

**[Rust]** In SVG and HTML output, identifiers are written as `data-id` (never `id`, see §4.4). The prototype still uses `id`. The selectors below write `#id` for convenience; in a custom CSS, write `[data-id="fw"]`.

```
.stage                                   container
  zone#id.classes > .title (+ .tag)      zone (.tag: `tag=`, D-050); a theme that sets the title
                                           below (`bottom`, no `top`) gets its room below (D-051)
  .diagram-title > .text, .sub, li.pill  title of the diagram (D-050), its subtitle lines and pills (v0.8)
  .stage.layout-<mode>                    timeline, frieze, mindmap (D-048): the value as written
  card#id.classes[.no-tag][.no-type][.no-body] > .title, ul > li[.classes] (> span.mono)   .no-type = no --c (D-048);
                                           each source line `|` is an output `li` (v0.8: the HTML keeps a list)
  card > .label                           the category (v0.8, §3.2.6)
  li > strong, em, code, mark[.class]     emphasis inside lines and cells (v0.8, §3.2.4)
  card > table > tr > th, td[.classes]     table (v0.8, §3.2.5)
  card.ok, .warn, .error > .state-mark     state mark (v0.8, §3.2.7); li.warn > .state-mark on a line
  card.legend li > .sample                 a legend's sample (v0.8, §3.2.8)
  zone > .sub                              a zone's description lines (v0.8)
  .dock.dock-<point>                       a docked element's cell, e.g. .dock-s, .dock-ne (v0.8, §3.2.9)
  card > .icon (svg, v0.6)                 icon; a.href wraps a linked card, zone or arrow; .link-mark = glyph
  card.look-header > .head > span.type + span.tag ; > .title ; > ul   (header style)
  svg.edges > g.edge.classes.from-<type>.to-<type>[data-id]    arrow
                > path.line              stroke
                > path.head              arrival head
                > path.tail              start head (hidden except .both)
                > circle.origin          start dot (hidden except .dot)
  .edge-label                            label; inside its .edge (HTML: a display:contents wrapper with the arrow's classes), D-049

  timeline mode:
  card.milestone (::after = diamond)     milestone
  card.phase (::after = bar, width --bar-w)   period
  .tl-axis, .tl-tick[.major], .tl-grid[.major]    axis, graduations, vertical lines
  .tl-mark[data-for]                     diamond of a milestone on the axis (hidden by the default theme, D-048)

  mindmap mode (D-048):
  card.mm-root, card.mm-branch, card.mm-leaf   root, its children, every card below them
  .mm-b0 … .mm-b6                        colour index of the branch (cards; arrows: .to-mm-b2)
  .tl-today, .tl-today-label             "today" marker

  frieze mode:
  card.event                             event (on its date; ::before = its date, D-048)
  card.era[.outside]                     era (band); .outside = name to the right of the band
  .fr-axis, .fr-tick, .fr-tick-label     axis and graduations
  .fr-century                            centuries in Roman numerals
  .fr-stem[data-for], .fr-dot[data-for]  stem and dot of an event
```

Heads use `currentColor`: an arrow's colour is set with `color`.

### 4.3 Provided arrow styles (combinable)

`.dashed` `.dotted` `.thick` `.flow` (animated dashes) `.both` (double head) `.nohead` (no head) `.dot` (dot at the start) `.link` (attachment: dotted without head, e.g. note → card), plus colour examples `.wan` and `.secure`.

### 4.4 Animation: what the engine writes

The engine **marks** the elements; it animates nothing itself in SVG.

**SVG output**:
- each node is a `<g>` that contains all its visuals; its children are **nested** in it (zone ⊃ cards ⊃ lines);
- links are in a **separate group, above** the nodes.

| Attribute | Where | Value |
|---|---|---|
| `data-id` | every node that has an `#id` | the Pug id. **No `id` attribute**, to avoid collisions between several diagrams on one page (a custom CSS targets `[data-id="fw"]`) |
| `data-step` | animated node or link | effective step, **as written** (decimals kept, no renumbering) |
| `data-fx` | same | `fade`, `up`, `zoom` or `draw` (default: `fade`; `draw` for links) |
| `data-until` | if `@N-M` | `M` |
| `data-focus` | if `:focus` | space-separated list: `4` or `4-6` (e.g. `data-focus="4 7-8"`) |
| `data-from`, `data-to` | link | ids of the endpoints |

Rules:
- a link has a `<path … pathLength="1">` for its stroke (`draw` effect: `stroke-dasharray: 1; stroke-dashoffset: 1 → 0`);
- a **non-animated element has no `data-step` attribute**. An element is "animated" if its effective step is **greater than that of the group containing it** (a child that appears with its parent has none), and every link of step > 0;
- an element that only has a `data-focus` (without its own appearance) carries no `data-step`;
- `data-until` follows the same logic: it is only written if the effective end is **earlier than that of the group containing it** (so almost always for a link that inherits the end of an endpoint);
- **decorative elements attached** to a node outside its group (axis diamond of a timeline, stem and dot of a frieze) carry that node's animation attributes and `data-for="<id>"`;
- the engine adds **no animation-related class or style, and no script** to the SVG. Type classes (`card t1`, `edge wan from-t0`) remain, since they belong to style, not animation.

**Root of the SVG** *(D-041)*: `data-states` = the ordered steps as written (`["1","2.5"]`), as on the HTML `.stage`, so that an integration (reveal.js fragments) can map `2.5` to its own index; absent on a static diagram. Fonts: `font-family` lists a generic fallback; `--embed-fonts` (CLI) embeds the fonts used (about 0.8 MB per weight).

**Embedded source** *(D-042)*: every rendered file carries its source and stylesheets (SVG: `<metadata><skemo-source>`; HTML: `<script type="application/json" id="skemo-source">`; PNG, GIF, WebP: a metadata chunk), so that it can be reopened (`skemo extract`). `--no-source` leaves it out.

**HTML output**: the same `data-*` attributes on the same elements; the standalone HTML also embeds its small step reader and the effects' CSS, which is its reason for being.

**PNG and GIF**: the engine **computes** the states itself (§3.8, rule 6). See the specification of the engine and the CLI: `--animated` for PNG (one file per state, without effects), `.gif` (effects included).

### 4.5 Card looks (`--look`): `band` (default), `header`, `classic`, `eyebrow`


A card **without lines** has its title **centred** (in every look; in the band, between the left padding and the tag). The engine gives it the class `no-body`, which the theme's rules use *(D-047)*. Stacked in a column (a `wrap="1"` zone, a stack) with cards that have lines, it keeps the column's alignment and gets no `no-body` *(D-056)*.

Four looks, chosen **per type** (`.segment { --look: header; }`) or **for the whole diagram** (`diagram(look="header")`); the value is inherited from the diagram, so a type can override it:

| `--look` | Card |
|---|---|
| `band` (default) | a coloured **band** that carries the **name** (left), *v0.8* the **category** as a small capitalised line above it, and the `tag` as a **chip** (right); the lines below, on a tinted background with an accent border |
| `header` | the older header style, described below: band with the category and the tag chip, name centred underneath |
| `classic` | the older base style: accent line on top, the tag as a badge in front of the name; *v0.8* the category as a small line above the name |
| `eyebrow` *(preview, D-048)* | a white card; the category is a small coloured label on its own line above a left-aligned title; *v0.8* the tag a chip at the right end of that line; no band |

A card **without a type** (no `--c`) is drawn light in every look: surface background, ink title, hairline border (class `no-type`, D-048). Colour is left to the types that mean something.

**Timeline look** *(D-048)*: a period is a thin bar under its label (the label may run past the bar's end), a milestone is one diamond with its name (the type's badge and the axis diamond are hidden), dependencies are thin dashed lines. **Mind map look** *(D-048)*: a dark root pill, tinted branch pills, plain-text leaves, curved branches without heads, one colour per branch.

`.note`, `.chip`, `.info`, `.legend` and `.text` always stay in plain style, and so do the cards of the timeline and frieze modes. A long name in the band is not wrapped: the card widens to hold it.

#### Header look (`--look: header`)

A card in header style has:
- a coloured **band** with its category (`label=` or `--label`) on the left, and its `tag` as a **chip** on the right;
- below, its **name centred and large**, then its lines centred;
- a background tinted with the accent and a border in the accent.

It is enabled **per type** (`.segment { --c: #4E9A3E; --label: "Segment"; --look: header; }`) or **for the whole diagram** (`diagram(look="header")`). Cards without a category or a `tag`, and `.note`, `.chip`, `.info`, `.legend`, `.text`, stay in plain style.

```pug
diagram(dir="right" look="header")
  card#seg.segment(tag="DHCP") seg-k8s-01
    | 10.1.20.0/28
    to(t0)
  card#ports.note Segment ports
    | db-node-a
    to(seg).link
```

Complete example: `fixtures/design.pug` + `fixtures/design.css`; classic network in header style: `fixtures/reseau-entete.pug`.

### 4.6 Themes and contrast

| Theme | Use | Particularities |
|---|---|---|
| `light` | default | secondary texts, arrows and borders with reinforced contrast (text ≥ 4.5:1) |
| `dark` | dark screens | only the base colours change; tints are recomputed from `--c` |
| `auto` | HTML | follows `prefers-color-scheme`; in PNG or SVG, rendered as `light` (with a warning) **[Rust]** |
| `print` | printing, **including black and white** | white background, black ink, no shadow; accent fills **darkened** (white text ≥ 4.5:1 even converted to grey); darker and thicker strokes and borders. Applied **automatically** when printing the HTML (`@media print`), whatever the chosen theme |

Rules:
- **information is never carried by colour alone**: types have a label (badge or band), and arrows whose direction matters have a stroke style (`.dashed`, `.dotted`, `.thick`, `.link`);
- in `print` theme, **dimensions may change slightly** (thicker borders): the layout is computed for the requested theme.

## 5. Complete examples

Timeline: `fixtures/timeline.pug` + `fixtures/timeline.css`. Frieze: `fixtures/frise.pug` + `fixtures/frise.css`. See also `fixtures/reseau-manuel-via.pug`: pinned positions, `via`, `side`, arrow styles, steps. Non-network example with a custom theme: `fixtures/process.pug` + `fixtures/process.css`. *(The fixtures are the prototype's reference files and keep their French labels.)*

## 6. File extension

**`.skemo`** (decision D-009, 2026-10-16). The content remains valid Pug: an editor can associate it with Pug highlighting. The test sets inherited from the prototype (`fixtures/*.pug`) keep their extension, the oracle never being modified; the tools accept any extension.

## 7. Read for compatibility, not part of the language *(v0.8)*

The prototype's files (`fixtures/*.pug`, the oracle, which is never modified) and files written before v0.8 use spellings that are no longer part of the language. The parser still reads them, without documenting them to users; `skemo fmt` rewrites them in v0.8 form.

| Old spelling | Read as | Note |
|---|---|---|
| `li text`, `li.class text` | `\| text`, `\|.class text` | under `title`, a plain `li` was a pill: read as `\|.pill` |
| `step=N`, `fx="up"` | `@N:up` | both notations on one node: error |
| `layout="frise"` | `layout="frieze"` | |
| `diagram(from=… to=…)` (timeline, frieze) | `diagram(start=… end=…)` | `to` was also the arrow's tag name |

A frieze written before v0.8 read its `end` as a bound: its eras now run one unit longer (one year at year precision), which the 2 px rule hides at the usual scales.

No longer read (warning `unknown-attribute`, value ignored): `engine`, `router` (the prototype's layout engines and routers; there is one of each), `elk` (raw options handed to the layout library: an untrusted file could ask for extreme values, and they bypassed the language's rules). The oracle's parity test sets the prototype's layout options through the engine's API, which no file can reach.
