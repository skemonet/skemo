---
name: skemo
description: Create diagrams as short `.skemo` text files and render them with the `skemo` command-line tool to PNG, SVG, WebP, GIF or step-by-step HTML. Use whenever the user asks for a diagram, schema, architecture or network drawing, flowchart, process, organisation chart, mind map, timeline, roadmap, Gantt-like plan or chronological frieze, or wants an existing `.skemo` file changed. Write skemo text instead of drawing SVG, HTML/CSS or draw.io XML by hand - layout, routing and rendering are done by skemo, so the diagram is a few lines to write and to edit (measured on a whole architecture-diagram task: about a third fewer tokens and four times faster than drawing it alone).
---

# skemo: diagrams as text

skemo turns a few lines of indented text into a laid-out, styled diagram. You write **structure only**
(cards, zones, arrows); the `skemo` binary computes positions, routes the arrows around obstacles and
renders the picture on the local CPU. Never compute coordinates yourself, never write the SVG yourself.

Full syntax reference: [references/format.md](references/format.md). Read it when you need a mode
(timeline, frieze, mindmap), animation markers, icons, links or the CSS variables in detail.

## 1. Check the tool

```bash
skemo --version
```

If it is missing, tell the user and offer the installer (Linux, macOS):
`curl -fsSL https://raw.githubusercontent.com/skemonet/skemo/main/cli/install.sh | sh` (installs into
`~/.local/bin`). Do not fall back to hand-drawn SVG without asking.

## 2. Workflow

1. **Pick the kind**: a graph of boxes and arrows (default), or `layout="timeline"`, `"frieze"`
   (chronological frieze), `"mindmap"`.
2. **Write `name.skemo`** (and `name.css` only if you need custom types or colours).
3. **Check**: `skemo check name.skemo`. Exit code 1 = errors, printed with line, column and a hint
   (`unknown-target`, `duplicate-id`…). Fix the text and check again.
4. **Render**: `skemo name.skemo` writes `name.png` next to the source. Pick another format with `-o`:
   `-o name.svg`, `-o name.webp`, `-o name.html` (interactive, steps), `-o name.gif`. Add `--css name.css`
   when there is a stylesheet, `--theme dark|print` when asked.
5. **Look once** at the PNG if you can view images, mainly to judge readability. Do not loop on
   screenshots: layout is automatic and overlaps cannot happen unless you pinned cards with `at=`.
6. **Edit by changing lines**, never by rewriting the file: adding a card or renaming a label is a
   one-line change, and skemo keeps the rest of the picture stable.

Warnings (unknown icon, unreachable mindmap card…) do not block; read them and fix what they name.
`--deny-warnings` makes them fatal, which is useful in CI.

## 3. The language in one example

A `.skemo` file is a subset of Pug: a few tags (`diagram`, `title`, `card`, `cards`, `zone`, `stack`, `to`)
and lines written `| text` under a card or a zone. The text of lines follows Markdown.

```pug
//- comment: not rendered
diagram(dir="right" lang="en")

  card#user.actor Customer
    to(web) HTTPS

  zone#cloud.cloud Production
    card#web.service(tag="nginx") Web front
      | 2 instances
      to(api)
    card#api.service API
      | REST + webhooks
      to(db)
      to(queue).dashed events
    card#db.store Orders DB
      | PostgreSQL **16**
    card#queue.store Job queue
```

```css
/* name.css: a type only sets its accent colour and its badge text */
.actor   { --c: #7048E8; --label: "USER"; }
.service { --c: #3E6FC4; --label: "SERVICE"; }
.store   { --c: #2F9E44; --label: "DATA"; }
.cloud   { --c: #8A93A3; }
```

| Write | Meaning |
|---|---|
| `card#id.type Title` | a card; `#id` unique, `.type` = CSS class (several allowed), text = title |
| `cards#id.type(count="4") Title` | (pre-release) a card drawn as a pile, for several instances of one thing; `count` adds a `×4` badge |
| `\| text` (under a card) | a content line; an IPv4/CIDR at its start is set in monospace; under a zone, a description line under its title |
| `zone#id.type Title` | a group; cards and zones indented below are inside it |
| `to(target).style Label` | an arrow **written inside its source card**, to the card `target` |
| `to(a b c)` | one arrow per target; to cards of the next row they fork from one trunk by themselves (pre-release); `join="trunk"` forces a trunk |
| `to(a constraint="false")` | (pre-release) an arrow that does not shape the layout (cross links, service discovery) |
| `diagram(dir="down")` | first line, optional: `dir` = `down` (default) `right` `up` `left` |
| `tag="Owner"` | text of the card's chip (an owner, a module, a port) |
| `label="RDS PostgreSQL 16"` | (pre-release) the card's category, above its name; default: the type's `--label` |
| `//- text` | comment |

Arrow styles, combinable: `.dashed` `.dotted` `.thick` `.flow` (animated) `.both` (two heads)
`.nohead` `.dot` (dot at start) `.link` (dotted, no head: annotation). Predefined card classes:
`.note` (dashed annotation), `.chip` (pill), `.info` (small box); zone class `.bare` (no frame,
caption below). Default theme types: `.t0` to `.t3`, `.cloud`, `.dmz`, `.lan`.

## 4. Rules that keep diagrams good and short

- **No coordinates.** Do not write `at=`, `via=` or `side=` unless the user asks to pin or adjust
  something. The automatic layout is better than guessed numbers, and guessed numbers cause overlaps.
- **Direction**: `dir="right"` for pipelines and processes, `down` (default) for hierarchies and networks.
- **Ids** are short, unique, lowercase (`api`, `db-main`): they are only used as arrow targets.
- **One attribute group per tag**, before the classes or right after the id:
  `to(core side="right-top").dashed` - never `to(core).dashed(side="right-top")`.
- **Indent with 2 spaces**; never mix tabs and spaces. A card only contains lines `|` and `to`; a zone
  contains lines, then cards and zones.
- **Short titles** (under ~30 characters): a long title widens the card, a long line wraps. Put details in
  `|` lines, at most 3 to 4 per card.
- **Types are classes styled in CSS**: set only `--c` (accent) and `--label` (badge). Never hard-code
  backgrounds or text colours: tints are derived from `--c`, so light, dark and print themes keep working.
  Colour is never the only carrier of meaning: give types a `--label` and important arrows a style.
- **Arrow labels** are short (1 to 3 words). A label on `to(a b)` is repeated on each arrow.
- **Pin the label language** with `diagram(lang="en")` in timelines and friezes (months, centuries).
- **Special characters**: `@2` at the end of a line is an animation marker; write `\@` for a literal `@`
  followed by a digit. `name@example.com` needs no escaping. In lines, `**`, `*`, backticks and `==` are
  Markdown emphasis when they wrap a word (`cron(0 2 * * ? *)` stays text); escape with `\*`. A line that
  ends with `|` is a table row: write `\|` for a literal final bar.
- Several arrows to the same target from one card are fine; cycles are fine.
- **Writing order is kept**: cards of one row appear in the order written, inside zones too.
- **Slides** (pre-release): add `diagram(fit="16:9")` (`8:9` for half a slide, or the box in px:
  `fit="1280x720"`). If the render warns that a setting would show the text larger, run
  `skemo fit f.skemo --write` instead of rewriting the diagram: it applies the `dir` or `wrap` setting
  that gives the largest text once fitted (not merely the closest proportions), never one that makes the
  arrows harder to follow. `zone#ns(wrap="3")` puts at most 3 children per row; a zone of four cards or
  more with no arrow (an inventory) is already laid as a grid. For a projected
  slide, render with `--font-scale 1.4` rather than overriding font sizes in CSS.
- **Fonts**: Inter and JetBrains Mono are built in. Another font needs its files: name the family in CSS
  (`.stage { --font: 'Source Serif 4', serif; }`) and render with `--font file.ttf` (one per weight);
  without them skemo warns and uses Inter.

## 5. Other kinds of diagram

**Timeline** (roadmap, plan): position comes from dates; top-level zones are lanes.

```pug
diagram(layout="timeline" start="2027-01" end="2027-06" scale="month" lang="en")
  card#kick.milestone(date="2027-01-11") Kick-off
  zone#team.lane Team
    card#build.work(start="2027-01-15" end="2027-03") Build
      to(ship)
    card#ship.work(start="2027-04" end="2027-05-15") Pilot
```

**Mind map**: the tree comes from the arrows, the root is the first card nobody targets.

```pug
diagram(layout="mindmap")
  card#topic Product launch
    to(market build)
  card#market Marketing
    to(site press)
  card#site Website
  card#press Press kit
  card#build Engineering
```

**Frieze** (`layout="frieze"`, history, BC dates): `card(date="-44-03-15")` for events,
`card(start="-27" end="476")` for eras. See the reference.

## 6. Richer cards, tables, legends and panels (pre-release)

```pug
diagram(dir="right" lang="en")
  title Shop platform
    | Production · eu-west-3 · 106 resources
    |.pill prod
  zone#vpc.net VPC 10.20.0.0/16
    | 3 availability zones
    card#api.service(label="ECS service" tag="module.app") api
      | `python3.12` · autoscaling **2–12**
      to(db).data 5432
    card#db.store.warn(label="RDS PostgreSQL 16") orders
      |.warn single-AZ replica
      | Multi-AZ · ==no deletion protection==
  zone#sec(at="s") Security
    card#sg(label="Security groups") Ingress
      | Group | Inbound              |
      |-------|----------------------|
      | alb   | 443 from ==0.0.0.0/0== |
      | api   | 8080 from alb        |
  card#key.legend(at="ne") Flows
    |.data data access
  card.text.wide(at="s")
    | Omitted for clarity: route tables, IAM attachments.
```

```css
.service { --c: #3E6FC4; }
.store   { --c: #2F9E44; }
.net     { --c: #0F766E; }
.edge.data { color: #0F766E; --label: "data access"; }
```

- **Category and chip**: `label=` (or the type's `--label`) is a small line above the name; `tag=` is the chip.
- **Emphasis** in lines and cells: `**bold**`, `*italic*`, `` `mono` ``, `==mark==` (red, or the line's state
  colour), `==.ok passed==` (a class's colour).
- **States**: `.ok`, `.warn`, `.error` on a card (a ring and a mark), a line (`|.warn …`) or a cell (`|.error x |`).
- **Tables**: Markdown rows `| a | b |`; `|---|` after the first row makes a header, `:-:` centres a column;
  `||` joins the cell on the left, `^^` the cell above; `|.error x |` gives a cell a class.
- **Legend**: `card.legend`; each `|.class text` shows a sample (an arrow for an arrow class, a chip for a card
  type, a small frame for a zone type, the icon of a class that sets `--icon`); with no lines it lists the
  classes that have a `--label`. **Plain text**: `card.text`.
- **Docking**: `at="s"` (or `n`, `e`, `w`, `ne`, `nw`, `se`, `sw`) on a top-level card or zone puts it around the
  drawing without moving anything in it: panels below (`at="s"`, side by side), a legend at the top right
  (`at="ne"`), a footnote `card.text.wide(at="s")`. Arrows between a docked element and the rest are not drawn.
- **Subtitle**: `|` lines under `title`; `|.pill` lines are pills beside it. A zone's `|` lines are a
  description under its title.

## 7. Steps (presentations)

Add markers at the end of a line: `@1`, `@2:up` (effect `fade` `up` `zoom` `draw` `focus`), `@2-4`
(visible from 2 to 4), `@0-2` (from the start to 2), `@3:focus` (highlight). A chip can appear on its own: `tag="8080 @1"`. The SVG
root lists the steps in `data-states` (map `2.5` to a whole index for reveal.js). Render `-o talk.html` (arrow keys step through),
`-o talk.gif`, or `--step 2 -o state2.png` for one state; a still PNG or WebP shows the last step.

```pug
diagram(dir="right")
  card#a Idea                @1
    to(b)
  card#b Prototype           @2:up
    to(c)
  card#c Product             @3:up @4:focus
```

## 8. Icons and links (pre-release)

`card#db(icon="database") Orders` draws a built-in Lucide icon; `skemo icons <word>` lists names.
A file works too (pre-release): `icon="img/logo.svg"`, next to the source; never invent a logo file.
`href="https://…"` makes a card, zone or arrow clickable in SVG and HTML. Only `http:`, `https:`,
`mailto:`, relative paths and `#fragments` are accepted.

Images rendered by skemo carry their source and stylesheets (like draw.io): when the user gives a
PNG/SVG/WebP made by skemo, run `skemo extract image.png` to get the `.skemo` back, edit it, and render
again; `skemo render image.png -o out.svg` also works directly. `--no-source` leaves the source out.

## 9. What to hand back

Give the user the `.skemo` file (and `.css`), the rendered file path, and the command to re-render.
If they will keep editing, suggest `skemo watch name.skemo` (live preview in the browser) or the
skemo editor.
