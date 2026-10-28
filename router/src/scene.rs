//! The scene: indexed obstacles and a sparse orthogonal visibility graph.
//!
//! The orthogonal visibility graph (Wybrow, Marriott, Stuckey, GD 2009, §2) has for nodes the
//! intersections of the horizontal and vertical lines through the "interesting points": the
//! obstacles' sides (with and without margin), the ports, the waypoints. It is not built: the
//! search walks it on demand (node = index of vertical × horizontal line), and asks the index
//! whether a stretch between two neighbouring lines is free.

use crate::geom::{Point, Rect};
use crate::model::ObstacleKind;

/// An obstacle ready for queries.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    pub id: String,
    pub kind: ObstacleKind,
    /// The rectangle itself: its inside is never crossed (shapes) or only when allowed (regions).
    pub hard: Rect,
    /// The rectangle grown by the margin: passing there costs more.
    pub soft: Rect,
    /// A shape some connector ignores (`Connector::ignore`): no hole in the graph, it blocks
    /// segment by segment the connectors it is active for, like a region.
    pub optional: bool,
}

impl Item {
    /// A shape every connector goes round: a hole in the graph.
    pub fn wall(&self) -> bool {
        self.kind == ObstacleKind::Shape && !self.optional
    }
}

/// Spatial index on a regular grid of cells (deterministic, no hash table).
#[derive(Clone, Debug)]
struct Buckets {
    x0: f64,
    y0: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl Buckets {
    fn new(items: &[Item]) -> Buckets {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for it in items {
            x0 = x0.min(it.soft.x0());
            y0 = y0.min(it.soft.y0());
            x1 = x1.max(it.soft.x1());
            y1 = y1.max(it.soft.y1());
        }
        if items.is_empty() {
            (x0, y0, x1, y1) = (0.0, 0.0, 1.0, 1.0);
        }
        let area = ((x1 - x0) * (y1 - y0)).max(1.0);
        // about one cell per obstacle, at most 256 × 256 cells
        let mut cell = (area / items.len().max(1) as f64).sqrt().max(16.0);
        while ((x1 - x0) / cell).ceil() > 256.0 || ((y1 - y0) / cell).ceil() > 256.0 {
            cell *= 2.0;
        }
        let nx = (((x1 - x0) / cell).ceil() as usize).max(1);
        let ny = (((y1 - y0) / cell).ceil() as usize).max(1);
        let mut b = Buckets {
            x0,
            y0,
            cell,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
        };
        for (k, it) in items.iter().enumerate() {
            let (i0, j0, i1, j1) = b.range(it.soft.x0(), it.soft.y0(), it.soft.x1(), it.soft.y1());
            for j in j0..=j1 {
                for i in i0..=i1 {
                    b.cells[j * nx + i].push(k as u32);
                }
            }
        }
        b
    }

    fn range(&self, ax: f64, ay: f64, bx: f64, by: f64) -> (usize, usize, usize, usize) {
        let c =
            |v: f64, o: f64, n: usize| (((v - o) / self.cell).floor().max(0.0) as usize).min(n - 1);
        (
            c(ax, self.x0, self.nx),
            c(ay, self.y0, self.ny),
            c(bx, self.x0, self.nx),
            c(by, self.y0, self.ny),
        )
    }
}

/// Obstacles et lignes candidates.
#[derive(Clone, Debug)]
pub(crate) struct Scene {
    pub items: Vec<Item>,
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    buckets: Buckets,
    /// Shapes whose margin cuts each vertical (`vsoft[i]`) or horizontal (`hsoft[j]`) line, strict
    /// inside: the length run in a margin is computed on demand.
    vsoft: Vec<Vec<u32>>,
    hsoft: Vec<Vec<u32>>,
    /// Indices of the regions (active or not depending on the connector).
    regions: Vec<usize>,
    /// Sparse visibility graph (Wybrow et al. 2009, §2): each interesting point casts a vertical and
    /// a horizontal segment that stop at the first shape. A node `(i, j)` exists if both lines are
    /// covered there. `vnodes[i]`: indices `j` of the nodes of vertical line `i`, sorted;
    /// `vlink[i][k]`: nodes `k` and `k + 1` linked.
    vnodes: Vec<Vec<u32>>,
    vlink: Vec<Vec<bool>>,
    hnodes: Vec<Vec<u32>>,
    hlink: Vec<Vec<bool>>,
    /// Identifier of each node (same order as `vnodes`), coordinates and neighbours by identifier
    /// (order of `Dir::ALL`, `u32::MAX` = none): a search without hashing.
    vids: Vec<Vec<u32>>,
    id_ij: Vec<(u32, u32)>,
    nbr: Vec<[u32; 4]>,
    /// Length run in the shapes' margins, per node and direction (edge to the neighbour).
    nsoft: Vec<[f64; 4]>,
}

/// Rounding of the lines' coordinates: merges floating-point noise.
pub(crate) fn snap(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

fn sort_dedup(v: &mut Vec<f64>) {
    for x in v.iter_mut() {
        *x = snap(*x);
    }
    v.sort_by(f64::total_cmp);
    v.dedup();
}

/// A corridor narrower than two margins between two shapes: routes run through it at equal
/// distance from both.
enum Corridor {
    /// Vertical line `x`, over the vertical overlap `[y0, y1]` of the two shapes.
    Vertical { x: f64, y0: f64, y1: f64 },
    /// Horizontal line `y`, over the horizontal overlap `[x0, x1]`.
    Horizontal { y: f64, x0: f64, x1: f64 },
}

fn corridors(shapes: &[Rect], buffer: f64) -> Vec<Corridor> {
    let mut out = Vec::new();
    for a in shapes {
        for b in shapes {
            let gx = b.x0() - a.x1();
            if gx > 0.0 && gx < 2.0 * buffer && a.y0() < b.y1() + buffer && b.y0() < a.y1() + buffer
            {
                out.push(Corridor::Vertical {
                    x: a.x1() + gx / 2.0,
                    y0: a.y0().max(b.y0()),
                    y1: a.y1().min(b.y1()),
                });
            }
            let gy = b.y0() - a.y1();
            if gy > 0.0 && gy < 2.0 * buffer && a.x0() < b.x1() + buffer && b.x0() < a.x1() + buffer
            {
                out.push(Corridor::Horizontal {
                    y: a.y1() + gy / 2.0,
                    x0: a.x0().max(b.x0()),
                    x1: a.x1().min(b.x1()),
                });
            }
        }
    }
    out
}

/// Lines of the grid: imposed coordinates, obstacles' edges (hard and with margin), middles of
/// corridors and an outer frame (so that everything can be gone round), rounded, sorted, without
/// duplicates.
fn grid_lines(
    items: &[Item],
    buffer: f64,
    mut xs: Vec<f64>,
    mut ys: Vec<f64>,
) -> (Vec<f64>, Vec<f64>) {
    for it in items {
        xs.extend([it.hard.x0(), it.hard.x1(), it.soft.x0(), it.soft.x1()]);
        ys.extend([it.hard.y0(), it.hard.y1(), it.soft.y0(), it.soft.y1()]);
    }
    let shapes: Vec<Rect> = items
        .iter()
        .filter(|i| i.kind == ObstacleKind::Shape)
        .map(|i| i.hard)
        .collect();
    for c in corridors(&shapes, buffer) {
        match c {
            Corridor::Vertical { x, .. } => xs.push(x),
            Corridor::Horizontal { y, .. } => ys.push(y),
        }
    }
    let bounds = |v: &[f64]| {
        v.iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), &c| (lo.min(c), hi.max(c)))
    };
    let ((x0, x1), (y0, y1)) = (bounds(&xs), bounds(&ys));
    if x0 <= x1 && y0 <= y1 {
        let m = 2.0 * buffer + 10.0;
        xs.extend([x0 - m, x1 + m]);
        ys.extend([y0 - m, y1 + m]);
    }
    sort_dedup(&mut xs);
    sort_dedup(&mut ys);
    (xs, ys)
}

/// Narrows `span` (free interval around `at`) by a shape `[lo, hi]` on this axis.
fn clip_span(span: &mut (f64, f64), lo: f64, hi: f64, at: f64) {
    if hi <= at {
        span.0 = span.0.max(hi);
    }
    if lo >= at {
        span.1 = span.1.min(lo);
    }
}

/// Indices of the (sorted) lines strictly between `lo` and `hi`; empty if the obstacle is flat.
fn lines_strictly_inside(lines: &[f64], lo: f64, hi: f64) -> std::ops::Range<usize> {
    let start = lines.partition_point(|&l| l <= lo);
    let end = lines.partition_point(|&l| l < hi).max(start);
    start..end
}

/// Merges the intervals that touch.
fn merge_intervals(v: &mut Vec<(f64, f64)>) {
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in v.iter() {
        match out.last_mut() {
            Some(l) if a <= l.1 + 1e-9 => l.1 = l.1.max(b),
            _ => out.push((a, b)),
        }
    }
    *v = out;
}

/// Index of the (sorted, disjoint) interval that holds `x`.
fn interval_at(v: &[(f64, f64)], x: f64) -> Option<usize> {
    let k = v.partition_point(|iv| iv.1 < x - 1e-9);
    (k < v.len() && v[k].0 <= x + 1e-9).then_some(k)
}

/// For each line: is node `k` in the same interval as node `k + 1`?
fn same_interval_links(ints: &[Vec<usize>]) -> Vec<Vec<bool>> {
    ints.iter()
        .map(|v| v.windows(2).map(|w| w[0] == w[1]).collect())
        .collect()
}

/// A line of nodes: margins of the shapes that cut it (sorted by start), position and identifier
/// of each node, links between consecutive nodes.
struct Line<'a> {
    margins: &'a [u32],
    pos: &'a [f64],
    ids: &'a [u32],
    link: &'a [bool],
    vertical: bool,
}

/// Neighbour and length run in a margin, per node and direction.
struct Neighbours {
    nbr: Vec<[u32; 4]>,
    nsoft: Vec<[f64; 4]>,
}

impl Neighbours {
    /// Links the consecutive nodes of a line; the length in a margin is the sum of the edge's overlaps
    /// with the margins (the margins wholly before the edge are skipped, thanks to the running
    /// maximum of their ends).
    fn link_line(&mut self, items: &[Item], line: &Line) {
        use crate::geom::Dir;
        let (fwd, back) = if line.vertical {
            (Dir::Down, Dir::Up)
        } else {
            (Dir::Right, Dir::Left)
        };
        let span = |k: u32| {
            let s = &items[k as usize].soft;
            if line.vertical {
                (s.y0(), s.y1())
            } else {
                (s.x0(), s.x1())
            }
        };
        let mut pmax = Vec::with_capacity(line.margins.len());
        let mut m = f64::NEG_INFINITY;
        for &k in line.margins {
            m = m.max(span(k).1);
            pmax.push(m);
        }
        let mut start = 0;
        for k in 0..line.link.len() {
            if !line.link[k] {
                continue;
            }
            let (lo, hi) = (line.pos[k], line.pos[k + 1]);
            while start < line.margins.len() && pmax[start] <= lo {
                start += 1;
            }
            let mut buf = 0.0;
            for &q in &line.margins[start..] {
                let (s0, s1) = span(q);
                if s0 >= hi {
                    break;
                }
                buf += (hi.min(s1) - lo.max(s0)).max(0.0);
            }
            let (a, b) = (line.ids[k] as usize, line.ids[k + 1] as usize);
            self.nbr[a][fwd.index()] = b as u32;
            self.nbr[b][back.index()] = a as u32;
            self.nsoft[a][fwd.index()] = buf;
            self.nsoft[b][back.index()] = buf;
        }
    }
}

impl Scene {
    /// `extra_x` / `extra_y`: imposed coordinates (ports, stubs, waypoints).
    pub fn new(
        items: Vec<Item>,
        buffer: f64,
        extra_x: Vec<f64>,
        extra_y: Vec<f64>,
        points: &[Point],
        sparse: bool,
    ) -> Scene {
        let (xs, ys) = grid_lines(&items, buffer, extra_x, extra_y);
        let buckets = Buckets::new(&items);
        let mut sc = Scene {
            items,
            xs,
            ys,
            buckets,
            vsoft: Vec::new(),
            hsoft: Vec::new(),
            regions: Vec::new(),
            vnodes: Vec::new(),
            vlink: Vec::new(),
            hnodes: Vec::new(),
            hlink: Vec::new(),
            vids: Vec::new(),
            id_ij: Vec::new(),
            nbr: Vec::new(),
            nsoft: Vec::new(),
        };
        sc.precompute();
        if sparse {
            sc.build_sparse(buffer, points);
        }
        sc
    }

    /// Hard rectangles of the shapes (regions do not block visibility).
    fn shape_rects(&self) -> Vec<Rect> {
        self.items
            .iter()
            .filter(|i| i.wall())
            .map(|i| i.hard)
            .collect()
    }

    /// Builds the sparse graph from the interesting points.
    fn build_sparse(&mut self, buffer: f64, extra: &[Point]) {
        let shapes = self.shape_rects();
        let pts = self.interesting_points(&shapes, buffer, extra);
        let (vcov, hcov) = self.line_coverage(&shapes, pts);
        self.build_nodes(&vcov, &hcov);
        self.assign_ids();
        self.build_neighbours();
    }

    /// Points that cast segments: corners (with and without margin), imposed points, middles of
    /// corridors, frame.
    fn interesting_points(&self, shapes: &[Rect], buffer: f64, extra: &[Point]) -> Vec<Point> {
        let mut pts: Vec<Point> = extra.to_vec();
        for it in &self.items {
            for r in [it.hard, it.soft] {
                pts.extend([
                    (r.x0(), r.y0()),
                    (r.x1(), r.y0()),
                    (r.x0(), r.y1()),
                    (r.x1(), r.y1()),
                ]);
            }
        }
        for c in corridors(shapes, buffer) {
            match c {
                Corridor::Vertical { x, y0, y1 } => pts.extend([(x, y0), (x, y1)]),
                Corridor::Horizontal { y, x0, x1 } => pts.extend([(x0, y), (x1, y)]),
            }
        }
        if let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) = (
            self.xs.first(),
            self.xs.last(),
            self.ys.first(),
            self.ys.last(),
        ) {
            pts.extend([(x0, y0), (x1, y0), (x0, y1), (x1, y1)]);
        }
        pts
    }

    /// Intervals covered on each vertical then horizontal line: the segment cast by each point stops
    /// at the first shape it meets.
    #[allow(clippy::type_complexity)]
    fn line_coverage(
        &self,
        shapes: &[Rect],
        pts: Vec<Point>,
    ) -> (Vec<Vec<(f64, f64)>>, Vec<Vec<(f64, f64)>>) {
        let (nx, ny) = (self.xs.len(), self.ys.len());
        let mut vcov: Vec<Vec<(f64, f64)>> = vec![Vec::new(); nx];
        let mut hcov: Vec<Vec<(f64, f64)>> = vec![Vec::new(); ny];
        let (ymin, ymax) = (
            self.ys.first().copied().unwrap_or(0.0),
            self.ys.last().copied().unwrap_or(0.0),
        );
        let (xmin, xmax) = (
            self.xs.first().copied().unwrap_or(0.0),
            self.xs.last().copied().unwrap_or(0.0),
        );
        let (vshapes, hshapes) = self.shapes_by_line(shapes);
        for p in pts {
            let p = (snap(p.0), snap(p.1));
            // every interesting point is on a line of the grid
            let (Some(i), Some(j)) = (self.ix(p.0), self.iy(p.1)) else {
                continue;
            };
            if vshapes[i].iter().any(|&k| shapes[k].contains_strict(p)) {
                continue;
            }
            let mut span = (ymin, ymax);
            for &k in &vshapes[i] {
                clip_span(&mut span, shapes[k].y0(), shapes[k].y1(), p.1);
            }
            vcov[i].push(span);
            let mut span = (xmin, xmax);
            for &k in &hshapes[j] {
                clip_span(&mut span, shapes[k].x0(), shapes[k].x1(), p.0);
            }
            hcov[j].push(span);
        }
        vcov.iter_mut().for_each(merge_intervals);
        hcov.iter_mut().for_each(merge_intervals);
        (vcov, hcov)
    }

    /// Shapes cut by each line (strict inside), so as not to walk them all.
    fn shapes_by_line(&self, shapes: &[Rect]) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let mut vshapes: Vec<Vec<usize>> = vec![Vec::new(); self.xs.len()];
        let mut hshapes: Vec<Vec<usize>> = vec![Vec::new(); self.ys.len()];
        for (k, r) in shapes.iter().enumerate() {
            for v in &mut vshapes[lines_strictly_inside(&self.xs, r.x0(), r.x1())] {
                v.push(k);
            }
            for h in &mut hshapes[lines_strictly_inside(&self.ys, r.y0(), r.y1())] {
                h.push(k);
            }
        }
        (vshapes, hshapes)
    }

    /// Nodes (intersections of two covered lines) and links between consecutive nodes of one
    /// interval.
    fn build_nodes(&mut self, vcov: &[Vec<(f64, f64)>], hcov: &[Vec<(f64, f64)>]) {
        let (nx, ny) = (self.xs.len(), self.ys.len());
        let mut vnodes = vec![Vec::new(); nx];
        let mut vint: Vec<Vec<usize>> = vec![Vec::new(); nx];
        let mut hnodes: Vec<Vec<u32>> = vec![Vec::new(); ny];
        let mut hint: Vec<Vec<usize>> = vec![Vec::new(); ny];
        for i in 0..nx {
            for (k, &(a, b)) in vcov[i].iter().enumerate() {
                let j0 = self.ys.partition_point(|&y| y < a - 1e-9);
                for j in j0..ny {
                    if self.ys[j] > b + 1e-9 {
                        break;
                    }
                    if let Some(hk) = interval_at(&hcov[j], self.xs[i]) {
                        vnodes[i].push(j as u32);
                        vint[i].push(k);
                        hnodes[j].push(i as u32);
                        hint[j].push(hk);
                    }
                }
            }
        }
        // the horizontal nodes were added by increasing i: already sorted
        self.vlink = same_interval_links(&vint);
        self.hlink = same_interval_links(&hint);
        self.vnodes = vnodes;
        self.hnodes = hnodes;
    }

    /// Identifier of each node (order of the vertical lines) and the reverse table.
    fn assign_ids(&mut self) {
        let mut vids = Vec::with_capacity(self.xs.len());
        let mut id_ij = Vec::new();
        for (i, v) in self.vnodes.iter().enumerate() {
            let mut ids = Vec::with_capacity(v.len());
            for &j in v {
                ids.push(id_ij.len() as u32);
                id_ij.push((i as u32, j));
            }
            vids.push(ids);
        }
        self.vids = vids;
        self.id_ij = id_ij;
    }

    /// Neighbours and lengths in margins, line by line (nodes sorted, consecutive links).
    fn build_neighbours(&mut self) {
        let n = self.id_ij.len();
        let mut graph = Neighbours {
            nbr: vec![[u32::MAX; 4]; n],
            nsoft: vec![[0.0; 4]; n],
        };
        // `hids[j][k]`: identifier of the k-th node of horizontal line `j`
        let mut hids: Vec<Vec<u32>> = vec![Vec::new(); self.ys.len()];
        for (id, &(_, j)) in self.id_ij.iter().enumerate() {
            hids[j as usize].push(id as u32);
        }
        let mut pos = Vec::new();
        for i in 0..self.xs.len() {
            pos.clear();
            pos.extend(self.vnodes[i].iter().map(|&j| self.ys[j as usize]));
            graph.link_line(
                &self.items,
                &Line {
                    margins: &self.vsoft[i],
                    pos: &pos,
                    ids: &self.vids[i],
                    link: &self.vlink[i],
                    vertical: true,
                },
            );
        }
        for (j, ids) in hids.iter().enumerate() {
            pos.clear();
            pos.extend(self.hnodes[j].iter().map(|&i| self.xs[i as usize]));
            graph.link_line(
                &self.items,
                &Line {
                    margins: &self.hsoft[j],
                    pos: &pos,
                    ids,
                    link: &self.hlink[j],
                    vertical: false,
                },
            );
        }
        self.nbr = graph.nbr;
        self.nsoft = graph.nsoft;
    }

    /// Identifier of node `(i, j)`, if it exists.
    pub fn id(&self, i: usize, j: usize) -> Option<u32> {
        let v = self.vnodes.get(i)?;
        let k = v.binary_search(&(j as u32)).ok()?;
        Some(self.vids[i][k])
    }

    /// Coordinates (indices of lines) of a node.
    pub fn ij(&self, id: u32) -> (usize, usize) {
        let (i, j) = self.id_ij[id as usize];
        (i as usize, j as usize)
    }

    /// Neighbour of a node in a direction.
    pub fn next(&self, id: u32, d: crate::geom::Dir) -> Option<u32> {
        let n = self.nbr[id as usize][d.index()];
        (n != u32::MAX).then_some(n)
    }

    /// Number of nodes (identifiers from 0 to n − 1).
    pub fn n_nodes(&self) -> usize {
        self.id_ij.len()
    }

    /// Number of nodes of the sparse graph (statistics, `ROUTER_STATS`).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn active_nodes(&self) -> usize {
        self.vnodes.iter().map(Vec::len).sum()
    }

    /// Edge of node `id` in direction `d`, to `next`: (blocked by an active region, length run in a
    /// margin, length run inside crossed regions).
    pub fn span(
        &self,
        id: u32,
        d: crate::geom::Dir,
        next: u32,
        active: &[bool],
    ) -> (bool, f64, f64) {
        let a = self.ij(id);
        let b = self.ij(next);
        let pa = (self.xs[a.0], self.ys[a.1]);
        let pb = (self.xs[b.0], self.ys[b.1]);
        let mut buf = self.nsoft[id as usize][d.index()];
        let mut inside = 0.0;
        for &r in &self.regions {
            let it = &self.items[r];
            if !active[r] && it.kind == ObstacleKind::Shape {
                // a shape this connector ignores: as if absent
                continue;
            }
            if !active[r] {
                // crossed region: length inside (for `region_cost`)
                let h = &it.hard;
                inside += if a.0 == b.0 {
                    // border included: running along a region's border costs as much as crossing it
                    if h.x0() <= pa.0 && pa.0 <= h.x1() {
                        (pa.1.max(pb.1).min(h.y1()) - pa.1.min(pb.1).max(h.y0())).max(0.0)
                    } else {
                        0.0
                    }
                } else if h.y0() <= pa.1 && pa.1 <= h.y1() {
                    (pa.0.max(pb.0).min(h.x1()) - pa.0.min(pb.0).max(h.x0())).max(0.0)
                } else {
                    0.0
                };
                continue;
            }
            if it.hard.hit_by_segment(pa, pb) {
                return (true, buf, inside);
            }
            // part of the stretch in the region's margin
            let s = &it.soft;
            let overlap = if a.0 == b.0 {
                if s.x0() < pa.0 && pa.0 < s.x1() {
                    (pa.1.max(pb.1).min(s.y1()) - pa.1.min(pb.1).max(s.y0())).max(0.0)
                } else {
                    0.0
                }
            } else if s.y0() < pa.1 && pa.1 < s.y1() {
                (pa.0.max(pb.0).min(s.x1()) - pa.0.min(pb.0).max(s.x0())).max(0.0)
            } else {
                0.0
            };
            buf += overlap;
        }
        (false, buf, inside)
    }

    /// Precomputation: regions, and shapes whose margin cuts each line.
    fn precompute(&mut self) {
        let (nx, ny) = (self.xs.len(), self.ys.len());
        // regions, and the shapes some connector ignores: checked per connector in `span`
        self.regions = (0..self.items.len())
            .filter(|&k| !self.items[k].wall())
            .collect();
        let mut vsoft: Vec<Vec<u32>> = vec![Vec::new(); nx];
        let mut hsoft: Vec<Vec<u32>> = vec![Vec::new(); ny];
        for (k, it) in self.items.iter().enumerate().filter(|(_, it)| it.wall()) {
            let s = it.soft;
            for v in &mut vsoft[lines_strictly_inside(&self.xs, s.x0(), s.x1())] {
                v.push(k as u32);
            }
            for h in &mut hsoft[lines_strictly_inside(&self.ys, s.y0(), s.y1())] {
                h.push(k as u32);
            }
        }
        // sorted by start, to stop the walk as soon as the stretch is passed
        let items = &self.items;
        for v in &mut vsoft {
            v.sort_by(|&a, &b| {
                items[a as usize]
                    .soft
                    .y0()
                    .total_cmp(&items[b as usize].soft.y0())
            });
        }
        for h in &mut hsoft {
            h.sort_by(|&a, &b| {
                items[a as usize]
                    .soft
                    .x0()
                    .total_cmp(&items[b as usize].soft.x0())
            });
        }
        self.vsoft = vsoft;
        self.hsoft = hsoft;
    }

    /// Index of a line's coordinate (it must have been given to `new`).
    pub fn ix(&self, x: f64) -> Option<usize> {
        self.xs.binary_search_by(|v| v.total_cmp(&snap(x))).ok()
    }
    /// Index of a horizontal line's coordinate.
    pub fn iy(&self, y: f64) -> Option<usize> {
        self.ys.binary_search_by(|v| v.total_cmp(&snap(y))).ok()
    }

    /// Does the orthogonal segment `a-b` touch the inside of an active obstacle?
    /// `active[k]`: obstacle `k` blocks this connector.
    pub fn blocked(&self, a: Point, b: Point, active: &[bool]) -> bool {
        let (i0, j0, i1, j1) =
            self.buckets
                .range(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
        for j in j0..=j1 {
            for i in i0..=i1 {
                for &k in &self.buckets.cells[j * self.buckets.nx + i] {
                    let k = k as usize;
                    if active[k] && self.items[k].hard.hit_by_segment(a, b) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Is `p` strictly inside an active obstacle?
    pub fn inside(&self, p: Point, active: &[bool]) -> bool {
        let (i, j, _, _) = self.buckets.range(p.0, p.1, p.0, p.1);
        self.buckets.cells[j * self.buckets.nx + i]
            .iter()
            .any(|&k| active[k as usize] && self.items[k as usize].hard.contains_strict(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictly_inside_excludes_the_edges_and_survives_flat_obstacles() {
        let lines = [0.0, 10.0, 20.0, 30.0];
        assert_eq!(lines_strictly_inside(&lines, 0.0, 30.0), 1..3);
        assert_eq!(lines_strictly_inside(&lines, 10.0, 20.0), 2..2);
        // obstacle without thickness (zero width): an empty range, no panic
        assert!(lines_strictly_inside(&lines, 10.0, 10.0).is_empty());
    }

    #[test]
    fn clip_span_stops_at_the_nearest_shape_on_each_side() {
        let mut span = (0.0, 100.0);
        clip_span(&mut span, 10.0, 20.0, 50.0); // shape before the point
        clip_span(&mut span, 70.0, 90.0, 50.0); // shape after the point
        assert_eq!(span, (20.0, 70.0));
    }

    #[test]
    fn merge_intervals_joins_touching_ones_only() {
        let mut v = vec![(5.0, 8.0), (0.0, 5.0), (10.0, 12.0)];
        merge_intervals(&mut v);
        assert_eq!(v, vec![(0.0, 8.0), (10.0, 12.0)]);
    }

    #[test]
    fn interval_lookup_tolerates_rounding() {
        let v = [(0.0, 5.0), (10.0, 20.0)];
        assert_eq!(interval_at(&v, 5.0 + 1e-12), Some(0));
        assert_eq!(interval_at(&v, 7.0), None);
        assert_eq!(interval_at(&v, 15.0), Some(1));
    }

    #[test]
    fn corridor_midline_is_equidistant() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(60.0, 0.0, 50.0, 50.0);
        let c = corridors(&[a, b], 10.0);
        assert!(matches!(c.as_slice(), [Corridor::Vertical { x, .. }] if (*x - 55.0).abs() < 1e-9));
    }
}
