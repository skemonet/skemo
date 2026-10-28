//! Orthogonal connector router.
//!
//! Give it fixed rectangles (shapes and regions) and connectors; it computes right-angled routes
//! that go round the obstacles, with few bends. It knows neither the diagram format nor CSS.
//!
//! Method: an orthogonal visibility graph and A* with a bend penalty, after Wybrow, Marriott and
//! Stuckey, *Orthogonal Connector Routing*, GD 2009 (see `PROVENANCE.md`).
//!
//! ```
//! use skemo_router::{route, Connector, End, Obstacle, Params, Rect};
//!
//! let obstacles = [
//!     Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 50.0)),
//!     Obstacle::shape("b", Rect::new(300.0, 200.0, 100.0, 50.0)),
//!     Obstacle::shape("wall", Rect::new(150.0, -100.0, 40.0, 400.0)),
//! ];
//! let c = Connector::new("a->b", End::shape("a"), End::shape("b"));
//! let (routes, diagnostics) = route(&obstacles, &[c], &Params::default());
//! assert!(diagnostics.is_empty());
//! // the wall makes the route pass under it: it leaves "a" by the bottom
//! assert_eq!(routes[0].points, vec![(50.0, 50.0), (50.0, 314.0), (350.0, 314.0), (350.0, 250.0)]);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod geom;
mod hint;
mod incremental;
mod model;
mod nudge;
mod router;
mod scene;
mod search;
mod simplify;

pub use geom::{Dir, Point, Rect, Side, path_length};
pub use incremental::Router;
pub use model::{Connector, Diagnostic, End, Obstacle, ObstacleKind, Params, Reason, Route};
pub use router::route;

/// The README's examples are compiled and run with the tests (`cargo test --doc`).
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
