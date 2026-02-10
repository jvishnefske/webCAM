/// Core geometry types for the CAM pipeline.
///
/// Swiss-cheese layer: **Geometry representation**
/// Extension point: add new geometry primitives by implementing Into<Polyline> or Into<Mesh>.
use serde::{Deserialize, Serialize};

// ── 3-D ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        Self {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            z: a.z + (b.z - a.z) * t,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    pub normal: Vec3,
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
}

impl Triangle {
    pub fn min_z(&self) -> f64 {
        self.v0.z.min(self.v1.z).min(self.v2.z)
    }
    pub fn max_z(&self) -> f64 {
        self.v0.z.max(self.v1.z).max(self.v2.z)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl BoundingBox {
    pub fn from_triangles(tris: &[Triangle]) -> Option<Self> {
        if tris.is_empty() {
            return None;
        }
        let mut min = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Vec3::new(f64::MIN, f64::MIN, f64::MIN);
        for t in tris {
            for v in [&t.v0, &t.v1, &t.v2] {
                min.x = min.x.min(v.x);
                min.y = min.y.min(v.y);
                min.z = min.z.min(v.z);
                max.x = max.x.max(v.x);
                max.y = max.y.max(v.y);
                max.z = max.z.max(v.z);
            }
        }
        Some(Self { min, max })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
    pub bounds: Option<BoundingBox>,
}

impl Mesh {
    pub fn new(triangles: Vec<Triangle>) -> Self {
        let bounds = BoundingBox::from_triangles(&triangles);
        Self { triangles, bounds }
    }
}

// ── 2-D ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn dist(a: Self, b: Self) -> f64 {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox2 {
    pub min: Vec2,
    pub max: Vec2,
}

impl BoundingBox2 {
    pub fn from_points(pts: &[Vec2]) -> Option<Self> {
        if pts.is_empty() {
            return None;
        }
        let mut min = Vec2::new(f64::MAX, f64::MAX);
        let mut max = Vec2::new(f64::MIN, f64::MIN);
        for p in pts {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Some(Self { min, max })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Polyline {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

impl Polyline {
    pub fn new(points: Vec<Vec2>, closed: bool) -> Self {
        Self { points, closed }
    }

    pub fn bounds(&self) -> Option<BoundingBox2> {
        BoundingBox2::from_points(&self.points)
    }
}

// ── Segment (used by slicer) ─────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Segment2 {
    pub a: Vec2,
    pub b: Vec2,
}

impl Segment2 {
    pub fn new(a: Vec2, b: Vec2) -> Self {
        Self { a, b }
    }
}

// ── Toolpath (intermediate representation) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolpathMove {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rapid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolpath {
    pub moves: Vec<ToolpathMove>,
}

impl Toolpath {
    pub fn new() -> Self {
        Self { moves: Vec::new() }
    }
    pub fn rapid(&mut self, x: f64, y: f64, z: f64) {
        self.moves.push(ToolpathMove {
            x,
            y,
            z,
            rapid: true,
        });
    }
    pub fn cut(&mut self, x: f64, y: f64, z: f64) {
        self.moves.push(ToolpathMove {
            x,
            y,
            z,
            rapid: false,
        });
    }
}

impl Default for Toolpath {
    fn default() -> Self {
        Self::new()
    }
}
