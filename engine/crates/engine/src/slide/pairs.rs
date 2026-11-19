//! The rules between two lines: a hard conflict, the crossings, and the price of a trunk shared
//! by lines saying different things.

use super::Pt;
use super::grid::{Role, Route};

/// Lines saying different things sharing a trunk (H2.20: a last resort): a bend…
pub(crate) const W_MERGE: f64 = 40.0;
/// … and each step of it, four steps of length (as late as possible).
pub(crate) const W_MERGE_STEP: f64 = 4.0;
/// Two arrows joined (`join=`) that do not share their trunk.
pub(crate) const W_JOIN: f64 = 300.0;

/// What the pair checks need of an arrow beside its route.
#[derive(Clone, Copy)]
pub(crate) struct Who<'a> {
    pub look: &'a str,
    pub src: usize,
    pub dst: usize,
    pub label: Option<&'a str>,
    pub join: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PairRes {
    pub bad: bool,
    pub cross: u32,
    pub merge: f64,
}

const BAD: PairRes = PairRes {
    bad: true,
    cross: 0,
    merge: 0.0,
};

fn adjacent(p: Pt, q: Pt) -> bool {
    (p.0 == q.0 && (p.1 - q.1).abs() == 1) || (p.1 == q.1 && (p.0 - q.0).abs() == 1)
}

/// At node `p` of route `r`, which side its other stretch leaves the axis to.
fn off_side(r: &Route, p: Pt, vertical: bool) -> Option<bool> {
    let (k, _) = r.find(p)?;
    let n = &r.nodes;
    let near = [k.checked_sub(1).map(|i| n[i]), n.get(k + 1).copied()];
    for w in near.into_iter().flatten() {
        let d = if vertical { w.0 - p.0 } else { w.1 - p.1 };
        if d != 0 {
            return Some(d > 0);
        }
    }
    None
}

/// Route `r` runs from `x` along axis `ax`, in the direction opposite to `s`.
fn runs_away(r: &Route, x: Pt, s: i32, ax: usize) -> bool {
    let Some((k, _)) = r.find(x) else {
        return false;
    };
    let n = &r.nodes;
    let c = |p: Pt, a: usize| if a == 0 { p.0 } else { p.1 };
    [k.checked_sub(1).map(|i| n[i]), n.get(k + 1).copied()]
        .into_iter()
        .flatten()
        .any(|w| c(w, 1 - ax) == c(x, 1 - ax) && (c(w, ax) - c(x, ax)) * s < 0)
}

/// The nodes of `r` from its start, or from its end when `reverse`.
fn ordered(r: &Route, reverse: bool) -> Vec<Pt> {
    if reverse {
        r.nodes.iter().rev().copied().collect()
    } else {
        r.nodes.clone()
    }
}

/// The nodes two routes both run through, in key order, with each one's role there.
fn shared<'r>(a: &'r Route, b: &'r Route) -> impl Iterator<Item = (u32, Role, Role)> + 'r {
    let (mut i, mut j) = (0, 0);
    std::iter::from_fn(move || {
        while i < a.keys.len() && j < b.keys.len() {
            let (ka, kb) = (a.keys[i], b.keys[j]);
            match ka.0.cmp(&kb.0) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    i += 1;
                    j += 1;
                    return Some((ka.0, ka.2, kb.2));
                }
            }
        }
        None
    })
}

/// Two straight stretches of different axes: a crossing.
fn crosses(ra: Role, rb: Role) -> bool {
    matches!(ra, Role::H | Role::V) && matches!(rb, Role::H | Role::V) && ra != rb
}

/// Two routes sharing a start port (or an end port, `reverse`) drawn alike may share their first
/// stretch, a trunk, and part once and for good, not right after a bend (H2.16).
fn trunk_ok(a: &Route, b: &Route, reverse: bool) -> (bool, usize) {
    let (na, nb) = (ordered(a, reverse), ordered(b, reverse));
    let k = na.iter().zip(&nb).take_while(|(p, q)| p == q).count();
    let prefix = &na[..k];
    let shared_ok = a.keys.iter().all(|e| {
        let p = super::grid::unkey(e.0);
        !b.has(p) || prefix.contains(&p)
    });
    if !shared_ok {
        return (false, k);
    }
    if k >= 2 {
        let d = k - 1;
        for (i, &p) in na.iter().enumerate().take(d).skip(1) {
            if a.find(p).is_some_and(|f| f.1 == Role::Bend) && d - i < 2 {
                return (false, k);
            }
        }
    }
    (true, k)
}

/// The rules between routes `a` and `b` of two different arrows.
pub(crate) fn pair(a: &Route, wa: Who, b: &Route, wb: Who) -> PairRes {
    let Some((cross, k)) = check(a, wa, b, wb) else {
        return BAD;
    };
    // joined arrows (`join=`) share their trunk: out of one card for `trunk`, into one for a name
    let joined = wa.join.is_some() && wa.join == wb.join;
    let merge = if joined {
        let apart = if wa.join == Some("trunk") {
            wa.src == wb.src && a.src() != b.src()
        } else {
            wa.dst == wb.dst && a.dst() != b.dst()
        };
        if apart { W_JOIN } else { 0.0 }
    } else if k >= 2 && wa.label != wb.label {
        W_MERGE + W_MERGE_STEP * (k - 1) as f64
    } else {
        0.0
    };
    PairRes {
        bad: false,
        cross,
        merge,
    }
}

/// The crossings and the trunk length of two routes, or `None` when they break a hard rule.
fn check(a: &Route, wa: Who, b: &Route, wb: Who) -> Option<(u32, usize)> {
    let (ba, bb) = (a.bbox, b.bbox);
    if ba.2 + 2 < bb.0 || bb.2 + 2 < ba.0 || ba.3 + 2 < bb.1 || bb.3 + 2 < ba.1 {
        return Some((0, 0));
    }
    // drawn alike, or joined (`join=`): their lines may share a trunk
    let same = wa.look == wb.look || (wa.join.is_some() && wa.join == wb.join);
    let rel_s = same && wa.src == wb.src && a.src() == b.src();
    let rel_d = same && wa.dst == wb.dst && a.dst() == b.dst();
    let rel = rel_s || rel_d;
    let mut k = 0;
    let mut skip: Vec<Pt> = Vec::new();
    let na = ordered(a, rel && !rel_s);
    if rel {
        let (ok, kk) = trunk_ok(a, b, !rel_s);
        if !ok {
            return None;
        }
        k = kk;
        skip.extend_from_slice(&na[..k]);
    }
    let crossings = crossings_off_trunk(a, b, &skip)?;
    if crossings >= 2 && !rel {
        return None; // crossing twice (H2.21)
    }
    if crossings > 0
        && ((wa.src == wb.src && a.sside == b.sside) || (wa.dst == wb.dst && a.tside == b.tside))
    {
        return None; // two lines of one side crossing: ports swapped (H2.21)
    }
    let branch = (rel && k >= 1).then(|| na[k - 1]);
    if bends_too_close(a, b, &skip, branch) || (!rel && ends_too_close(a, b)) {
        return None;
    }
    Some((crossings, if rel { k } else { 0 }))
}

/// The crossings of two routes off their trunk, or `None` when they meet otherwise: on one
/// another, a bend or an end on the other line, or a crossing next to a head (H2.10).
fn crossings_off_trunk(a: &Route, b: &Route, skip: &[Pt]) -> Option<u32> {
    let mut crossings = 0u32;
    for (key, ra, rb) in shared(a, b) {
        let p = super::grid::unkey(key);
        if skip.contains(&p) {
            continue;
        }
        if !crosses(ra, rb) {
            return None;
        }
        if p == a.nodes[a.nodes.len() - 2] || p == b.nodes[b.nodes.len() - 2] {
            return None;
        }
        crossings += 1;
    }
    Some(crossings)
}

/// Bends and branchings of different lines a step apart on one axis (H2.18), unless both turn
/// the same way into lanes a step apart; where two trunk mates part, the one turning counts.
fn bends_too_close(a: &Route, b: &Route, skip: &[Pt], branch: Option<Pt>) -> bool {
    let inner = |r: &Route| -> Vec<Pt> {
        r.pts[1..r.pts.len() - 1]
            .iter()
            .copied()
            .filter(|p| !skip.contains(p) || Some(*p) == branch)
            .collect()
    };
    let (ea, eb) = (inner(a), inner(b));
    ea.iter().any(|&p| {
        eb.iter().any(|&q| {
            adjacent(p, q) && {
                let vertical = p.0 == q.0;
                off_side(a, p, vertical) != off_side(b, q, vertical)
            }
        })
    })
}

/// Two unrelated lines end to end on one line, a step apart (H2.5), unless they turn alike.
fn ends_too_close(a: &Route, b: &Route) -> bool {
    let (na, nb) = (a.pts.len(), b.pts.len());
    for (ia, &p) in a.pts.iter().enumerate() {
        for (ib, &q) in b.pts.iter().enumerate() {
            if !adjacent(p, q) {
                continue;
            }
            let ax = usize::from(p.0 == q.0);
            let (pc, qc) = if ax == 0 { (p.0, q.0) } else { (p.1, q.1) };
            let sgn = if qc > pc { 1 } else { -1 };
            if runs_away(a, p, sgn, ax) && runs_away(b, q, -sgn, ax) {
                let inner_a = ia > 0 && ia + 1 < na;
                let inner_b = ib > 0 && ib + 1 < nb;
                if inner_a && inner_b && off_side(a, p, ax == 1) == off_side(b, q, ax == 1) {
                    continue;
                }
                return true;
            }
        }
    }
    false
}

/// Crossings between the routes drawn.
pub(crate) fn count_crossings(routes: &[&Route]) -> usize {
    let mut n = 0;
    for i in 0..routes.len() {
        for j in i + 1..routes.len() {
            n += shared(routes[i], routes[j])
                .filter(|&(_, ra, rb)| crosses(ra, rb))
                .count();
        }
    }
    n
}
