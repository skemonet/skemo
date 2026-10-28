//! Incremental API for the editor (R-14).
//!
//! `update` returns only the routes that changed, but the result is always that of a call to
//! [`route`] on the whole scene (D-031: the drawing must not depend on the editing history; R2's
//! spacing is global). The scene is therefore computed again in full as soon as a change touches
//! at least one connector (an end, or a route near a changed obstacle); otherwise nothing is
//! computed. Routing only the connectors touched would be an optimisation only, allowed if the
//! result stays identical to the full computation.

use std::collections::BTreeMap;

use crate::geom::Rect;
use crate::model::{Connector, Diagnostic, End, Obstacle, Params, Route};
use crate::router::route;

/// Router with a state, for interactive editing.
#[derive(Debug, Clone)]
pub struct Router {
    params: Params,
    obstacles: BTreeMap<String, Obstacle>,
    /// Order in which the obstacles were added (determinism).
    order: Vec<String>,
    connectors: BTreeMap<String, Connector>,
    corder: Vec<String>,
    routes: BTreeMap<String, Route>,
    diagnostics: Vec<Diagnostic>,
    /// Areas changed since the last `update` (before and after), and connectors changed.
    dirty_rects: Vec<Rect>,
    dirty_ids: Vec<String>,
    dirty_conns: Vec<String>,
}

impl Router {
    /// Routeur vide.
    pub fn new(params: Params) -> Self {
        Router {
            params,
            obstacles: BTreeMap::new(),
            order: Vec::new(),
            connectors: BTreeMap::new(),
            corder: Vec::new(),
            routes: BTreeMap::new(),
            diagnostics: Vec::new(),
            dirty_rects: Vec::new(),
            dirty_ids: Vec::new(),
            dirty_conns: Vec::new(),
        }
    }

    /// Adds or moves an obstacle.
    pub fn set_obstacle(&mut self, o: Obstacle) {
        if let Some(old) = self.obstacles.get(&o.id) {
            if old == &o {
                return;
            }
            self.dirty_rects.push(old.rect);
        } else {
            self.order.push(o.id.clone());
        }
        self.dirty_rects.push(o.rect);
        self.dirty_ids.push(o.id.clone());
        self.obstacles.insert(o.id.clone(), o);
    }

    /// Retire un obstacle.
    pub fn remove_obstacle(&mut self, id: &str) {
        if let Some(old) = self.obstacles.remove(id) {
            self.dirty_rects.push(old.rect);
            self.dirty_ids.push(id.to_string());
            self.order.retain(|k| k != id);
        }
    }

    /// Ajoute ou modifie un connecteur.
    pub fn set_connector(&mut self, c: Connector) {
        if self.connectors.get(&c.id) == Some(&c) {
            return;
        }
        if !self.connectors.contains_key(&c.id) {
            self.corder.push(c.id.clone());
        }
        self.dirty_conns.push(c.id.clone());
        self.connectors.insert(c.id.clone(), c);
    }

    /// Retire un connecteur.
    pub fn remove_connector(&mut self, id: &str) {
        if self.connectors.remove(id).is_some() {
            self.corder.retain(|k| k != id);
            self.routes.remove(id);
            self.dirty_conns.push(id.to_string());
        }
    }

    fn touched(&self, c: &Connector) -> bool {
        if self.dirty_conns.contains(&c.id) {
            return true;
        }
        let end_hit = |e: &End| matches!(e, End::Shape { id, .. } if self.dirty_ids.contains(id));
        if end_hit(&c.from) || end_hit(&c.to) {
            return true;
        }
        let buffer = self.params.shape_buffer.max(0.0) + self.params.ideal_nudging.max(0.0);
        match self.routes.get(&c.id) {
            None => true,
            Some(r) => r.points.windows(2).any(|w| {
                self.dirty_rects
                    .iter()
                    .any(|d| d.inflate(buffer).hit_by_segment(w[0], w[1]))
            }),
        }
    }

    /// Computes again what needs it; returns the routes that changed.
    pub fn update(&mut self) -> Vec<Route> {
        let any = self.connectors.values().any(|c| self.touched(c));
        self.dirty_rects.clear();
        self.dirty_ids.clear();
        self.dirty_conns.clear();
        if !any {
            return Vec::new();
        }
        let obstacles: Vec<Obstacle> = self
            .order
            .iter()
            .filter_map(|k| self.obstacles.get(k).cloned())
            .collect();
        let connectors: Vec<Connector> = self
            .corder
            .iter()
            .filter_map(|k| self.connectors.get(k).cloned())
            .collect();
        let (routes, diags) = route(&obstacles, &connectors, &self.params);
        self.diagnostics = diags;
        let mut changed = Vec::new();
        let mut next = BTreeMap::new();
        for r in routes {
            if self.routes.get(&r.id) != Some(&r) {
                changed.push(r.clone());
            }
            next.insert(r.id.clone(), r);
        }
        self.routes = next;
        changed
    }

    /// Every current route.
    pub fn routes(&self) -> impl Iterator<Item = &Route> {
        self.corder.iter().filter_map(|k| self.routes.get(k))
    }

    /// Diagnostics of the last computation.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;

    #[test]
    fn incremental_matches_full_route() {
        let mut r = Router::new(Params::default());
        r.set_obstacle(Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 50.0)));
        r.set_obstacle(Obstacle::shape("b", Rect::new(300.0, 200.0, 100.0, 50.0)));
        r.set_obstacle(Obstacle::shape(
            "loin",
            Rect::new(2000.0, 2000.0, 50.0, 50.0),
        ));
        r.set_connector(Connector::new("a->b", End::shape("a"), End::shape("b")));
        assert_eq!(r.update().len(), 1);
        // nothing changed: nothing to do
        assert!(r.update().is_empty());
        // a far obstacle moves: no route touched
        r.set_obstacle(Obstacle::shape(
            "loin",
            Rect::new(2100.0, 2000.0, 50.0, 50.0),
        ));
        assert!(r.update().is_empty());
        // a wall in the way: the route changes, and it is identical to the full computation
        let mur = Obstacle::shape("mur", Rect::new(150.0, -100.0, 40.0, 400.0));
        r.set_obstacle(mur.clone());
        let changed = r.update();
        assert_eq!(changed.len(), 1);
        let obstacles = [
            Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 50.0)),
            Obstacle::shape("b", Rect::new(300.0, 200.0, 100.0, 50.0)),
            Obstacle::shape("loin", Rect::new(2100.0, 2000.0, 50.0, 50.0)),
            mur,
        ];
        let (full, _) = route(
            &obstacles,
            &[Connector::new("a->b", End::shape("a"), End::shape("b"))],
            &Params::default(),
        );
        assert_eq!(changed[0], full[0]);
    }
}

#[cfg(test)]
mod free_ends {
    use crate::geom::Rect;
    use crate::model::{Connector, End, Obstacle, Params};
    use crate::router::route;

    /// R-16: a free end (an arrow dragged into the void by the editor).
    #[test]
    fn route_to_free_point() {
        let obstacles = [
            Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 50.0)),
            Obstacle::shape("mur", Rect::new(150.0, -100.0, 40.0, 400.0)),
        ];
        let c = Connector::new("a->p", End::shape("a"), End::Point { x: 400.0, y: 20.0 });
        let (routes, diags) = route(&obstacles, &[c], &Params::default());
        assert!(diags.is_empty(), "{diags:?}");
        let p = &routes[0].points;
        assert_eq!(*p.last().unwrap(), (400.0, 20.0));
        assert!(p.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1));
    }
}
