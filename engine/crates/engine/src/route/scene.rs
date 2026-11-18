//! Translation of the scene for the router: cards, zones and zone titles as obstacles, arrows as
//! connectors, with what a diagram drawn in steps lets each arrow ignore.

use super::Mode;
use super::groups::{auto_buses, auto_forks, auto_joins, keep_crowds};
use super::sides::{back_channel, corner_back, crossable, edge_sides, fan_in, straight_back};
use crate::layout::{Box2, Placement};
use skemo_lang::{Kind, Model};
use skemo_router::{Connector, End, Obstacle, Rect};
use std::collections::BTreeMap;

/// Translation of the scene for the router (E-ROUTE-2): obstacles and connectors. In graph mode
/// the zone titles (`titles`, from `labels::zone_titles`) are shapes: an arrow entering a zone
/// goes beside its title, not through it. A title that overlaps a card (theme moving it) is left
/// out, so that no card end ends up inside an obstacle.
pub fn router_scene(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    elk: &Placement,
    mode: Mode,
    soft: &[(String, Box2)],
    titles: &[(String, Box2)],
) -> (Vec<Obstacle>, Vec<Connector>) {
    let mut all_zones: Vec<String> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone)
        .map(|n| n.id.clone())
        .collect();
    let mut obstacles = Vec::new();
    for n in &model.nodes {
        let Some(b) = boxes.get(&n.id) else { continue };
        let r = Rect::new(b.x, b.y, b.w, b.h);
        obstacles.push(match n.kind {
            Kind::Card => Obstacle::shape(n.id.clone(), r),
            Kind::Zone => Obstacle::region(n.id.clone(), r),
        });
    }
    // a title over a card is ignored by that card's own arrows only (`title_obstacles`)
    let mut over_cards: Vec<(String, Vec<String>)> = Vec::new();
    if mode.automatic() {
        let (t, over) = title_obstacles(model, boxes, titles);
        obstacles.extend(t);
        over_cards = over;
    }
    // extra-cost bands (crossable by all, costly to run along: `region_cost`)
    for (id, b) in soft {
        obstacles.push(Obstacle::region(id.clone(), Rect::new(b.x, b.y, b.w, b.h)));
        all_zones.push(id.clone());
    }
    let (all_forks, forks, joins) = auto_fans(model, boxes, mode);
    // buses read every fork, merged or not: a bus is no crowd rule's business
    let buses = auto_buses(model, boxes, &all_forks);
    let mut conns = Vec::new();
    for e in &model.edges {
        if !boxes.contains_key(&e.from) || !boxes.contains_key(&e.to) {
            continue;
        }
        let (s1, s2, back, mindmap_group) = edge_sides(model, boxes, e, mode, &buses);
        // a join (named, D-050, or automatic, D-091) is routed from its target, so that the
        // arrows into one card share a trunk like a fork out of it; `finish` turns the route round
        let turned = fan_in(e) || joins.contains_key(&e.id);
        let (a, b) = if turned {
            ((&e.to, s2), (&e.from, s1))
        } else {
            ((&e.from, s1), (&e.to, s2))
        };
        let mut c = Connector::new(
            e.id.clone(),
            End::Shape {
                id: a.0.clone(),
                side: a.1,
            },
            End::Shape {
                id: b.0.clone(),
                side: b.1,
            },
        );
        c.checkpoints = e
            .via
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|[x, y]| (x, y))
            .collect();
        c.pass_through = if mode.automatic() {
            crossable(model, e)
        } else {
            all_zones.clone()
        };
        c.hint = elk
            .routes
            .get(&e.id)
            .filter(|_| !back && !turned)
            .map(|(p, _)| p.clone());
        // hyperedge (`join="trunk"`): the arrows of one `to(a b c)` share a trunk; a named join, the
        // arrows into one card that carry it (D-050)
        c.group = e
            .join
            .as_ref()
            .map(|j| {
                if fan_in(e) {
                    format!("{}|join|{j}", e.to)
                } else {
                    format!("{}|{j}", e.from)
                }
            })
            .or(mindmap_group)
            .or_else(|| joins.get(&e.id).cloned())
            .or_else(|| buses.get(&e.id).map(|b| b.group.clone()))
            .or_else(|| forks.get(&e.id).cloned());
        if let Some(b) = buses.get(&e.id) {
            c.checkpoints = b.via.clone();
        }
        // a back edge face to face goes straight (`straight_back`), one to a side turns a corner
        // (`corner_back`): no loop round the row
        if back
            && !straight_back(model, boxes, e)
            && corner_back(model, boxes, e).is_none()
            && let Some(p) = back_channel(model, boxes, e)
        {
            c.checkpoints = vec![p];
        }
        conns.push(c);
    }
    ignore_titles_over_ends(&over_cards, &mut conns);
    step_ignores(model, &obstacles, &mut conns);
    (obstacles, conns)
}

/// Edge id → group of an automatic fork or join.
type Fans = BTreeMap<String, String>;

/// Automatic forks and joins (D-047, D-091), in graph mode only: every fork, then those drawn
/// merged and the joins (D-093).
fn auto_fans(model: &Model, boxes: &BTreeMap<String, Box2>, mode: Mode) -> (Fans, Fans, Fans) {
    if !mode.automatic() {
        return (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    }
    let forks = auto_forks(model, boxes);
    let joins = auto_joins(model, boxes, &forks);
    let (kept_forks, kept_joins) = keep_crowds(model, boxes, &forks, &joins);
    (forks, kept_forks, kept_joins)
}

/// In a diagram drawn in steps, an arrow is routed among what is shown at the same time as it
/// (D-060): the cards, zones and zone titles whose steps never meet its own are no obstacle to
/// it. The layout does not change, so nothing moves from one step to the next; only the lines of
/// arrows that never live with those cards may pass where they stand.
fn step_ignores(model: &Model, obstacles: &[Obstacle], conns: &mut [Connector]) {
    let anim = crate::anim::resolve(model);
    let own = |id: &str| {
        anim.elements.get(id).map(|r| {
            (
                r.appear.value,
                r.until.as_ref().map_or(f64::INFINITY, |u| u.value),
            )
        })
    };
    // a group that draws nothing (a bare zone without title, a stack) is on screen only while
    // something inside it is: the span of its contents
    let life = |id: &str| match model.node(id) {
        Some(n) if n.kind == Kind::Zone && crate::layout::invisible(n) => model
            .nodes
            .iter()
            .filter(|d| d.kind == Kind::Card && model.ancestors(&d.id).iter().any(|a| a.id == id))
            .filter_map(|d| own(&d.id))
            .reduce(|(b0, y0), (b1, y1)| (b0.min(b1), y0.max(y1))),
        _ => own(id),
    };
    for c in conns.iter_mut() {
        let Some((a, z)) = own(&c.id) else {
            continue;
        };
        c.life = Some((a, z));
        let apart = |id: &str| life(id).is_some_and(|(b, y)| y < a || z < b);
        for n in model.nodes.iter().filter(|n| apart(&n.id)) {
            if n.kind == Kind::Card {
                c.ignore.push(n.id.clone());
            } else {
                c.pass_through.push(n.id.clone());
                let title = format!("\u{0}title|{}", n.id);
                c.ignore.extend(
                    obstacles
                        .iter()
                        .filter(|o| o.id == title || o.id.starts_with(&format!("{title}|")))
                        .map(|o| o.id.clone()),
                );
            }
        }
    }
}
/// The arrows of a card a zone title overlaps ignore that title (`title_obstacles`).
fn ignore_titles_over_ends(over_cards: &[(String, Vec<String>)], conns: &mut [Connector]) {
    for c in conns {
        let ends = [&c.from, &c.to].map(|e| match e {
            End::Shape { id, .. } => Some(id.clone()),
            End::Point { .. } => None,
        });
        for (title, cards) in over_cards {
            if ends.iter().flatten().any(|id| cards.contains(id)) {
                c.ignore.push(title.clone());
            }
        }
    }
}

/// Zone titles as shapes, with the cards each one overlaps (a theme moving a title, zones over
/// one another): those cards' own arrows ignore it, so that no end of theirs lies inside an
/// obstacle, while every other arrow still goes round it. Left out altogether, such a title let
/// any line run across it.
fn title_obstacles(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
) -> (Vec<Obstacle>, Vec<(String, Vec<String>)>) {
    let cards: Vec<(&str, &Box2)> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), b)))
        .collect();
    let hits = |a: &Box2, b: &Box2| {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    };
    let mut over = Vec::new();
    // a zone may have several boxes (its title, its description lines, its tag pill last, as
    // `labels::zone_titles` lists them): one obstacle each; only the pill is a tag
    let obstacles = titles
        .iter()
        .enumerate()
        .map(|(k, (id, t))| {
            let more = k > 0 && titles[k - 1].0 == *id;
            let last = titles.get(k + 1).is_none_or(|(next, _)| next != id);
            let tag = more && last && model.node(id).is_some_and(|n| n.tag.is_some());
            // every box its own id: the router keeps one shape per id
            let oid = if more {
                format!("\u{0}title|{id}|{k}")
            } else {
                format!("\u{0}title|{id}")
            };
            let under: Vec<String> = cards
                .iter()
                .filter(|(_, c)| hits(c, t))
                .map(|(c, _)| (*c).to_string())
                .collect();
            if !under.is_empty() {
                over.push((oid.clone(), under));
            }
            // a line may pass `TAG_CLEAR` from a zone's tag pill, closer than from a card (D-057):
            // the router keeps its margin round every shape, so the pill's box is shrunk by the
            // difference
            let d = if tag {
                (skemo_router::Params::default().shape_buffer - TAG_CLEAR).max(0.0)
            } else {
                0.0
            };
            let (w, h) = ((t.w - 2.0 * d).max(1.0), (t.h - 2.0 * d).max(1.0));
            let (x, y) = (t.x + (t.w - w) / 2.0, t.y + (t.h - h) / 2.0);
            Obstacle::shape(oid, Rect::new(x, y, w, h))
        })
        .collect();
    (obstacles, over)
}

/// Room kept between a line and a zone's tag pill (px), less than round a card: the pill is a small
/// framed chip on the title's row, and the axis of the zone's first cards often runs just past it
/// (D-057). Titles keep the router's margin.
const TAG_CLEAR: f64 = 6.0;
