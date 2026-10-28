#!/usr/bin/env python3
"""Writes src/crates.txt: the Rust crates compiled into `skemo` (normal dependencies of skemo-cli,
every platform, the project's own skemo-* crates left out), one per line: name, version, licence.
`skemo licences` prints it; `tests/licences.rs` checks it against Cargo.lock.

Run it again after a dependency change: `python3 tools/gen-licences.py` (needs cargo, offline)."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
meta = json.loads(subprocess.run(
    ["cargo", "metadata", "--format-version", "1", "--offline"],
    cwd=root, check=True, capture_output=True, text=True).stdout)
packages = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
start = next(p["id"] for p in meta["packages"] if p["name"] == "skemo-cli")
seen, todo = set(), [start]
while todo:
    i = todo.pop()
    if i in seen:
        continue
    seen.add(i)
    for d in nodes[i]["deps"]:
        if any(k["kind"] in (None, "normal") for k in d["dep_kinds"]):
            todo.append(d["pkg"])
rows = sorted({(packages[i]["name"], packages[i]["version"], packages[i]["license"] or "see its repository")
               for i in seen if not packages[i]["name"].startswith("skemo")})
out = "".join(f"{n} {v} {l}\n" for n, v, l in rows)
(root / "src" / "crates.txt").write_text(out)
print(f"src/crates.txt: {len(rows)} crates")
