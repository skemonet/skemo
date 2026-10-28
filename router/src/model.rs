//! Data model of the API (SPEC §4).

use crate::geom::{Point, Rect, Side};

/// Nature d'un obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObstacleKind {
    /// Never crossed; may be a connector's end.
    Shape,
    /// Crossed only by the connectors that allow it (`pass_through`), or that have an end inside.
    Region,
}

/// Un rectangle fixe.
#[derive(Clone, Debug, PartialEq)]
pub struct Obstacle {
    /// Identifiant unique.
    pub id: String,
    /// Position et taille.
    pub rect: Rect,
    /// Shape or region.
    pub kind: ObstacleKind,
}

impl Obstacle {
    /// A shape.
    pub fn shape(id: impl Into<String>, rect: Rect) -> Self {
        Obstacle {
            id: id.into(),
            rect,
            kind: ObstacleKind::Shape,
        }
    }
    /// A region.
    pub fn region(id: impl Into<String>, rect: Rect) -> Self {
        Obstacle {
            id: id.into(),
            rect,
            kind: ObstacleKind::Region,
        }
    }
}

/// End of a connector.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// A shape; `side: None` = the best side.
    Shape {
        /// Identifier of the shape.
        id: String,
        /// Imposed side.
        side: Option<Side>,
    },
    /// A free point (editor).
    Point {
        /// Abscisse.
        x: f64,
        /// y.
        y: f64,
    },
}

impl End {
    /// A shape, any side.
    pub fn shape(id: impl Into<String>) -> End {
        End::Shape {
            id: id.into(),
            side: None,
        }
    }
    /// A shape, imposed side.
    pub fn shape_side(id: impl Into<String>, side: Side) -> End {
        End::Shape {
            id: id.into(),
            side: Some(side),
        }
    }
}

/// A link to route.
#[derive(Clone, Debug, PartialEq)]
pub struct Connector {
    /// Identifiant unique.
    pub id: String,
    /// Start.
    pub from: End,
    /// End.
    pub to: End,
    /// Waypoints, in order.
    pub checkpoints: Vec<Point>,
    /// Regions this connector may cross.
    pub pass_through: Vec<String>,
    /// Shapes this connector ignores: it may cross them as if they were absent (a shape not shown
    /// at the same time as the connector, in a diagram drawn in steps). Its own ends always count.
    pub ignore: Vec<String>,
    /// Hint route (not used before R3).
    pub hint: Option<Vec<Point>>,
    /// Hyperedge (R-18): the connectors of one group, leaving one shape, share a port and are not
    /// spread apart: their common part reads as a single trunk.
    pub group: Option<String>,
}

impl Connector {
    /// Plain connector between two ends.
    pub fn new(id: impl Into<String>, from: End, to: End) -> Self {
        Connector {
            id: id.into(),
            from,
            to,
            checkpoints: Vec::new(),
            pass_through: Vec::new(),
            ignore: Vec::new(),
            hint: None,
            group: None,
        }
    }
}

/// Parameters (defaults from the prototype, SPEC §6).
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    /// Margin around the shapes.
    pub shape_buffer: f64,
    /// Gap aimed at between parallel segments (R2).
    pub ideal_nudging: f64,
    /// Cost of a bend.
    pub segment_penalty: f64,
    /// Cost of a crossing with a connector already routed.
    pub crossing_penalty: f64,
    /// Length under which a jog is removed.
    pub min_jog: f64,
    /// Least gap between ports on one side (R2).
    pub port_spacing: f64,
    /// Extra cost, per px, of a route through a shape's margin (the route stays possible when a
    /// corridor is narrower than two margins).
    pub buffer_cost: f64,
    /// Extra cost, per px, of a route inside a crossed region (0 by default): pushes routes to the
    /// spaces between regions (a timeline's gutters).
    pub region_cost: f64,
    /// Most states explored per connector (a safeguard).
    pub max_expansions: usize,
    /// Main direction of the drawing, if any (a layered layout drawn downwards: `Down`). With
    /// `flow_penalty`, a connector leaves its source by the side facing that way and enters its
    /// target by the side facing back, unless another side is clearly cheaper.
    pub flow: Option<crate::geom::Dir>,
    /// Extra cost of a start or an end by another side than the flow's (twice for the opposite
    /// side). 0 = no preference (default).
    pub flow_penalty: f64,
    /// Largest gap between the ports of one side when the side has room (a side of length `l`
    /// with `n` ports: `l / (n + 1)`, at most this). 0 = always `ideal_nudging` apart (default).
    pub port_spread: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            shape_buffer: 14.0,
            ideal_nudging: 14.0,
            segment_penalty: 50.0,
            crossing_penalty: 100.0,
            min_jog: 6.0,
            port_spacing: 12.0,
            buffer_cost: 4.0,
            region_cost: 0.0,
            max_expansions: 4_000_000,
            flow: None,
            flow_penalty: 0.0,
            port_spread: 0.0,
        }
    }
}

/// Route of a connector: an orthogonal polyline, from the start to the end.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// Identifier of the connector.
    pub id: String,
    /// Corners, ends included.
    pub points: Vec<Point>,
}

/// Cause of a failure or of an invalid input, structured: the caller translates it into the
/// user's language; `Display` gives the English (neutral) text.
#[derive(Clone, Debug, PartialEq)]
pub enum Reason {
    /// Rectangle d'obstacle non fini.
    NonFiniteRect,
    /// Obstacle identifier already used: the duplicate is ignored.
    DuplicateId,
    /// The end names a region, not a shape.
    NotAShape(String),
    /// The end names an unknown shape.
    UnknownShape(String),
    /// Free end not finite.
    NonFiniteEnd,
    /// Free end inside an obstacle.
    EndInsideObstacle,
    /// No side of the shape is clear.
    NoClearSide(String),
    /// Aucun chemin.
    NoPath,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::NonFiniteRect => write!(f, "non-finite rectangle"),
            Reason::DuplicateId => write!(f, "duplicate identifier, ignored"),
            Reason::NotAShape(id) => write!(f, "« {id} » is a region, not a shape"),
            Reason::UnknownShape(id) => write!(f, "unknown shape « {id} »"),
            Reason::NonFiniteEnd => write!(f, "non-finite end point"),
            Reason::EndInsideObstacle => write!(f, "free end point inside an obstacle"),
            Reason::NoClearSide(id) => write!(f, "no side of « {id} » is clear"),
            Reason::NoPath => write!(f, "no path"),
        }
    }
}

/// What could not be done as asked (R-12). Never a panic.
#[derive(Clone, Debug, PartialEq)]
pub enum Diagnostic {
    /// No route possible: the connector is missing from the results.
    Unroutable {
        /// Connecteur.
        id: String,
        /// Cause.
        reason: Reason,
    },
    /// Waypoint out of reach: the connector is routed without it.
    CheckpointUnreachable {
        /// Connecteur.
        id: String,
        /// Rank of the waypoint.
        index: usize,
    },
    /// The imposed side cannot be used (blocked by another shape): another side was used.
    SideIgnored {
        /// Connecteur.
        id: String,
        /// `true` for the start, `false` for the end.
        at_start: bool,
    },
    /// Obstacle ou connecteur invalide (valeur non finie, id inconnu, doublon).
    InvalidInput {
        /// Identifier concerned.
        id: String,
        /// Cause.
        reason: Reason,
    },
}
