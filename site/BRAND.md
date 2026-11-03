# skemo brand charter

The look of skemo.net, its documentation and anything that presents the project (README images, slides, social cards).
The values live in `site/src/brand.css`; this page says what they are for. The diagrams themselves keep skemo's own
default theme: the brand frames them, it does not restyle them.

## The mark

![mark](src/brand/mark.svg)

A route drawn as an **S**: it leaves from a node at the bottom left, passes a junction in the middle and ends on an amber
dot. It says what skemo does, joining boxes with clean lines, and it reads as the initial of the name.

| File | Use |
|---|---|
| `src/brand/logo-light.png`, `logo-dark.png` | the mark and the word, on light and on dark backgrounds (header, footer, README) |
| `src/brand/icon-light.png`, `icon-dark.png` | the mark alone in a rounded square (app icons, avatars, social profiles) |
| `src/brand/mark.svg` (= `src/favicon.svg`) | the dark icon as a vector, for the favicon and small sizes |

- **Clear space**: at least the amber dot's diameter on every side.
- **Minimum size**: the logo 20 px tall on screen, the icon 16 px.
- **Do not** recolour the mark, move the amber dot, add effects (shadow, outline, gradient), or set the logo on a busy
  picture. On a mid-tone background, use the icon instead of the logo.
- The word is always lowercase: **skemo**, also at the start of a sentence.

## Colours

The palette is the logo's: an ink, a paper and one accent. Everything else is a neutral derived from them.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--amber` / `--accent` | `#E8A33D` | `#E8A33D` | the route's end: the primary button, dots, bars, selection, focus ring. Never for body text on light |
| `--accent-ink` | `#8F550A` | `#F0B65A` | amber text and links (6.4:1 on white, 9.7:1 on the dark surface) |
| `--accent-soft` | `#FBEEDA` | `#2A2114` | the current item's tint |
| `--ink` | `#14161B` | `#F3F1EC` | headings and strong text |
| `--ink-2` | `#3A3D45` | `#C9C7C1` | body text |
| `--muted` | `#5C6068` | `#9BA0A8` | secondary text (5.8:1 and 7.2:1) |
| `--bg` / `--bg-2` | `#F6F5F1` / `#EFEDE7` | `#0F1115` / `#0B0D10` | page and alternate sections: warm paper, the icon's black |
| `--surface` / `--surface-2` | `#FFFFFF` / `#FAF9F6` | `#16191F` / `#1B1F26` | cards, panels, window bars |
| `--line` / `--line-2` | `#DEDBD3` / `#CFCBC1` | `#262A32` / `#343943` | borders and rules |
| `--code-bg` | `#14161B` | `#0B0D10` | code blocks, dark in both schemes |

The site follows the reader's colour scheme; both schemes are first-class, and the dark one uses the icon's background.
Text meets WCAG AA (4.5:1) everywhere; the amber button carries ink text (8.4:1).

## Type

- **Inter** (400, 500, 700) for everything read: headings tight (`letter-spacing` −0.025 to −0.035 em), body at 16–17 px,
  line height 1.6.
- **JetBrains Mono** for code, file names, commands, and the small uppercase labels (eyebrows, table of contents,
  breadcrumbs): the same fonts skemo embeds in its renderings, so the site and the diagrams match.
- Both are SIL OFL 1.1 and served from the site, no third-party font service.

## Components

- **Eyebrow**: a mono uppercase label after a 7 px amber dot (the route's end) above each section title.
- **Buttons**: primary amber with ink text; secondary on the surface with a hairline border. 10 px radius.
- **Window**: a rendered diagram in a framed panel with a bar naming the file (`diagram.skemo → diagram.svg`).
- **Panels and tiles**: surface, 1 px line, 12 px radius (18 px for large panels), a soft shadow only on the hero window.
  Hover: the border turns amber.
- **Tabs** (diagram / source): small pills; the selected one inverts to ink.
- **Dotted plane**: the hero's background is a 22 px dot grid, the canvas a diagram is laid on, with a faint amber glow.
- **Code**: dark blocks in both schemes; inline code on `--bg-2` with a hairline border.
- **Diagrams** are rendered with `--transparent` so they sit on the panel's surface, in the reader's scheme.

## Voice

Plain and exact: say what skemo does and how it is measured, never more. Short sentences, the reader as "you", no
superlatives, figures only when a script computes them. The same words in both languages (English first, French
written for French readers, not word for word).
