#!/usr/bin/env python3
"""Measure how many tokens a model writes to produce a diagram: skemo source vs. the equivalent SVG.

For every example, the cost of the skemo route is the `.skemo` source plus its custom CSS (if any);
the cost of the drawing route is the SVG that skemo renders from it, i.e. the same picture written
out with explicit coordinates, reduced to what a careful hand would write (`lean`): without the
embedded source, the class and data attributes, with the font declared once and coordinates to one
decimal. Tokens are counted with a byte-pair-like approximation (words, groups
of up to three digits, single punctuation marks), applied identically to both sides: absolute counts
differ from any given tokenizer, the ratio is what matters.

Usage: measure_tokens.py <skemo binary> <examples dir> <output json>
"""
import json
import pathlib
import re
import subprocess
import sys

TOKEN = re.compile(r" ?[A-Za-z]+| ?\d{1,3}|\n+| +|[^\sA-Za-z\d]")


def tokens(text: str) -> int:
    return len(TOKEN.findall(text))


def lean(svg: str) -> str:
    """The rendered SVG minus what does not draw: the embedded `.skemo` source (metadata), class,
    data and xml:space attributes; the font family set once on the root; coordinates to one decimal."""
    svg = re.sub(r"<metadata>.*?</metadata>", "", svg, flags=re.S)
    svg = re.sub(r' (?:class|data-[\w-]+|xml:space|font-family)="[^"]*"', "", svg)
    svg = svg.replace("<svg ", '<svg font-family="Inter, sans-serif" ', 1)
    return re.sub(r"(\d+\.\d)\d+", r"\1", svg)


def measure(skemo: str, examples: pathlib.Path) -> dict:
    rows = []
    for src in sorted(examples.glob("*.skemo")):
        css = src.with_suffix(".css")
        cmd = [skemo, "--lang", "en", "render", str(src), "-q", "--format", "svg", "-o", "-"]
        if css.exists():
            cmd += ["--css", str(css)]
        svg = lean(subprocess.run(cmd, check=True, capture_output=True, text=True).stdout)
        text = src.read_text() + (css.read_text() if css.exists() else "")
        rows.append({
            "example": src.stem,
            "skemo_bytes": len(text.encode()),
            "svg_bytes": len(svg.encode()),
            "skemo_tokens": tokens(text),
            "svg_tokens": tokens(svg),
        })
    total_skemo = sum(r["skemo_tokens"] for r in rows)
    total_svg = sum(r["svg_tokens"] for r in rows)
    return {
        "examples": rows,
        "skemo_tokens": total_skemo,
        "svg_tokens": total_svg,
        "ratio": round(total_svg / total_skemo, 1),
        "saving_percent": round(100 * (1 - total_skemo / total_svg)),
    }


def main() -> None:
    skemo, examples, out = sys.argv[1], pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3])
    summary = measure(skemo, examples)
    rows = summary["examples"]
    out.write_text(json.dumps(summary, indent=2) + "\n")
    for r in rows:
        print(f'{r["example"]:10} skemo {r["skemo_tokens"]:5}  svg {r["svg_tokens"]:6}  x{r["svg_tokens"] / r["skemo_tokens"]:.1f}')
    print(f'total      skemo {summary["skemo_tokens"]:5}  svg {summary["svg_tokens"]:6}  '
          f'x{summary["ratio"]}  saving {summary["saving_percent"]} %')


if __name__ == "__main__":
    main()
