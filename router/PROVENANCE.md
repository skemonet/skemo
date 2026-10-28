# Code provenance

Mandatory register (see SPEC §3): for each module, where the ideas and the code come from.
**No entry may cite the source code of libavoid, libavoid-js or libavoid-rust.**

| Module | Source | Type | Source licence | Notes |
|---|---|---|---|---|
| *(example)* `search/hightower.rs` | hightower (surt91), commit `…` | code taken and adapted | MIT OR Apache-2.0 | notice kept at the top of the file |
| *(example)* `nudge/` | Wybrow, Marriott, Stuckey, GD 2009, §4 | algorithm re-implemented from the paper | — | |
| `spikes/r0-hightower/` (milestone R0, outside the crate) | hightower (surt91), commit `c849003` | **git dependency**, no code copied; adapter and metrics written for the spike | MIT OR Apache-2.0 (declared in `Cargo.toml`; the repository has no LICENSE file) | evaluation tool, not published |
| `src/nudge.rs` (R2) | Wybrow, Marriott, Stuckey, *Orthogonal Connector Routing*, GD 2009, §4 (ordering and spacing of shared segments) | idea from the paper, re-implemented; greedy, symmetric separation (no quadratic programming), order chosen by comparing the crossings of the two possible orders | — | original code; no libavoid source consulted |
| `src/nudge.rs`, `center_segments` | general drawing practice (bends centred in the free corridor between shapes) | original code | — | no source consulted |
| `src/hint.rs`, `valid_hint` (R-15) | SPEC §5 (R-15) | original code | — | reuse of a valid hint route |
| `src/scene.rs`, sparse graph | Wybrow, Marriott, Stuckey, GD 2009, §2 (visibility segments stopped at the first obstacle) | idea from the paper, re-implemented (coverage of lines by intervals, nodes at covered intersections) | — | original code |
