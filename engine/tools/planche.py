#!/usr/bin/env python3
"""Demo contact sheet: the prototype oracle (Chromium) on the left, the Rust rendering (no browser) on the right.
Usage: tools/planche.py <skemo binary> <output folder>   (from the root of the engine repository)
Produces <output>/planche.svg (embedded PNG images); convert it with rsvg-convert."""
import base64, struct, subprocess, sys, os
BIN, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
CASES = [
    ("auto-elk", "reseau.pug", None, None),
    ("manuel-via", "reseau-manuel-via.pug", None, None),
    ("deux-pins", "reseau-2pins.pug", None, None),
    ("process", "process.pug", "process.css", None),
    ("design", "design.pug", "design.css", None),
    ("design-sombre", "design.pug", "design.css", "dark"),
    ("design-print", "design.pug", "design.css", "print"),
    ("entete", "reseau-entete.pug", None, None),
    ("timeline", "timeline.pug", "timeline.css", None),
    ("frise", "frise.pug", "frise.css", None),
    ("frise-print", "frise.pug", "frise.css", "print"),
]
def size(p):
    with open(p, "rb") as f:
        h = f.read(24)
    return struct.unpack(">II", h[16:24])
rows, y, W = [], 60, 0
for name, pug, css, theme in CASES:
    out = f"{OUT}/{name}.png"
    cmd = [BIN, "render", f"fixtures/{pug}", "-o", out, "-q"] + (["--css", f"fixtures/{css}"] if css else []) + (["--theme", theme] if theme else [])
    subprocess.run(cmd, check=True)
    imgs = []
    for p in (f"fixtures/{name}.png", out):
        w, h = size(p)
        imgs.append((p, w / 2, h / 2))
    h = max(i[2] for i in imgs)
    x = 20
    parts = [f'<text x="20" y="{y + 18}" font-family="Inter" font-size="16" font-weight="700" fill="#1F2430">{name}</text>']
    for k, (p, w, hh) in enumerate(imgs):
        data = base64.b64encode(open(p, "rb").read()).decode()
        label = "oracle (POC, Chromium)" if k == 0 else "skemo (Rust, sans navigateur)"
        parts.append(f'<text x="{x}" y="{y + 40}" font-family="Inter" font-size="12" fill="#4A5262">{label}</text>')
        parts.append(f'<image x="{x}" y="{y + 48}" width="{w}" height="{hh}" href="data:image/png;base64,{data}"/>')
        parts.append(f'<rect x="{x}" y="{y + 48}" width="{w}" height="{hh}" fill="none" stroke="#D3D8DF"/>')
        x += w + 30
    W = max(W, x)
    rows.append("".join(parts))
    y += h + 80
svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{y}" viewBox="0 0 {W} {y}"><rect width="100%" height="100%" fill="#FFFFFF"/><text x="20" y="34" font-family="Inter" font-size="22" font-weight="800" fill="#1F2430">skemo — oracle du POC face au moteur Rust</text>{"".join(rows)}</svg>'
open(f"{OUT}/planche.svg", "w").write(svg)
print(f"{OUT}/planche.svg")
