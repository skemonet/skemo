//! Basic geometry: points, rectangles, sides and directions.
//! Units: px (`f64`), origin at the top left, y pointing down.

/// Un point `(x, y)`.
pub type Point = (f64, f64);

/// Rectangle `x, y` (coin haut-gauche), `w × h`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// x of the top-left corner.
    pub x: f64,
    /// y of the top-left corner.
    pub y: f64,
    /// Largeur.
    pub w: f64,
    /// Hauteur.
    pub h: f64,
}

impl Rect {
    /// Creates a rectangle.
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    /// Bord gauche.
    pub fn x0(&self) -> f64 {
        self.x
    }
    /// Bord droit.
    pub fn x1(&self) -> f64 {
        self.x + self.w
    }
    /// Bord haut.
    pub fn y0(&self) -> f64 {
        self.y
    }
    /// Bord bas.
    pub fn y1(&self) -> f64 {
        self.y + self.h
    }

    /// Centre.
    pub fn center(&self) -> Point {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Every value is finite.
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.w.is_finite() && self.h.is_finite()
    }

    /// Rectangle with a positive width and height (a negative `w` or `h` is flipped).
    pub fn normalized(&self) -> Rect {
        let (x, w) = if self.w < 0.0 {
            (self.x + self.w, -self.w)
        } else {
            (self.x, self.w)
        };
        let (y, h) = if self.h < 0.0 {
            (self.y + self.h, -self.h)
        } else {
            (self.y, self.h)
        };
        Rect { x, y, w, h }
    }

    /// Rectangle grown by `d` on every side.
    pub fn inflate(&self, d: f64) -> Rect {
        Rect {
            x: self.x - d,
            y: self.y - d,
            w: self.w + 2.0 * d,
            h: self.h + 2.0 * d,
        }
    }

    /// Is `p` in the closed rectangle?
    pub fn contains(&self, p: Point) -> bool {
        self.x0() <= p.0 && p.0 <= self.x1() && self.y0() <= p.1 && p.1 <= self.y1()
    }

    /// Is `p` strictly inside?
    pub fn contains_strict(&self, p: Point) -> bool {
        self.x0() < p.0 && p.0 < self.x1() && self.y0() < p.1 && p.1 < self.y1()
    }

    /// Does the orthogonal segment `a-b` touch the open inside of the rectangle?
    /// Running along an edge does not count.
    pub fn hit_by_segment(&self, a: Point, b: Point) -> bool {
        let (sx0, sx1) = (a.0.min(b.0), a.0.max(b.0));
        let (sy0, sy1) = (a.1.min(b.1), a.1.max(b.1));
        sx1 > self.x0() && sx0 < self.x1() && sy1 > self.y0() && sy0 < self.y1()
    }
}

/// Side of a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// Haut.
    Top,
    /// Bas.
    Bottom,
    /// Gauche.
    Left,
    /// Droite.
    Right,
}

impl Side {
    /// The four sides, in a fixed order.
    pub const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

    /// Outward direction (perpendicular to the side, away from the shape).
    pub fn outward(self) -> Dir {
        match self {
            Side::Top => Dir::Up,
            Side::Bottom => Dir::Down,
            Side::Left => Dir::Left,
            Side::Right => Dir::Right,
        }
    }

    /// Point of the side at fraction `t` (0 = start, 1 = end; left to right or top to
    /// bottom).
    pub fn point(self, r: &Rect, t: f64) -> Point {
        match self {
            Side::Top => (r.x0() + r.w * t, r.y0()),
            Side::Bottom => (r.x0() + r.w * t, r.y1()),
            Side::Left => (r.x0(), r.y0() + r.h * t),
            Side::Right => (r.x1(), r.y0() + r.h * t),
        }
    }
}

/// Direction of travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    /// y decreasing.
    Up,
    /// y croissant.
    Down,
    /// x decreasing.
    Left,
    /// x croissant.
    Right,
}

impl Dir {
    /// The four directions, in a fixed order.
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

    /// Opposite direction.
    pub fn reverse(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }

    /// Horizontale ?
    pub fn horizontal(self) -> bool {
        matches!(self, Dir::Left | Dir::Right)
    }

    /// Vecteur unitaire.
    pub fn vector(self) -> Point {
        match self {
            Dir::Up => (0.0, -1.0),
            Dir::Down => (0.0, 1.0),
            Dir::Left => (-1.0, 0.0),
            Dir::Right => (1.0, 0.0),
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// Inverse de `index` ; 4 (ou plus) : aucune direction.
    pub(crate) fn from_code(c: u64) -> Option<Dir> {
        Dir::ALL.get(c as usize).copied()
    }
}

/// Length (Manhattan) of a polyline.
pub fn path_length(p: &[Point]) -> f64 {
    p.windows(2)
        .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
        .sum()
}
