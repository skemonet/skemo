//! Lanes beside the arrow labels (`grid-label-lanes`): a line does not run through another
//! arrow's label when the gap they stand in holds both side by side. The lines that run upright
//! through the place of a label stand beside it, to its left or to its right, as near as they
//! were as the label lets ([`fit`]): the label's room counts them (`room`), and once the labels
//! are placed the lines move ([`labelled`]) and the labels are placed again. Two labels that lie
//! over one another on upright lines side by side part the same way ([`apart`]): a straight line
//! from a card to the card under it slides along the side they share.

use super::finish::{MARGIN, PORT_ROOM, TAIL};
use super::{cards, cuts, on};
use crate::labels::{LANE, LANE_EDGE, PAD, Slot};
use crate::layout::Box2;
use crate::measure::Size;
use crate::route::Points;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

type Routes = BTreeMap<String, Points>;
type Centres = BTreeMap<String, (f64, f64)>;
/// A track moved: its index, its new x.
type Shift = (usize, f64);
/// A line moved: its arrow, the index of its segment, its new x.
type Move = (String, usize, f64);

/// A line may touch a label's box by this much without running through it (`quality`'s own).
const TOUCH: f64 = 1.0;
/// A line closer than this to a zone's border runs along it (`quality`'s `hugging`).
const HUG: f64 = 10.0;
/// How many ways of clearing one label are tried (each one places the labels again).
const TRIES: usize = 6;
/// How many times the labels are gone through: a line moved for one label may free another.
const ROUNDS: usize = 2;

/// What stands in the drawing, for the rule.
pub(crate) struct Around<'a> {
    model: &'a Model,
    /// The cards, with their ids.
    cards: Vec<(&'a str, Box2)>,
    /// The visible zones' frames.
    frames: Vec<Box2>,
    /// The zone titles and their tags.
    titles: &'a [(String, Box2)],
    /// The joins and the forks: arrow id → its group.
    groups: &'a BTreeMap<String, String>,
    /// The labels' sizes.
    sizes: &'a BTreeMap<String, Size>,
}

impl<'a> Around<'a> {
    pub(crate) fn new(
        model: &'a Model,
        boxes: &'a BTreeMap<String, Box2>,
        titles: &'a [(String, Box2)],
        groups: &'a BTreeMap<String, String>,
        sizes: &'a BTreeMap<String, Size>,
    ) -> Self {
        let frames = model
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Zone && !crate::layout::invisible(n))
            .filter_map(|n| boxes.get(&n.id).copied())
            .collect();
        Around {
            model,
            cards: cards(model, boxes),
            frames,
            titles,
            groups,
            sizes,
        }
    }
}

/// An upright stretch of a line.
pub(crate) struct Track<'a> {
    pub line: &'a str,
    /// The segment of the line: from its point `seg` to the next.
    pub seg: usize,
    pub x: f64,
    /// Its top and its bottom.
    pub y: (f64, f64),
    /// How far to the left and to the right it may stand: a stretch between two level ones, of
    /// an arrow that shares no trunk, moves as long as those two keep their direction and some
    /// length (the router's margin out of a card, a tail before a head); any other stays.
    pub reach: (f64, f64),
}

/// Every upright stretch of `routes`.
pub(crate) fn tracks<'a>(routes: &'a Routes, groups: &BTreeMap<String, String>) -> Vec<Track<'a>> {
    let level = |a: (f64, f64), b: (f64, f64)| (a.1 - b.1).abs() < 1e-9;
    let mut out = Vec::new();
    for (line, r) in routes {
        for (seg, s) in r.windows(2).enumerate() {
            let (p, q) = (s[0], s[1]);
            if (p.0 - q.0).abs() > 1e-9 || level(p, q) {
                continue;
            }
            let free = seg >= 1
                && seg + 2 < r.len()
                && level(r[seg - 1], p)
                && level(q, r[seg + 2])
                && !groups.contains_key(line);
            let mut reach = (p.0, p.0);
            if free {
                reach = (f64::NEG_INFINITY, f64::INFINITY);
                let first = if seg == 1 { MARGIN } else { LANE };
                let last = if seg + 3 == r.len() { TAIL } else { LANE };
                for (end, least) in [(r[seg - 1].0, first), (r[seg + 2].0, last)] {
                    if end < p.0 {
                        reach.0 = reach.0.max(end + least);
                    } else {
                        reach.1 = reach.1.min(end - least);
                    }
                }
            }
            out.push(Track {
                line,
                seg,
                x: p.0,
                y: (p.1.min(q.1), p.1.max(q.1)),
                reach,
            });
        }
    }
    out
}

/// Is `line` another arrow than the label's (`of`), and not one of its join or fork, which run
/// under the label they share (D-054)?
pub(crate) fn foreign(groups: &BTreeMap<String, String>, line: &str, of: &str) -> bool {
    let group = groups.get(line);
    line != of && !(group.is_some() && group == groups.get(of))
}

/// How a label and the lines through its place stand side by side.
pub(crate) struct Fit {
    /// The room the label still lacks, the lines standing as far aside as they may (px; none or
    /// less: it fits; infinite: the lines may stand on neither side).
    pub lack: f64,
    /// The ways that leave the label its room, the one that moves the lines the least first:
    /// how far the lines move in all, and `(track, its new x)` for each line that moves.
    pub plans: Vec<(f64, Vec<Shift>)>,
}

/// The label `w` wide in `slot`, and the lines of `tracks` that run upright through it: some go
/// to its left, the others to its right, in the order they stand in. On each side a line may
/// stand `LANE_EDGE` from what bounds the gap, `LANE` from the line before it and from one that
/// stays (and within its `reach`), and never further from that side than it is. `at`: the left
/// edge of the label where it stands now, kept when it can be. `ok`: may the lines stand so
/// (`(track, its new x)`, [`refusal`])? A side the lines may not stand on is no place, however
/// wide the gap gets.
pub(crate) fn fit(
    slot: &Slot,
    w: f64,
    at: Option<f64>,
    tracks: &[&Track],
    ok: &dyn Fn(&[Shift]) -> bool,
) -> Fit {
    let (mut lo, mut hi) = slot.gap;
    let (mut a, mut b) = slot.slide;
    let mut through: Vec<usize> = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        let level = t.y.1 > slot.rows.0 + TOUCH && t.y.0 < slot.rows.1 - TOUCH;
        if !level || t.x <= lo || t.x >= hi {
            continue;
        }
        if t.x <= a + TOUCH {
            // a line beside the label's place already: the lanes start from it
            lo = lo.max(t.x - (LANE_EDGE - LANE));
            a = a.max(t.x + PAD);
        } else if t.x >= b - TOUCH {
            hi = hi.min(t.x + (LANE_EDGE - LANE));
            b = b.min(t.x - PAD);
        } else {
            through.push(i);
        }
    }
    if through.is_empty() {
        return Fit {
            lack: w - (b - a),
            plans: Vec::new(),
        };
    }
    through.sort_by(|i, j| tracks[*i].x.total_cmp(&tracks[*j].x));
    let lanes = Lanes { tracks, through };
    let k = lanes.through.len();
    let mut lack = f64::INFINITY;
    let mut plans: Vec<(f64, Vec<Shift>)> = Vec::new();
    // the first `left` lines to the left of the label, the others to its right
    for left in 0..=k {
        let aside = lanes.aside(left, (lo, hi));
        let from = if left > 0 {
            a.max(aside[left - 1] + PAD)
        } else {
            a
        };
        let to = if left < k {
            b.min(aside[left] - PAD)
        } else {
            b
        };
        if to - from < w {
            // too narrow: what it lacks counts when the lines may stand that far aside
            let far: Vec<Shift> = (0..k)
                .filter(|i| (aside[*i] - lanes.line(*i).x).abs() > 1e-6)
                .map(|i| (lanes.through[i], aside[i]))
                .collect();
            if ok(&far) {
                lack = lack.min(w - (to - from));
            }
            continue;
        }
        // the label where it stands, else at an end of its room; the lines moved the least
        let last = (to - w).max(from);
        for x in [at.unwrap_or(from).clamp(from, last), from, last] {
            let Some((cost, moves)) = lanes.moves_at((x, x + w), left, &aside) else {
                continue;
            };
            if moves.is_empty() {
                // no line under the label there
                lack = lack.min(w - (to - from));
            } else if plans.iter().any(|(_, m)| *m == moves) {
            } else if ok(&moves) {
                lack = lack.min(w - (to - from));
                plans.push((cost, moves));
            }
        }
    }
    plans.sort_by(|a, b| a.0.total_cmp(&b.0));
    Fit { lack, plans }
}

/// The lines running upright through a label's place (`through`, indices of `tracks`, from
/// left to right), among all the tracks of the gap.
struct Lanes<'a, 'b> {
    tracks: &'a [&'a Track<'b>],
    through: Vec<usize>,
}

impl<'b> Lanes<'_, 'b> {
    /// The `i`-th line through the label's place.
    fn line(&self, i: usize) -> &Track<'b> {
        self.tracks[self.through[i]]
    }

    /// `x` for the line `t`, moved on (`dir`: to the right, or to the left) past the lines that
    /// stay and run along it.
    fn past(&self, mut x: f64, t: &Track, dir: f64) -> f64 {
        for _ in 0..self.tracks.len() {
            let near = |(j, o): &(usize, &&Track<'b>)| {
                !self.through.contains(j)
                    && o.line != t.line
                    && (o.x - x).abs() < LANE - 0.5
                    && o.y.1 > t.y.0
                    && o.y.0 < t.y.1
            };
            match self.tracks.iter().enumerate().find(near) {
                Some((_, o)) => x = o.x + dir * LANE,
                None => break,
            }
        }
        x
    }

    /// How far aside each line may stand, the first `left` to the left of the label, the others
    /// to its right, in the gap `(lo, hi)`.
    fn aside(&self, left: usize, (lo, hi): (f64, f64)) -> Vec<f64> {
        let k = self.through.len();
        let mut aside = vec![0.0; k];
        for i in 0..left {
            let t = self.line(i);
            let mut x = (lo + LANE_EDGE).max(t.reach.0);
            if i > 0 {
                x = x.max(aside[i - 1] + LANE);
            }
            aside[i] = self.past(x, t, 1.0).min(t.x);
        }
        for i in (left..k).rev() {
            let t = self.line(i);
            let mut x = (hi - LANE_EDGE).min(t.reach.1);
            if i + 1 < k {
                x = x.min(aside[i + 1] - LANE);
            }
            aside[i] = self.past(x, t, -1.0).max(t.x);
        }
        aside
    }

    /// The lines moved the least so that the label stands over `(x0, x1)`: how far they move in
    /// all and where each moved one goes; none when a line would have to go past its `aside`.
    fn moves_at(
        &self,
        (x0, x1): (f64, f64),
        left: usize,
        aside: &[f64],
    ) -> Option<(f64, Vec<Shift>)> {
        let mut moves = Vec::new();
        let mut cost = 0.0;
        let mut limit = x0 - PAD;
        for i in (0..left).rev() {
            let t = self.line(i);
            if t.x <= limit {
                break;
            }
            let new = self.past(limit.max(aside[i]), t, -1.0);
            if new < aside[i] - 1e-6 {
                return None;
            }
            if t.x - new > 1e-6 {
                moves.push((self.through[i], new));
                cost += t.x - new;
            }
            limit = new - LANE;
        }
        let mut limit = x1 + PAD;
        for (i, &most) in aside.iter().enumerate().skip(left) {
            let t = self.line(i);
            if t.x >= limit {
                break;
            }
            let new = self.past(limit.min(most), t, 1.0);
            if new > most + 1e-6 {
                return None;
            }
            if new - t.x > 1e-6 {
                moves.push((self.through[i], new));
                cost += new - t.x;
            }
            limit = new + LANE;
        }
        Some((cost, moves))
    }
}

/// The labels of `routes` (`place`), then, for each label that a line runs through, or that
/// stands over a card, a frame or another label (`faults`), the lines moved beside a place of
/// its line and the labels placed again; a move is kept only when the labels have fewer faults
/// in all after it.
pub(crate) fn labelled(
    around: &Around,
    routes: &mut Routes,
    place: &dyn Fn(&Routes) -> Centres,
) -> Centres {
    let mut labels = place(routes);
    if !on("grid-label-lanes") {
        return labels;
    }
    let ids: Vec<String> = labels.keys().cloned().collect();
    for _ in 0..ROUNDS {
        let mut moved = false;
        for id in &ids {
            let before = hidden(around, routes, &labels);
            if before == 0 {
                break;
            }
            let ways = plans_for(around, routes, &labels, id);
            for moves in ways
                .into_iter()
                .chain(apart(around, routes, &labels, id))
                .take(TRIES)
            {
                let mut trial = routes.clone();
                for (line, seg, x) in &moves {
                    if let Some(r) = trial.get_mut(line) {
                        r[*seg].0 = *x;
                        r[*seg + 1].0 = *x;
                    }
                }
                let again = place(&trial);
                let after = hidden(around, &trial, &again);
                if crate::trace::wanted(id) {
                    eprintln!("trace lanes {id}: {moves:?}, defects {before} → {after}");
                }
                if after < before {
                    *routes = trial;
                    labels = again;
                    moved = true;
                    break;
                }
            }
        }
        if !moved {
            break;
        }
    }
    crate::trace::routes("grid-label-lanes", routes);
    labels
}

/// The box of the label of `id`.
fn label_box(around: &Around, labels: &Centres, id: &str) -> Option<Box2> {
    let (c, s) = (labels.get(id)?, around.sizes.get(id)?);
    Some(Box2 {
        x: c.0 - s.w / 2.0,
        y: c.1 - s.h / 2.0,
        w: s.w,
        h: s.h,
    })
}

/// Does the line `r` run through the box `b` of a label?
fn through(r: &Points, b: &Box2) -> bool {
    r.windows(2).any(|s| cuts(s[0], s[1], b, -TOUCH))
}

/// Do the boxes `a` and `b` lie over one another?
pub(super) fn meet(a: &Box2, b: &Box2) -> bool {
    a.right().min(b.right()) - a.x.max(b.x) > TOUCH
        && a.bottom().min(b.bottom()) - a.y.max(b.y) > TOUCH
}

/// Does the label `l` stand astride the frame of the zone `z`?
pub(super) fn astride(l: &Box2, z: &Box2) -> bool {
    let across_x = |x: f64| l.x + TOUCH < x && x < l.right() - TOUCH;
    let across_y = |y: f64| l.y + TOUCH < y && y < l.bottom() - TOUCH;
    (l.y < z.bottom() && z.y < l.bottom() && (across_x(z.x) || across_x(z.right())))
        || (l.x < z.right() && z.x < l.right() && (across_y(z.y) || across_y(z.bottom())))
}

/// What is wrong with the label of `id`, counted as the ledger does (`quality`): the lines of
/// the other arrows through it, the cards under it and the zone frames it stands astride; and,
/// with `after`, the labels after it that it lies over (each pair of labels counted once).
pub(super) fn faults(
    around: &Around,
    routes: &Routes,
    labels: &Centres,
    id: &str,
    after: bool,
) -> usize {
    let Some(l) = label_box(around, labels, id) else {
        return 0;
    };
    let lines = routes
        .iter()
        .filter(|(line, r)| foreign(around.groups, line, id) && through(r, &l))
        .count();
    let others = labels
        .keys()
        .filter(|o| {
            if after {
                o.as_str() > id
            } else {
                o.as_str() != id
            }
        })
        .filter_map(|o| label_box(around, labels, o))
        .filter(|m| meet(&l, m))
        .count();
    let cards = around.cards.iter().filter(|(_, c)| meet(&l, c)).count();
    let frames = around.frames.iter().filter(|z| astride(&l, z)).count();
    lines + others + cards + frames
}

/// What the rule must not trade a label for another: the faults of all the labels.
pub(crate) fn hidden(around: &Around, routes: &Routes, labels: &Centres) -> usize {
    labels
        .keys()
        .map(|id| faults(around, routes, labels, id, true))
        .sum()
}

/// The ways of clearing the label of `id`, the one that moves the lines the least first: for
/// each, `(line, segment, new x)` for the lines that move. The label stays where it stands, or
/// goes to another place of its line (`labels::slots`) once the lines there stand aside. None
/// when nothing is wrong with the label, when no place is wide enough for it beside the lines,
/// or when a line may not stand where it should (`refused`).
fn plans_for(around: &Around, routes: &Routes, labels: &Centres, id: &str) -> Vec<Vec<Move>> {
    let why = |what: &str| {
        if crate::trace::wanted(id) {
            eprintln!("trace lanes {id}: {what}");
        }
    };
    let (Some(label), Some(r)) = (label_box(around, labels, id), routes.get(id)) else {
        return Vec::new();
    };
    if faults(around, routes, labels, id, false) == 0 {
        return Vec::new();
    }
    let all = tracks(routes, around.groups);
    let others: Vec<&Track> = all
        .iter()
        .filter(|t| foreign(around.groups, t.line, id))
        .collect();
    let card_boxes: Vec<Box2> = around.cards.iter().map(|(_, b)| *b).collect();
    // where the label stands, then the other places of its line
    let here = crate::labels::slot_at(r, &label, &card_boxes, &around.frames);
    let size = (label.w, label.h);
    let elsewhere = crate::labels::slots(r, size, &card_boxes, &around.frames);
    let mut out: Vec<(f64, Vec<Move>)> = Vec::new();
    for (slot, at) in here
        .iter()
        .map(|s| (s, Some(label.x)))
        .chain(elsewhere.iter().map(|s| (s, None)))
    {
        let ok = |plan: &[Shift]| match refusal(around, routes, &others, plan) {
            Some((line, no)) => {
                why(&format!(
                    "rows {:.0?}: not {plan:.1?}: {line}, {no}",
                    slot.rows
                ));
                false
            }
            None => true,
        };
        let found = fit(slot, label.w, at, &others, &ok);
        if found.plans.is_empty() {
            why(&format!(
                "rows {:.0?}: {:.1} px short in {:.1?}, sliding in {:.1?}",
                slot.rows, found.lack, slot.gap, slot.slide
            ));
        }
        for (cost, plan) in found.plans {
            let moves: Vec<Move> = plan
                .iter()
                .map(|(t, x)| (others[*t].line.to_string(), others[*t].seg, *x))
                .collect();
            if !out.iter().any(|(_, m)| *m == moves) {
                out.push((cost, moves));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, m)| m).collect()
}

/// The ways of parting the label of `id` from a label it lies over, when both stand on upright
/// lines side by side (two arrows between the same two cards, one each way): the two lines move
/// apart, each by half, else the other alone, else its own alone, until the labels stand side
/// by side, each over its line, `LANE` between them. None when the labels are not so.
fn apart(around: &Around, routes: &Routes, labels: &Centres, id: &str) -> Vec<Vec<Move>> {
    let Some(l) = label_box(around, labels, id) else {
        return Vec::new();
    };
    let all = tracks(routes, around.groups);
    // the upright stretch of `line` that its label `b` stands over
    let under = |line: &str, b: &Box2| {
        all.iter().position(|t| {
            t.line == line
                && t.x > b.x
                && t.x < b.right()
                && t.y.0 < b.y + b.h / 2.0
                && t.y.1 > b.y + b.h / 2.0
        })
    };
    let Some(own) = under(id, &l) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for other in labels.keys().filter(|o| o.as_str() != id) {
        let Some(m) = label_box(around, labels, other).filter(|m| meet(&l, m)) else {
            continue;
        };
        let Some(its) = under(other, &m).filter(|j| all[*j].line != id) else {
            continue;
        };
        let (a, b) = (&all[own], &all[its]);
        let dx = b.x - a.x;
        let short = (l.w + m.w) / 2.0 + LANE - dx.abs();
        if dx.abs() < 1e-6 || short <= 0.0 {
            continue;
        }
        let s = dx.signum();
        let lines = [a, b];
        for (da, db) in [(short / 2.0, short / 2.0), (0.0, short), (short, 0.0)] {
            let plan: Vec<Shift> = [(0, a.x - s * da), (1, b.x + s * db)]
                .into_iter()
                .filter(|(k, x)| (lines[*k].x - x).abs() > 1e-6)
                .collect();
            if refusal(around, routes, &lines, &plan).is_none() {
                out.push(
                    plan.iter()
                        .map(|(k, x)| (lines[*k].line.to_string(), lines[*k].seg, *x))
                        .collect(),
                );
            }
        }
    }
    out
}

/// What keeps the lines of `tracks` from standing as `plan` says (`(track, its new x)`), if
/// anything: the first line that may not, and why ([`refused`]).
pub(crate) fn refusal<'a>(
    around: &Around,
    routes: &Routes,
    tracks: &[&Track<'a>],
    plan: &[Shift],
) -> Option<(&'a str, &'static str)> {
    let moving: Vec<&str> = plan.iter().map(|(t, _)| tracks[*t].line).collect();
    plan.iter().find_map(|(t, x)| {
        refused(around, routes, tracks[*t], *x, &moving).map(|no| (tracks[*t].line, no))
    })
}

/// What keeps the upright stretch `t` from standing at `x`, if anything: the three segments
/// stay clear of the cards and of the zone titles, the upright one runs along no zone border nor
/// closer than a lane to another line (but those of `moving`, which move with it), and the line
/// crosses no more lines than it did.
fn refused(
    around: &Around,
    routes: &Routes,
    t: &Track,
    x: f64,
    moving: &[&str],
) -> Option<&'static str> {
    let r = &routes[t.line];
    let i = t.seg;
    // a straight line from a card to the card under it slides along the side they share
    let straight = r.len() == 2;
    if !straight && (i == 0 || i + 2 >= r.len()) {
        return Some("an end of the line");
    }
    let (n1, n2) = ((x, r[i].1), (x, r[i + 1].1));
    // the segments before and after the upright one (a straight line has none)
    let (p0, p3) = if straight {
        (n1, n2)
    } else {
        (r[i - 1], r[i + 2])
    };
    let ends: Vec<&str> = around
        .model
        .edges
        .iter()
        .find(|e| e.id == t.line)
        .map_or(Vec::new(), |e| vec![e.from.as_str(), e.to.as_str()]);
    for (card, b) in &around.cards {
        let own = ends.contains(card);
        if straight && own {
            if x < b.x + PORT_ROOM || x > b.right() - PORT_ROOM {
                return Some("off its card");
            }
            continue;
        }
        if cuts(n1, n2, b, MARGIN - 0.5) || (!own && (cuts(p0, n1, b, 0.0) || cuts(n2, p3, b, 0.0)))
        {
            return Some("a card");
        }
    }
    let on_title = |(_, t): &(String, Box2)| {
        cuts(n1, n2, t, 0.0) || cuts(p0, n1, t, 0.0) || cuts(n2, p3, t, 0.0)
    };
    if around.titles.iter().any(on_title) {
        return Some("a zone title");
    }
    let along = |z: &Box2| {
        t.y.1.min(z.bottom()) - t.y.0.max(z.y) > 0.0
            && ((x - z.x).abs() < HUG || (x - z.right()).abs() < HUG)
    };
    if around.frames.iter().any(along) {
        return Some("along a zone border");
    }
    let near = |(line, other): (&String, &Points)| {
        line != t.line
            && !moving.contains(&line.as_str())
            && other.windows(2).any(|s| {
                let (y0, y1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
                (s[0].0 - s[1].0).abs() < 1e-9
                    && (s[0].0 - x).abs() < LANE - 0.5
                    && y1.min(t.y.1) - y0.max(t.y.0) > 0.0
            })
    };
    if routes.iter().any(near) {
        return Some("another line there");
    }
    let mut moved = r.clone();
    (moved[i].0, moved[i + 1].0) = (x, x);
    let others = || {
        routes
            .iter()
            .filter(|(line, _)| *line != t.line)
            .map(|(_, o)| o)
    };
    (crossings(&moved, others()) > crossings(r, others())).then_some("one more crossing")
}

/// How many times the line `r` crosses the lines of `others`.
pub(super) fn crossings<'a>(r: &Points, others: impl Iterator<Item = &'a Points>) -> usize {
    let cross = |a: &[(f64, f64)], b: &[(f64, f64)]| {
        // `a` upright, `b` level, each through the other's inside
        let (x, y) = (a[0].0, b[0].1);
        (a[0].0 - a[1].0).abs() < 1e-9
            && (b[0].1 - b[1].1).abs() < 1e-9
            && x > b[0].0.min(b[1].0) + 0.5
            && x < b[0].0.max(b[1].0) - 0.5
            && y > a[0].1.min(a[1].1) + 0.5
            && y < a[0].1.max(a[1].1) - 0.5
    };
    others
        .map(|o| {
            r.windows(2)
                .flat_map(|a| o.windows(2).map(move |b| (a, b)))
                .filter(|(a, b)| cross(a, b) || cross(b, a))
                .count()
        })
        .sum()
}
