#!/usr/bin/env python3
"""Contact sheet of icons and links: the same diagram in three looks and three themes, plus the
secondary cards, a zone and linked cards with and without the marker.
Usage: tools/planche-icons.py <skemo binary> <output png>   (from the root of the engine repository)
Needs rsvg-convert."""
import base64, os, struct, subprocess, sys, tempfile

BIN, OUT = sys.argv[1], sys.argv[2]
TMP = tempfile.mkdtemp()

MAIN = """diagram(dir="right" look="{look}")
  zone#vpc(icon="cloud") Production VPC
    card#web.app(icon="globe" tag="App") Storefront
      li Next.js
      to(db)
    card#db.data(icon="database" tag="DB") Orders
      li PostgreSQL 16
      li 10.0.0.12/24
"""
EXTRA = """diagram(dir="right")
  card#n.note(icon="info") A note
  card#c.chip(icon="user") A chip
  card#i.info(icon="triangle-alert") An info
  zone#z(icon="shield" href="#a") Linked zone
    card#a(icon="key" href="https://example.org/doc" tag="Doc") Linked card
      to(b href="#b") uplink
    card#b(href="docs/b.md") Relative link
"""
CSS_TYPES = ".app { --c: #2F6FDE; } .data { --c: #0E8A7B; }"
NOMARK = "card, zone { --link-mark: none; }"


def write(name, text):
    p = os.path.join(TMP, name)
    open(p, "w").write(text)
    return p


def render(src, theme, css=None):
    s = write("d.skemo", src)
    c = write("t.css", CSS_TYPES + (css or ""))
    out = os.path.join(TMP, f"o{len(os.listdir(TMP))}.png")
    subprocess.run([BIN, "render", s, "-o", out, "-q", "--css", c, "--theme", theme], check=True)
    return out


def size(p):
    h = open(p, "rb").read(24)
    return struct.unpack(">II", h[16:24])


def img(p, x, y, scale=0.5):
    w, h = size(p)
    data = base64.b64encode(open(p, "rb").read()).decode()
    return f'<image x="{x}" y="{y}" width="{w * scale}" height="{h * scale}" href="data:image/png;base64,{data}"/>', w * scale, h * scale


parts, y, W = [], 20, 0
themes = ["light", "dark", "print"]
for look in ["band", "header", "classic"]:
    x, rowh = 20, 0
    parts.append(f'<text x="20" y="{y + 14}" font-family="Inter" font-size="15" font-weight="700" fill="#1F2430">look = {look}</text>')
    y += 24
    for th in themes:
        s, w, h = img(render(MAIN.format(look=look), th), x, y)
        parts.append(s)
        x += w + 16
        rowh = max(rowh, h)
    W = max(W, x)
    y += rowh + 20
parts.append(f'<text x="20" y="{y + 14}" font-family="Inter" font-size="15" font-weight="700" fill="#1F2430">secondary cards, zone and links: marker on (left) and off (right)</text>')
y += 24
x, rowh = 20, 0
for css in [None, NOMARK]:
    s, w, h = img(render(EXTRA, "light", css), x, y)
    parts.append(s)
    x += w + 16
    rowh = max(rowh, h)
W, H = max(W, x), y + rowh + 20
svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}"><rect width="100%" height="100%" fill="#ffffff"/>{"".join(parts)}</svg>'
svg_path = write("sheet.svg", svg)
subprocess.run(["rsvg-convert", svg_path, "-o", OUT], check=True)
print(OUT, W, H)
