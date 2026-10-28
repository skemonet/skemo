//! A better writing order for a generated source (D-065): the engine keeps the writing order of
//! siblings in a row, so the order a generator writes decides much of the drawing. A hill climb
//! swaps sibling cards and zones while the drawing gets better (fewer crossings first, then fewer
//! bends), from a barycentre sweep, with kicks out of its local optima (D-070). For programs that write skemo text (importers), once: layout never reorders by itself, and the file
//! stays what the author edits from then on.

use crate::Options;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;

/// Rounds of the climb at most (each applies the best swap of the round).
const ROUNDS: usize = 40;

/// Kicks tried from each local optimum: its least bad swaps that add a crossing (a swap that
/// only adds bends leads back to the same optimum).
const KICKS: usize = 5;

/// The layouts the kicks may spend, in rounds of swaps (a round lays out every swap once).
const BUDGET_ROUNDS: usize = 8;

/// A line of the source and the lines indented under it.
#[derive(Clone)]
struct Block {
    line: String,
    kids: Vec<Block>,
}

/// The source as a tree of blocks, by indentation.
fn parse(src: &str) -> Block {
    let mut root = Block {
        line: String::new(),
        kids: Vec::new(),
    };
    // path of (indent, index) from the root to the last block
    let mut path: Vec<(usize, usize)> = Vec::new();
    for line in src.lines() {
        let indent = line.len() - line.trim_start().len();
        while path.last().is_some_and(|(i, _)| *i >= indent) {
            path.pop();
        }
        let mut node = &mut root;
        for (_, k) in &path {
            node = &mut node.kids[*k];
        }
        node.kids.push(Block {
            line: line.to_owned(),
            kids: Vec::new(),
        });
        path.push((indent, node.kids.len() - 1));
    }
    root
}

fn dump(b: &Block, out: &mut String) {
    for k in &b.kids {
        out.push_str(&k.line);
        out.push('\n');
        dump(k, out);
    }
}

/// A card or a zone: what the engine lays out in writing order.
fn movable(b: &Block) -> bool {
    let t = b.line.trim_start();
    ["cards", "card", "zone"].iter().any(|k| {
        t.strip_prefix(k)
            .is_some_and(|r| r.starts_with(['#', '.', '(', ' ']) || r.is_empty())
    })
}

/// Every possible swap: (path to the parent block, i, j) for two movable siblings that both
/// hold an end of an arrow (`linked`): the order of the others changes no crossing.
fn swaps(
    b: &Block,
    path: &mut Vec<usize>,
    linked: &HashSet<String>,
    out: &mut Vec<(Vec<usize>, usize, usize)>,
) {
    let holds_one = |k: &Block| {
        let mut mine = Vec::new();
        ids(k, &mut mine);
        mine.iter().any(|id| linked.contains(id))
    };
    let idx: Vec<usize> = (0..b.kids.len())
        .filter(|&i| movable(&b.kids[i]) && holds_one(&b.kids[i]))
        .collect();
    for (a, &i) in idx.iter().enumerate() {
        for &j in &idx[a + 1..] {
            out.push((path.clone(), i, j));
        }
    }
    for (k, kid) in b.kids.iter().enumerate() {
        path.push(k);
        swaps(kid, path, linked, out);
        path.pop();
    }
}

fn swapped(root: &Block, (path, i, j): &(Vec<usize>, usize, usize)) -> Block {
    let mut r = root.clone();
    let mut node = &mut r;
    for k in path {
        node = &mut node.kids[*k];
    }
    node.kids.swap(*i, *j);
    r
}

type Seg = ((f64, f64), (f64, f64));

/// A horizontal and a vertical segment crossing strictly inside both.
fn crosses(a: Seg, b: Seg) -> bool {
    let h = |s: Seg| (s.0.1 - s.1.1).abs() < 0.5;
    let (h1, v1) = match (h(a), h(b)) {
        (true, false) => (a, b),
        (false, true) => (b, a),
        _ => return false,
    };
    let (x0, x1) = (h1.0.0.min(h1.1.0), h1.0.0.max(h1.1.0));
    let (y0, y1) = (v1.0.1.min(v1.1.1), v1.0.1.max(v1.1.1));
    let (x, y) = (v1.0.0, h1.0.1);
    x > x0 + 0.5 && x < x1 - 0.5 && y > y0 + 0.5 && y < y1 - 0.5
}

/// Score of a drawing (lower is better): crossings weigh most, then bends.
fn score(routes: &BTreeMap<String, Vec<(f64, f64)>>) -> f64 {
    let segs: Vec<Vec<Seg>> = routes
        .values()
        .map(|p| p.windows(2).map(|w| (w[0], w[1])).collect())
        .collect();
    let mut crossings = 0;
    for (a, sa) in segs.iter().enumerate() {
        for sb in &segs[a + 1..] {
            crossings += sa
                .iter()
                .map(|&s| sb.iter().filter(|&&t| crosses(s, t)).count())
                .sum::<usize>();
        }
    }
    let bends: usize = routes.values().map(|p| p.len().saturating_sub(2)).sum();
    100.0 * crossings as f64 + 2.0 * bends as f64
}

/// What a warning of the layout costs: an order that hides a card behind others (`wrap`) or
/// draws something the writer is told to change is worse than ten crossings.
const WARNING: f64 = 1000.0;

/// Score of the drawing of `src` (lower is better), infinite if it does not build.
pub fn evaluate(src: &str, opts: &Options) -> f64 {
    crate::from_source(src, opts).map_or(f64::INFINITY, |(d, _)| {
        score(&d.routes) + WARNING * d.warnings.len() as f64
    })
}

fn text(b: &Block) -> String {
    let mut s = String::new();
    dump(b, &mut s);
    s
}

/// The `#id` of a card or zone line.
fn id_of(b: &Block) -> Option<&str> {
    let t = b.line.trim_start();
    let r = &t[t.find('#')? + 1..];
    let end = r.find(['.', '(', ' ']).unwrap_or(r.len());
    let head = &t[..t.find('#')?];
    (!head.contains(' ')).then_some(&r[..end])
}

fn ids(b: &Block, out: &mut Vec<String>) {
    if let Some(id) = id_of(b) {
        out.push(id.to_owned());
    }
    for k in &b.kids {
        ids(k, out);
    }
}

/// One barycentre sweep (Sugiyama's heuristic): the movable siblings of every block sorted by
/// the mean place of what they link to outside themselves, along the axis they are spread on in
/// the drawing of `root`. A sibling with no such link keeps its own place.
fn barycentre(root: &Block, opts: &Options) -> Option<Block> {
    let (d, _) = crate::from_source(&text(root), opts).ok()?;
    let centre = |id: &str| d.boxes.get(id).map(|b| (b.x + b.w / 2.0, b.y + b.h / 2.0));
    let mut out = root.clone();
    sort_kids(&mut out, &d.model.edges, &centre);
    Some(out)
}

fn sort_kids(
    b: &mut Block,
    edges: &[skemo_lang::Edge],
    centre: &dyn Fn(&str) -> Option<(f64, f64)>,
) {
    let idx: Vec<usize> = (0..b.kids.len()).filter(|&i| movable(&b.kids[i])).collect();
    let own: Vec<Option<(f64, f64)>> = idx
        .iter()
        .map(|&i| id_of(&b.kids[i]).and_then(centre))
        .collect();
    let spread = |f: fn(&(f64, f64)) -> f64| {
        let v: Vec<f64> = own.iter().flatten().map(f).collect();
        v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - v.iter().copied().fold(f64::INFINITY, f64::min)
    };
    if idx.len() > 1 {
        let x_axis = spread(|c| c.0) >= spread(|c| c.1);
        let pick = |c: (f64, f64)| if x_axis { c.0 } else { c.1 };
        let mut keyed: Vec<(f64, usize)> = idx
            .iter()
            .zip(&own)
            .map(|(&i, c)| {
                let mut mine = Vec::new();
                ids(&b.kids[i], &mut mine);
                let others: Vec<f64> = edges
                    .iter()
                    .filter_map(|e| match (mine.contains(&e.from), mine.contains(&e.to)) {
                        (true, false) => centre(&e.to),
                        (false, true) => centre(&e.from),
                        _ => None,
                    })
                    .map(pick)
                    .collect();
                let k = if others.is_empty() {
                    c.map_or(f64::INFINITY, pick)
                } else {
                    others.iter().sum::<f64>() / others.len() as f64
                };
                (k, i)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        let blocks: Vec<Block> = keyed.iter().map(|&(_, i)| b.kids[i].clone()).collect();
        for (&i, blk) in idx.iter().zip(blocks) {
            b.kids[i] = blk;
        }
    }
    for k in &mut b.kids {
        sort_kids(k, edges, centre);
    }
}

type Swap = (Vec<usize>, usize, usize);

/// The search's state: the options, and the score of every source already laid out (a kick's
/// climb often meets them again).
struct Search<'a> {
    opts: &'a Options,
    /// The ends of the source's arrows.
    linked: HashSet<String>,
    seen: Mutex<HashMap<String, f64>>,
}

impl Search<'_> {
    fn spent(&self) -> usize {
        self.seen.lock().map_or(0, |m| m.len())
    }

    fn score(&self, src: &str) -> f64 {
        if let Some(s) = self.seen.lock().ok().and_then(|m| m.get(src).copied()) {
            return s;
        }
        let s = evaluate(src, self.opts);
        if let Ok(mut m) = self.seen.lock() {
            m.insert(src.to_owned(), s);
        }
        s
    }

    /// Every swap of `root` but `tabu`, and the score of the drawing it gives, in parallel.
    fn neighbours(&self, root: &Block, tabu: Option<&Swap>) -> Vec<(Swap, f64)> {
        let mut cands = Vec::new();
        swaps(root, &mut Vec::new(), &self.linked, &mut cands);
        cands.retain(|c| Some(c) != tabu);
        let threads = std::thread::available_parallelism().map_or(1, usize::from);
        let chunk = cands.len().div_ceil(threads).max(1);
        let scores: Vec<f64> = std::thread::scope(|s| {
            let handles: Vec<_> = cands
                .chunks(chunk)
                .map(|c| {
                    s.spawn(move || {
                        c.iter()
                            .map(|sw| self.score(&text(&swapped(root, sw))))
                            .collect::<Vec<f64>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        cands.into_iter().zip(scores).collect()
    }

    /// Hill climb from `root` (score `best`): the best swap of each round while it gains; `tabu`
    /// (the kick that led here) is never undone. Returns the local optimum, its score and its
    /// neighbours.
    fn climb(&self, mut root: Block, mut best: f64, tabu: Option<&Swap>) -> Climbed {
        for _ in 0..ROUNDS {
            let near = self.neighbours(&root, tabu);
            // the best swap of the round; ties go to the first one (the same on any machine)
            let Some((sw, s)) = near.iter().min_by(|a, b| a.1.total_cmp(&b.1)).cloned() else {
                return (root, best, near);
            };
            if s >= best {
                return (root, best, near);
            }
            root = swapped(&root, &sw);
            best = s;
        }
        (root, best, Vec::new())
    }
}

type Climbed = (Block, f64, Vec<(Swap, f64)>);

/// The source with its siblings reordered for a better drawing, and the scores before and after.
/// Two starts are climbed, the written order and one barycentre sweep of it; the better is kept.
/// At a local optimum with crossings, its `KICKS` least bad swaps that add one are tried in turn
/// as new starts; the first whose climb ends lower is taken, and the search goes on from there.
/// The kicks' layouts are bounded, so the result is the same on any machine.
pub fn improve(src: &str, opts: &Options) -> (String, f64, f64) {
    let linked = skemo_lang::parse(src)
        .model
        .edges
        .iter()
        .flat_map(|e| [e.from.clone(), e.to.clone()])
        .collect();
    let search = Search {
        opts,
        linked,
        seen: Mutex::default(),
    };
    let start = search.score(src);
    let root = parse(src);
    let mut cands = Vec::new();
    swaps(&root, &mut Vec::new(), &search.linked, &mut cands);
    // two starts, each climbed: the written order and one barycentre sweep of it
    let mut climbed = search.climb(root.clone(), start, None);
    if let Some(b) = barycentre(&root, opts) {
        let s = search.score(&text(&b));
        let c = search.climb(b, s, None);
        if c.1 < climbed.1 {
            climbed = c;
        }
    }
    let budget = search.spent() + BUDGET_ROUNDS * cands.len();
    let (mut root, mut best, mut near) = climbed;
    // kicks trade a crossing for a way out: none left, nothing to gain
    'kicks: while best >= 100.0 {
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (sw, s) in near.iter().filter(|n| n.1 > best + 50.0).take(KICKS) {
            if search.spent() >= budget {
                break 'kicks;
            }
            let c = search.climb(swapped(&root, sw), *s, Some(sw));
            if c.1 < best {
                (root, best, near) = c;
                continue 'kicks;
            }
        }
        break;
    }
    (text(&root), start, best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_dump_keep_the_source() {
        let src = "diagram\n  zone#a A\n    card#x X\n      to(y)\n  card#y Y\n";
        assert_eq!(text(&parse(src)), src);
    }

    #[test]
    fn two_crossed_links_are_written_apart() {
        let src = "diagram(dir=\"down\")\n  card#a A\n    to(y)\n  card#b B\n    to(x)\n  card#x X\n  card#y Y\n";
        let (out, before, after) = improve(src, &Options::default());
        assert!(before >= 100.0, "{before}");
        assert!(after < 100.0, "{after}\n{out}");
    }

    #[test]
    fn only_linked_cards_and_zones_move() {
        let root = parse("diagram\n  title T\n  card#a A\n  cards#b B\n  zone#z Z\n  card#c C\n");
        let linked = ["a", "b", "z"].map(String::from).into_iter().collect();
        let mut out = Vec::new();
        swaps(&root, &mut Vec::new(), &linked, &mut out);
        // the three movable and linked children of `diagram`: three pairs
        assert_eq!(out.len(), 3);
    }
}
