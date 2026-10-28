//! Editing session (E-ROUTE-3, base of the editor): a card is dragged, the zones and arrows follow
//! live (incremental router, without running ELK again nor touching the text); at the end of the
//! drag, `commit` writes `at=` into the source (`lang`'s surgical editing).

use crate::diagram::{Diagram, Error, Options, frame, from_source};
use crate::route::{Mode, Points, finish, groups_of, params, router_scene};
use skemo_lang::Kind;
use skemo_lang::tr;
use std::collections::BTreeMap;

pub struct Session {
    pub source: String,
    opts: Options,
    pub diagram: Diagram,
    router: skemo_router::Router,
    /// The router's routes before `route::finish` (kept up to date incrementally).
    raw: BTreeMap<String, Points>,
    /// Cards moved since the last `commit`: position (file coordinates).
    moved: BTreeMap<String, (f64, f64)>,
    /// Waypoints and sides changed since the last `commit`, per arrow.
    vias: BTreeMap<String, Vec<(f64, f64)>>,
    sides: BTreeMap<String, String>,
}

impl Session {
    /// Opens a session on a source (graph mode only: in timeline and frieze, the position comes from
    /// the dates).
    pub fn new(source: &str, opts: Options) -> Result<Session, Error> {
        let (d, _) = from_source(source, &opts)?;
        if d.timeline.is_some() || d.frise.is_some() {
            return Err(Error::Internal(tr!(
                "dragging a card makes no sense in timeline or frieze mode (the position comes from the dates)",
                "glisser une carte n'a pas de sens en mode timeline ou frise (la position vient des dates)"
            )));
        }
        let p = if d.model.option("layout") == Some("mindmap") {
            params()
        } else {
            crate::route::graph_params(&d.model)
        };
        let mut router = skemo_router::Router::new(p);
        let titles = crate::labels::zone_titles(&d.model, &d.scene, &d.boxes);
        let (obstacles, conns) =
            router_scene(&d.model, &d.boxes, &d.elk, Mode::Graph, &[], &titles);
        for o in obstacles {
            router.set_obstacle(o);
        }
        for c in conns {
            router.set_connector(c);
        }
        let raw = router
            .update()
            .into_iter()
            .map(|r| (r.id, r.points))
            .collect();
        Ok(Session {
            source: source.to_string(),
            opts,
            diagram: d,
            router,
            raw,
            moved: BTreeMap::new(),
            vias: BTreeMap::new(),
            sides: BTreeMap::new(),
        })
    }

    /// Moves the card `id` to `(x, y)` (top-left corner, file coordinates) and updates zones, arrows,
    /// labels and framing. Returns the arrows routed again.
    pub fn drag(&mut self, id: &str, x: f64, y: f64) -> Result<Vec<String>, String> {
        let d = &mut self.diagram;
        if d.model.node(id).filter(|n| n.kind == Kind::Card).is_none() {
            return Err(tr!("unknown card « {id} »", "carte « {id} » inconnue"));
        }
        if !d.boxes.contains_key(id) {
            return Err(tr!(
                "card « {id} » is not placed",
                "carte « {id} » non placée"
            ));
        }
        // same chain as the full computation (D-031): integer `at=` in memory, then final boxes
        let (x, y) = (x.round(), y.round());
        if let Some(m) = d.model.nodes.iter_mut().find(|m| m.id == id) {
            m.at = Some([x, y]);
        }
        self.moved.insert(id.to_string(), (x, y));
        let (pad, head) = d.scene.zone_pad_head();
        d.boxes = crate::layout::final_boxes(&d.model, &d.sizes, &d.elk, pad, head);
        Ok(self.reroute())
    }

    /// Updates the router with the current model and boxes, then the routes, labels and framing.
    /// Returns the arrows routed again.
    fn reroute(&mut self) -> Vec<String> {
        let d = &mut self.diagram;
        let titles = crate::labels::zone_titles(&d.model, &d.scene, &d.boxes);
        let (obstacles, conns) =
            router_scene(&d.model, &d.boxes, &d.elk, Mode::Graph, &[], &titles);
        let groups = groups_of(&conns);
        for o in obstacles {
            self.router.set_obstacle(o);
        }
        for c in conns {
            self.router.set_connector(c);
        }
        for r in self.router.update() {
            self.raw.insert(r.id, r.points);
        }
        // the finishing passes look at neighbouring arrows: an arrow the router left alone may
        // still change, so the arrows re-traced are those whose drawing changed
        let routes = finish(&d.model, &d.boxes, &titles, &groups, &self.raw, Mode::Graph);
        let ids: Vec<String> = routes
            .iter()
            .filter(|(id, p)| d.routes.get(*id) != Some(*p))
            .map(|(id, _)| id.clone())
            .collect();
        d.routes = routes;
        d.labels = crate::labels::place(&d.model, &d.scene, &d.boxes, &d.routes, &d.sizes);
        d.title_box = crate::diagram::title_box(&d.model, &d.scene, &d.boxes, &d.routes);
        let mut framed = d.boxes.clone();
        if let Some(b) = d.title_box {
            framed.insert("\u{0}title".into(), b);
        }
        framed.extend(crate::diagram::label_boxes(&d.labels, &d.sizes));
        d.frame = frame(&framed, &d.routes);
        ids
    }

    /// Waypoints of the arrow `edge` (file coordinates, rounded to the integer as in the file;
    /// empty = none): only this arrow is routed again. Returns the arrows routed again.
    pub fn set_via(&mut self, edge: &str, points: &[(f64, f64)]) -> Result<Vec<String>, String> {
        let e = self.edge_mut(edge)?;
        let pts: Vec<[f64; 2]> = points.iter().map(|(x, y)| [x.round(), y.round()]).collect();
        e.via = if pts.is_empty() { None } else { Some(pts) };
        self.vias.insert(
            edge.to_string(),
            points.iter().map(|(x, y)| (x.round(), y.round())).collect(),
        );
        Ok(self.reroute())
    }

    /// Attachment sides of the arrow `edge` (`"start-end"`, one half may be empty; empty = automatic).
    pub fn set_side(&mut self, edge: &str, side: &str) -> Result<Vec<String>, String> {
        let e = self.edge_mut(edge)?;
        e.side = if side.is_empty() || side == "-" {
            None
        } else {
            Some(side.to_string())
        };
        self.sides.insert(edge.to_string(), side.to_string());
        Ok(self.reroute())
    }

    fn edge_mut(&mut self, id: &str) -> Result<&mut skemo_lang::Edge, String> {
        self.diagram
            .model
            .edges
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| tr!("unknown arrow « {id} »", "flèche « {id} » inconnue"))
    }

    /// Writes the positions of the moved cards into the source (`at="x,y"`, integers) and computes the
    /// diagram again from that source. Returns the new source.
    pub fn commit(&mut self) -> Result<String, String> {
        let mut src = self.source.clone();
        for (id, (x, y)) in &self.moved {
            src = skemo_lang::edit::move_card(&src, id, *x, *y).map_err(|e| e.to_string())?;
        }
        for (id, p) in &self.vias {
            src = skemo_lang::edit::set_via(&src, id, p).map_err(|e| e.to_string())?;
        }
        for (id, v) in &self.sides {
            src = skemo_lang::edit::set_side(&src, id, v).map_err(|e| e.to_string())?;
        }
        let fresh = Session::new(&src, self.opts.clone()).map_err(|e| e.to_string())?;
        *self = fresh;
        Ok(self.source.clone())
    }

    /// Releases a card (back to automatic placement) and computes again. Returns the source.
    pub fn release(&mut self, id: &str) -> Result<String, String> {
        let src = skemo_lang::edit::release_card(&self.source, id).map_err(|e| e.to_string())?;
        *self = Session::new(&src, self.opts.clone()).map_err(|e| e.to_string())?;
        Ok(self.source.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "diagram(dir=\"right\")\n  card#a A\n    to(b)\n    to(c)\n  card#b B\n  card#c C\n  card#d D\n    to(e)\n  card#e E\n";

    #[test]
    fn via_and_side_touch_one_arrow_and_survive_commit() {
        let mut s = Session::new(TWO, Options::default()).unwrap();
        let other = s.diagram.routes["d->e"].clone();
        s.set_via("a->b", &[(400.0, 300.0)]).unwrap();
        // arrows that share no card with the edited one do not move
        assert_eq!(s.diagram.routes["d->e"], other);
        // the waypoint lies on the route (a vertex, or a point of a segment)
        let r = &s.diagram.routes["a->b"];
        assert!(
            r.windows(2).any(|w| {
                let (lo, hi) = (
                    (w[0].0.min(w[1].0), w[0].1.min(w[1].1)),
                    (w[0].0.max(w[1].0), w[0].1.max(w[1].1)),
                );
                400.0 >= lo.0 - 1.0
                    && 400.0 <= hi.0 + 1.0
                    && 300.0 >= lo.1 - 1.0
                    && 300.0 <= hi.1 + 1.0
            }),
            "{r:?}"
        );
        s.set_side("a->c", "bottom-").unwrap();
        let live = s.diagram.routes.clone();
        let src = s.commit().unwrap();
        assert!(src.contains("via=\"400,300\""), "{src}");
        assert!(src.contains("side=\"bottom-\""), "{src}");
        assert_eq!(
            s.diagram.routes, live,
            "the preview is the reloaded drawing"
        );
        s.set_via("a->b", &[]).unwrap();
        assert!(!s.commit().unwrap().contains("via="));
    }

    #[test]
    fn drag_then_commit() {
        let src = "card#a A\n  to(b) suite\ncard#b B\n  to(c)\nzone#z Z\n  card#c C\n";
        let mut s = Session::new(src, Options::default()).unwrap();
        let before = s.diagram.routes["a->b"].clone();
        let changed = s.drag("b", 600.0, 400.0).unwrap();
        assert!(changed.contains(&"a->b".to_string()));
        assert_ne!(s.diagram.routes["a->b"], before);
        assert_eq!(
            (s.diagram.boxes["b"].x, s.diagram.boxes["b"].y),
            (600.0, 400.0)
        );
        let out = s.commit().unwrap();
        assert!(out.contains("card#b(at=\"600,400\") B"));
        assert_eq!(
            (s.diagram.boxes["b"].x, s.diagram.boxes["b"].y),
            (600.0, 400.0)
        );
        // the zone follows its card
        s.drag("c", 900.0, 100.0).unwrap();
        let z = s.diagram.boxes["z"];
        assert!(z.x < 900.0 && z.right() > 900.0);
        let back = s.release("b").unwrap();
        assert!(back.contains("card#b B"));
    }
}
