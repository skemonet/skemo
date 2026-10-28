# Requirements — Orthogonal connector router (`router`)

> **Generic, reusable** Rust library: give it fixed rectangles and connectors, and it computes clean orthogonal routes that go around them.
> It knows neither the diagram format, nor cards, nor CSS. The engine (`engine`) uses it, but any other project must be able to do so.
> **Targeted licence: MIT OR Apache-2.0.**

## 1. Why this repository exists

The prototype used two routers:
- the one built into ELK, valid **only** on a layout computed by ELK;
- **libavoid**, in WebAssembly, as soon as a card is positioned by hand.

Problems: two code paths, an abrupt change of look of the diagram at the first `at=`, and an LGPL licence incompatible with a statically linked Rust binary under a permissive licence.

**Decision (D-010)**: a **single router**, written in Rust, used in all cases (automatic layout as well as pinned positions). ELK is now only used to place the cards; its routes can serve as **hints**.

## 2. Starting point: hightower

**hightower** (github.com/surt91/hightower, **MIT OR Apache-2.0**) is an orthogonal router in Rust:
- **Hightower's heuristic (1969)**: "escape lines" leave both endpoints and are joined where they cross; obstacles are indexed in B-trees. It is very fast (≈ 1 to 5 µs per route for 20 rectangles), with few bends, but incomplete (≈ 2 % of existing paths missed);
- **complete fallback method**: `route_visibility`, A* on an orthogonal visibility graph, which never misses a path;
- also: a "second improvement" post-processing that removes detours, a `boundary_retreat` option, re-routing with existing routes as obstacles, SVG rendering, and an egui demo compilable to WebAssembly;
- scenario tests and property tests (comparison with a BFS on a grid).

**What it lacks for us**: the **spacing of parallel segments** (nudging), **attachment sides** and the **distribution of attachment points**, **waypoints**, **traversable zones**, **incremental re-routing**. It is also a single-author research project, with no published version.

**To do at milestone R0**: evaluate hightower on our test sets (§8), read its code, then choose between three options:
- **(a) depend** on the crate and add our layers on top;
- **(b) fork / vendor** the code into this repository, keeping the licence notice and the attribution;
- **(c) rewrite** from the papers, if the code is too fragile.

*(Option (c) was chosen: D-018.)*

## 3. Writing rule: "from the papers", never from libavoid's code

libavoid is **LGPL-2.1+**. Translating its code, even by rephrasing, would produce a derived work, hence under LGPL.

- **Forbidden**: opening, reading or translating the source code of libavoid (C++), libavoid-js or libavoid-rust (the latter is a port, hence probably LGPL).
- **Allowed**: the **published papers** (§10), **hightower**'s code (MIT/Apache, with attribution), and the libavoid-js **outputs** present in `fixtures/` (they are data, not code).
- **Traceability**: keep `PROVENANCE.md`, which associates each module with the paper or source (hightower) it draws inspiration from.

*This is not legal advice: have it validated before distribution.*

## 4. Data model (API)

```rust
// Units: px (f64), origin at the top left, y downwards.
pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }

pub enum ObstacleKind {
    Shape,   // never crossed; possible connector endpoints
    Region,  // traversable only by the connectors that allow it (passThrough)
}
pub struct Obstacle { pub id: String, pub rect: Rect, pub kind: ObstacleKind }

pub enum Side { Top, Bottom, Left, Right }
pub enum End {
    Shape { id: String, side: Option<Side> },   // None = best side
    Point { x: f64, y: f64 },                   // free endpoint (editor)
}
pub struct Connector {
    pub id: String,
    pub from: End, pub to: End,
    pub checkpoints: Vec<(f64, f64)>,           // waypoints, in order
    pub pass_through: Vec<String>,              // regions this connector may cross
    pub ignore: Vec<String>,                    // shapes it crosses as if absent (not shown with it)
    pub hint: Option<Vec<(f64, f64)>>,          // indicative route (e.g. ELK): to follow if valid
}
pub struct Params { /* §6 */ }

pub struct Route { pub id: String, pub points: Vec<(f64, f64)> }   // orthogonal polyline
pub enum Diagnostic { Unroutable { id: String }, CheckpointUnreachable { id: String, index: usize }, … }

pub fn route(obstacles: &[Obstacle], connectors: &[Connector], params: &Params) -> (Vec<Route>, Vec<Diagnostic>);

// Incremental API (editor):
pub struct Router { … }
impl Router {
    pub fn new(params: Params) -> Self;
    pub fn set_obstacle(&mut self, o: Obstacle);       // add or move
    pub fn remove_obstacle(&mut self, id: &str);
    pub fn set_connector(&mut self, c: Connector);
    pub fn remove_connector(&mut self, id: &str);
    pub fn update(&mut self) -> Vec<Route>;            // only re-routes what is affected
}
```

Exact names are free. The **concepts** are imposed.

## 5. Requirements

Priorities: **M** = mandatory (v1), **S** = desirable, **C** = later.

| ID | Requirement | Prio |
|---|---|---|
| R-01 | **Orthogonal** routes (horizontal and vertical segments); first and last points **on the edge** of the shape, perpendicular to the side | M |
| R-02 | **No** segment crosses a `Shape` (except at its own endpoints), with a `shape_buffer` margin | M |
| R-03 | **Regions**: a connector only crosses a `Region` if its id is in `pass_through`; otherwise it is an obstacle for it | M |
| R-04 | **Completeness**: if a path exists, it is found (fast heuristic, then complete fallback method) | M |
| R-05 | **Cost** = length + `segment_penalty` × bends + `crossing_penalty` × crossings with other connectors | M |
| R-06 | **Forced side** (`side`) at the start or the end; otherwise the best side | M |
| R-07 | **Waypoints**: crossed in order, with a clean continuity of direction (no U-turn on the point) | M |
| R-08 | **Spacing** (nudging) of overlapping collinear segments: ideal gap `ideal_nudging`, never < 6 px, order chosen to **minimise crossings** (see Wybrow et al. 2009, §10) | M |
| R-09 | **Distribution of attachment points**: several connectors on the same side are spread along the side (not all in the middle), in the order that avoids crossings | M |
| R-10 | **Simplification**: merging of collinear segments, removal of micro-jogs (< `min_jog`) | M |
| R-11 | **Determinism**: same inputs → same outputs, bit for bit (stable order, no `HashMap` iterated without sorting) | M |
| R-12 | **Diagnostics** rather than panic: connector impossible to route, unreachable waypoint (in that case, route without that point and report it) | M |
| R-13 | **WebAssembly-compatible** (`wasm32-unknown-unknown`): no system dependency, no mandatory `std::time` nor thread | M |
| R-14 | **Incremental**: `Router::update` returns the same result as a full computation (D-031: the drawing does not depend on the editing history). It recomputes everything as soon as one connector is touched (modified obstacle, or route crossing its zone); re-routing only the touched connectors is merely an optimisation, allowed only if the result stays bit-identical | S |
| R-15 | **Hint** (`hint`): if the indicative route is valid (R-01 to R-03), it is reused and only spaced; otherwise route normally | S |
| R-16 | Free endpoints (`End::Point`), for the editor (dragging an arrow into the void) | S |
| R-17 | **Non-orthogonal polyline** mode and **curve** mode (corner smoothing) | C (not for now) |
| R-18 | **Hyperedges** (one start, several ends sharing a trunk), D-034. The trunk leaves by a side that every target lies entirely beyond (the widest gap if several), otherwise towards the targets' centre; a branch enters its target by the side facing the trunk; branches split at one point (D-045, D-047) | C |
| R-19 | **Centring** (D-035): free intermediate segments are moved to the middle of the free corridor between the shapes that border them (not for segments carrying a waypoint) | M |
| R-20 | **Straight lines** (D-044): two shapes whose facing sides overlap across are linked by one segment (no jog between a short shape and a taller one), unless the connector is a hyperedge, has waypoints or a forced side that does not face; connectors both ways between the same sides stay straight when spread | S |
| R-22 | **Flow** (D-047): `Params::flow` and `flow_penalty`: a start by another side than the flow's, or an end by another side than the one facing it, costs extra (twice for the opposite side); none by default | S |
| R-23 | **Roomy sides** (D-047): `Params::port_spread`: the ports of a side are `length / (n + 1)` apart, at most `port_spread`, at least the ideal spacing; 0 by default | S |
| R-24 | **No shared line at a port** (D-047): two connectors (outside one group) whose end segments lie on one line and overlap: the arrival moves along its side to the nearest place one spacing from the other ports | S |
| R-21 | **Ports beside an obstacle** (D-044): when a shape sits right in front of the middle of a side (closer than two buffers), the side also offers the nearest clear points on each side of it | S |

## 6. Parameters (defaults from the prototype)

| Parameter | Default | Role |
|---|---|---|
| `shape_buffer` | 14 | margin around the shapes |
| `ideal_nudging` | 14 | target gap between parallel segments |
| `segment_penalty` | 50 | cost of a bend |
| `crossing_penalty` | 100 | cost of a crossing |
| `min_jog` | 6 | length under which a jog is removed |
| `port_spacing` | 12 | minimum gap between attachment points on the same side |

## 7. Expected quality (measurable criteria)

For each test set:
- 0 violations of R-01, R-02, R-03 and R-07;
- bends per connector ≤ those of `expected` + 1;
- total length ≤ 1.15 × that of `expected`;
- minimum spacing between parallel segments ≥ 12 px;
- crossings between connectors ≤ those of `expected`.

`expected` = the prototype's libavoid routes. It is a **quality reference**, not a pixel target.

Performance (native, standard machine): 50 shapes / 100 connectors < 20 ms; 500 shapes / 1,000 connectors < 500 ms.

## 8. Test sets (`fixtures/`)

Extracted from the prototype's oracle (its `*.layout.json` reference outputs):

| File | Contents |
|---|---|
| `manuel-avoid.json` | 8 hand-placed cards and 2 zones, 8 connectors |
| `manuel-via.json` | same + `side`, `checkpoints`, varied styles |
| `deux-pins.json` | ELK layout with 2 cards moved (the "few `at=`" case) |
| `design.json` | header style (cards of varied sizes), 4 chips converging on the same side of the Gateway (**spacing and distribution of attachment points**), `side` imposed on a reverse-direction arrow |
| `conflit.json` | **overlapping cards** (Web placed on the Switch): the router must not panic, and must report or go around cleanly |
| `timeline.json` | engine timeline mode: long, tight lanes, **all traversable** (`passThrough` = all lanes), forced sides, reduced margin (`shapeBuffer` 8). Hard case: dependencies between lanes must use the gutters between lanes |

Format:

```json
{
  "name": "manuel-via",
  "provenance": "…",
  "units": "px, origin at the top left, y downwards",
  "params": { "shapeBuffer": 14, "idealNudging": 14, "segmentPenalty": 50, "crossingPenalty": 100 },
  "obstacles": [ { "id": "fw", "rect": [355, 150, 220, 94], "kind": "shape" },
                 { "id": "lan", "rect": [358, 314, 674, 342], "kind": "region" } ],
  "connectors": [ { "id": "fw->mail", "from": { "shape": "fw", "side": "left" }, "to": { "shape": "mail", "side": "top" },
                    "checkpoints": [[300, 420]], "passThrough": ["dmz"] } ],
  "expected": { "fw->mail": [[355, 204.2], [300, 204.2], [300, 438], [125.3, 438], [125.3, 470]] },
  "checks": [ "…" ]
}
```

`rect` = `[x, y, w, h]`. The `expected` routes start and end on the shapes' edges.

Other tests written:
- **robustness tests** on random degenerate scenes (`tests/robustness.rs`): never a panic, deterministic output, invalid input reported;
- a **visual review tool**: `cargo run --example review -- fixtures/manuel-via.json` produces an SVG superimposing our route (solid) and `expected` (dashed), for human review; `--clean` draws our routes only;
- input fuzzing (degenerate rectangles, overlaps, connector from a shape to itself).

## 9. Milestones

| Milestone | Contents | Exit criterion |
|---|---|---|
| **R0** — spike | Run hightower **as is** on the 4 sets (without spacing, without sides); measure violations, bends, length, time; read the code | Report + review SVGs; choice (a), (b) or (c) validated by the user |
| **R1** — routing | R-01 to R-07, R-10 to R-13 | §7 criteria, except spacing, on the 4 sets |
| **R2** — beauty | R-08 and R-09 (spacing, distribution of attachment points) | All §7 criteria; visual review validated |
| **R3** — editor | R-14 to R-16; WASM build; API documentation; crate publication | Minimal WASM example; complete `cargo doc` |

## 10. References (to read, in this order)

1. Wybrow, Marriott, Stuckey — **Orthogonal Connector Routing**, Graph Drawing 2009 (LNCS 5849). Orthogonal visibility graph, A* with bend penalty, **spacing and ordering of segments**. https://people.eng.unimelb.edu.au/pstuckey/papers/gd09.pdf
2. Marriott, Stuckey, Wybrow — **Seeing Around Corners: Fast Orthogonal Connector Routing**, Diagrams 2014. Speed-ups and alternatives to the full visibility graph. https://users.monash.edu/~mwybrow/papers/marriott-diagrams-2014.pdf
3. Hightower — **A solution to line-routing problems on the continuous plane**, DAC 1969 (basis of hightower).
4. codecentric blog post — **How to Connect Two Boxes? Orthogonal Routing in Microseconds** (presentation of hightower and comparison with visibility + A*). https://www.codecentric.de/en/knowledge-hub/blog/how-do-you-connect-two-boxes-orthogonal-routing-in-microseconds
5. **hightower** code: https://github.com/surt91/hightower

## 11. Out of scope

Placement of the shapes, connector labels (the engine handles them, but `Route` must make it possible to find the longest segment), rendering, styles.
