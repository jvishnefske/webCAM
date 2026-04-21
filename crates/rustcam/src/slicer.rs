/// Mesh slicer — intersects a triangle mesh with horizontal planes to produce
/// 2-D contour slices.
///
/// Swiss-cheese layer: **3-D → 2-D projection**
/// Extension point: swap in adaptive slicing, skin detection, etc.
use crate::geometry::{Mesh, Polyline, Segment2, Vec2, Vec3};

/// Slice a mesh at uniform Z intervals. Returns one `Vec<Polyline>` per layer.
pub fn slice_mesh(mesh: &Mesh, layer_height: f64) -> Vec<(f64, Vec<Polyline>)> {
    let bounds = match &mesh.bounds {
        Some(b) => b,
        None => return Vec::new(),
    };

    let z_min = bounds.min.z + layer_height * 0.5;
    let z_max = bounds.max.z;
    let mut layers = Vec::new();
    let mut z = z_min;

    while z <= z_max {
        let contours = slice_at_z(mesh, z);
        if !contours.is_empty() {
            layers.push((z, contours));
        }
        z += layer_height;
    }
    layers
}

/// Slice the mesh at a single Z height, returning closed contour(s).
pub fn slice_at_z(mesh: &Mesh, z: f64) -> Vec<Polyline> {
    let segments = collect_segments(mesh, z);
    chain_segments(segments)
}

/// For every triangle that straddles the Z plane, compute the intersection
/// line segment.
fn collect_segments(mesh: &Mesh, z: f64) -> Vec<Segment2> {
    let mut segs = Vec::new();
    for tri in &mesh.triangles {
        if tri.min_z() > z || tri.max_z() < z {
            continue;
        }
        if let Some(seg) = intersect_triangle_z(tri.v0, tri.v1, tri.v2, z) {
            segs.push(seg);
        }
    }
    segs
}

fn intersect_triangle_z(a: Vec3, b: Vec3, c: Vec3, z: f64) -> Option<Segment2> {
    let verts = [a, b, c];
    let edges = [(0, 1), (1, 2), (2, 0)];
    let mut pts: Vec<Vec2> = Vec::new();

    for &(i, j) in &edges {
        let p = verts[i];
        let q = verts[j];
        if (p.z - z) * (q.z - z) < 0.0 {
            // edge crosses the plane
            let t = (z - p.z) / (q.z - p.z);
            let ip = Vec3::lerp(p, q, t);
            pts.push(Vec2::new(ip.x, ip.y));
        } else if (p.z - z).abs() < 1e-10 {
            pts.push(Vec2::new(p.x, p.y));
        }
    }

    // Deduplicate very close points
    pts.dedup_by(|a, b| Vec2::dist(*a, *b) < 1e-10);

    if pts.len() >= 2 {
        Some(Segment2::new(pts[0], pts[1]))
    } else {
        None
    }
}

/// Query the surface normal at an XY point.
///
/// Returns the interpolated normal of the highest triangle at the XY intersection,
/// or None if the point is outside all triangles.
pub fn surface_normal_at(mesh: &Mesh, x: f64, y: f64) -> Option<Vec3> {
    let mut best: Option<(f64, Vec3)> = None;

    for tri in &mesh.triangles {
        if let Some(z) = triangle_z_at_xy(tri.v0, tri.v1, tri.v2, x, y) {
            match best {
                None => best = Some((z, tri.normal)),
                Some((max_z, _)) if z > max_z => best = Some((z, tri.normal)),
                _ => {}
            }
        }
    }

    best.map(|(_, normal)| normal)
}

/// Number of samples placed around the tool-disc perimeter for the flat
/// and ball projection routines. The full disc is sampled as: the center,
/// `DISC_SAMPLES` points at the full radius, and `DISC_SAMPLES / 2` points
/// at half the radius. Tunable in one place without touching call sites.
const DISC_SAMPLES: usize = 16;

/// Lowest legal tool-center Z for a flat cylinder end mill of radius
/// `radius` centered at `(x, y)`.
///
/// Walks a polar grid (center + `DISC_SAMPLES` at `r` + `DISC_SAMPLES/2`
/// at `r/2`) and returns the maximum `mesh_height_at` over those samples.
/// Samples that fall off the mesh are ignored; if no sample hits the mesh
/// the result is `None`.
///
/// A zero radius collapses to pure pointwise `mesh_height_at`. A negative
/// radius is rejected with `None`.
pub fn project_flat_tool(mesh: &Mesh, x: f64, y: f64, radius: f64) -> Option<f64> {
    if radius < 0.0 {
        return None;
    }
    let mut best: Option<f64> = None;
    for (sx, sy, _d) in disc_samples(x, y, radius) {
        if let Some(sz) = mesh_height_at(mesh, sx, sy) {
            best = Some(best.map_or(sz, |m| m.max(sz)));
        }
    }
    best
}

/// Lowest legal tool-center Z for a ball-end mill of radius `radius`
/// centered at `(x, y)`.
///
/// Each disc sample at planar distance `d` from the tool axis contributes
/// `sz + sqrt(radius^2 - d^2)` — the Z at which a sphere of radius
/// `radius` would sit tangent to that sample point. The maximum of these
/// contributions is the tool-center Z.
///
/// Returns `None` for a negative radius or when no sample overlaps the
/// mesh.
pub fn project_ball_tool(mesh: &Mesh, x: f64, y: f64, radius: f64) -> Option<f64> {
    if radius < 0.0 {
        return None;
    }
    let r2 = radius * radius;
    let mut best: Option<f64> = None;
    for (sx, sy, d) in disc_samples(x, y, radius) {
        if let Some(sz) = mesh_height_at(mesh, sx, sy) {
            // d <= radius is always true by construction of disc_samples.
            let lift = (r2 - d * d).max(0.0).sqrt();
            let candidate = sz + lift;
            best = Some(best.map_or(candidate, |m| m.max(candidate)));
        }
    }
    best
}

/// Enumerate the `(x, y, d)` triples for the disc sampling grid around
/// `(cx, cy)`, where `d` is the planar distance from the center. For
/// `radius == 0` only the center sample is yielded.
fn disc_samples(cx: f64, cy: f64, radius: f64) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::with_capacity(1 + DISC_SAMPLES + DISC_SAMPLES / 2);
    out.push((cx, cy, 0.0));
    if radius > 0.0 {
        let mid = radius * 0.5;
        for i in 0..DISC_SAMPLES {
            let theta = (i as f64) * core::f64::consts::TAU / (DISC_SAMPLES as f64);
            out.push((cx + radius * theta.cos(), cy + radius * theta.sin(), radius));
        }
        let inner_count = DISC_SAMPLES / 2;
        for i in 0..inner_count {
            let theta = (i as f64) * core::f64::consts::TAU / (inner_count as f64);
            out.push((cx + mid * theta.cos(), cy + mid * theta.sin(), mid));
        }
    }
    out
}

/// Query the mesh height at an XY point by casting a vertical ray downward.
///
/// Returns the highest Z coordinate where the ray intersects the mesh,
/// or None if the point is outside all triangles.
pub fn mesh_height_at(mesh: &Mesh, x: f64, y: f64) -> Option<f64> {
    let mut max_z: Option<f64> = None;

    for tri in &mesh.triangles {
        if let Some(z) = triangle_z_at_xy(tri.v0, tri.v1, tri.v2, x, y) {
            max_z = Some(max_z.map_or(z, |current| current.max(z)));
        }
    }

    max_z
}

/// Compute the Z height of a triangle at a given XY point using barycentric coordinates.
///
/// Returns None if the point lies outside the triangle's XY projection.
fn triangle_z_at_xy(v0: Vec3, v1: Vec3, v2: Vec3, x: f64, y: f64) -> Option<f64> {
    // Compute barycentric coordinates for point (x, y) in the XY projection of the triangle
    let denom = (v1.y - v2.y) * (v0.x - v2.x) + (v2.x - v1.x) * (v0.y - v2.y);
    if denom.abs() < 1e-10 {
        return None; // Degenerate triangle
    }

    let u = ((v1.y - v2.y) * (x - v2.x) + (v2.x - v1.x) * (y - v2.y)) / denom;
    let v = ((v2.y - v0.y) * (x - v2.x) + (v0.x - v2.x) * (y - v2.y)) / denom;
    let w = 1.0 - u - v;

    // Check if point is inside triangle (with small tolerance for edge cases)
    let eps = 1e-9;
    if u >= -eps && v >= -eps && w >= -eps {
        Some(u * v0.z + v * v1.z + w * v2.z)
    } else {
        None
    }
}

/// Chain loose segments into closed polylines by matching endpoints.
fn chain_segments(segments: Vec<Segment2>) -> Vec<Polyline> {
    if segments.is_empty() {
        return Vec::new();
    }

    let eps = 1e-6;
    let mut used = vec![false; segments.len()];
    let mut polylines = Vec::new();

    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }
        used[start_idx] = true;
        let mut chain = vec![segments[start_idx].a, segments[start_idx].b];

        loop {
            let tail = *chain.last().unwrap();
            let mut found = false;
            for j in 0..segments.len() {
                if used[j] {
                    continue;
                }
                if Vec2::dist(segments[j].a, tail) < eps {
                    used[j] = true;
                    chain.push(segments[j].b);
                    found = true;
                    break;
                } else if Vec2::dist(segments[j].b, tail) < eps {
                    used[j] = true;
                    chain.push(segments[j].a);
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }

        let closed = chain.len() > 2 && Vec2::dist(chain[0], *chain.last().unwrap()) < eps;
        if closed {
            chain.pop(); // remove duplicate closing point
        }
        polylines.push(Polyline::new(chain, closed));
    }
    polylines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Triangle;

    fn make_flat_quad_mesh(z: f64) -> Mesh {
        // Two triangles forming a 10x10 square at height z, extending from z-1 to z+1
        // so the plane at z intersects them.
        let t1 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, z - 1.0),
            v1: Vec3::new(10.0, 0.0, z + 1.0),
            v2: Vec3::new(10.0, 10.0, z - 1.0),
        };
        let t2 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, z - 1.0),
            v1: Vec3::new(10.0, 10.0, z - 1.0),
            v2: Vec3::new(0.0, 10.0, z + 1.0),
        };
        Mesh::new(vec![t1, t2])
    }

    #[test]
    fn test_slice_produces_segments() {
        let mesh = make_flat_quad_mesh(5.0);
        let contours = slice_at_z(&mesh, 5.0);
        assert!(!contours.is_empty());
    }

    fn make_box_mesh() -> Mesh {
        // Simple box from 0,0,0 to 10,10,5 - just the top face for height query
        let t1 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 5.0),
            v1: Vec3::new(10.0, 0.0, 5.0),
            v2: Vec3::new(10.0, 10.0, 5.0),
        };
        let t2 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 5.0),
            v1: Vec3::new(10.0, 10.0, 5.0),
            v2: Vec3::new(0.0, 10.0, 5.0),
        };
        Mesh::new(vec![t1, t2])
    }

    fn make_ramp_mesh() -> Mesh {
        // Ramp from z=0 at y=0 to z=10 at y=10
        let t1 = Triangle {
            normal: Vec3::new(0.0, -1.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 0.0),
            v1: Vec3::new(10.0, 0.0, 0.0),
            v2: Vec3::new(10.0, 10.0, 10.0),
        };
        let t2 = Triangle {
            normal: Vec3::new(0.0, -1.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 0.0),
            v1: Vec3::new(10.0, 10.0, 10.0),
            v2: Vec3::new(0.0, 10.0, 10.0),
        };
        Mesh::new(vec![t1, t2])
    }

    #[test]
    fn test_mesh_height_at_box_center() {
        let mesh = make_box_mesh();
        let z = mesh_height_at(&mesh, 5.0, 5.0);
        assert!(z.is_some());
        assert!((z.unwrap() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_mesh_height_at_box_corner() {
        let mesh = make_box_mesh();
        let z = mesh_height_at(&mesh, 0.0, 0.0);
        assert!(z.is_some());
        assert!((z.unwrap() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_mesh_height_at_outside_bounds() {
        let mesh = make_box_mesh();
        let z = mesh_height_at(&mesh, 20.0, 20.0);
        assert!(z.is_none());
    }

    #[test]
    fn test_mesh_height_at_ramp() {
        let mesh = make_ramp_mesh();
        // At y=5, z should be 5 (linear interpolation)
        let z = mesh_height_at(&mesh, 5.0, 5.0);
        assert!(z.is_some());
        assert!((z.unwrap() - 5.0).abs() < 0.001);

        // At y=0, z should be 0
        let z0 = mesh_height_at(&mesh, 5.0, 0.0);
        assert!(z0.is_some());
        assert!((z0.unwrap() - 0.0).abs() < 0.001);

        // At y=10, z should be 10
        let z10 = mesh_height_at(&mesh, 5.0, 10.0);
        assert!(z10.is_some());
        assert!((z10.unwrap() - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_req_005_surface_normal_flat() {
        // Flat horizontal surface should have normal pointing up (+Z)
        let mesh = make_box_mesh();
        let normal = surface_normal_at(&mesh, 5.0, 5.0);
        assert!(normal.is_some());
        let n = normal.unwrap();
        assert!(
            (n.z - 1.0).abs() < 0.001,
            "Z normal should be 1.0, got {}",
            n.z
        );
    }

    #[test]
    fn test_req_005_surface_normal_ramp() {
        // Ramp surface should have angled normal
        let mesh = make_ramp_mesh();
        let normal = surface_normal_at(&mesh, 5.0, 5.0);
        assert!(normal.is_some());
        let n = normal.unwrap();
        // Ramp goes up in Y direction, so normal has negative Y and positive Z
        assert!(n.y < 0.0, "Y normal should be negative for ramp");
        assert!(n.z > 0.0, "Z normal should be positive");
    }

    #[test]
    fn test_req_005_surface_normal_outside() {
        // Point outside mesh should return None
        let mesh = make_box_mesh();
        let normal = surface_normal_at(&mesh, 20.0, 20.0);
        assert!(normal.is_none());
    }

    // -------- Task-001: disc-sampled tool-shape projection -------------

    /// Flat base triangle at z=5 covering a large quad in XY.
    fn make_flat_base(z: f64) -> Vec<Triangle> {
        let t1 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(-10.0, -10.0, z),
            v1: Vec3::new(10.0, -10.0, z),
            v2: Vec3::new(10.0, 10.0, z),
        };
        let t2 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(-10.0, -10.0, z),
            v1: Vec3::new(10.0, 10.0, z),
            v2: Vec3::new(-10.0, 10.0, z),
        };
        vec![t1, t2]
    }

    /// A 2x2 horizontal bump at z=7 centered at origin, sitting on top of a
    /// flat base at z=5. Useful for verifying that the disc projection
    /// catches raised features the pointwise `mesh_height_at` would miss.
    fn make_bump_mesh() -> Mesh {
        let mut tris = make_flat_base(5.0);
        let bump1 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(-1.0, -1.0, 7.0),
            v1: Vec3::new(1.0, -1.0, 7.0),
            v2: Vec3::new(1.0, 1.0, 7.0),
        };
        let bump2 = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(-1.0, -1.0, 7.0),
            v1: Vec3::new(1.0, 1.0, 7.0),
            v2: Vec3::new(-1.0, 1.0, 7.0),
        };
        tris.push(bump1);
        tris.push(bump2);
        Mesh::new(tris)
    }

    #[test]
    fn test_req_001_flat_projection_on_horizontal_triangle() {
        // On a flat plane at z=5, project_flat_tool returns 5.0 at any point
        // whose disc overlaps the plane.
        let mesh = Mesh::new(make_flat_base(5.0));
        let z = project_flat_tool(&mesh, 0.0, 0.0, 1.0).expect("center on plane");
        assert!((z - 5.0).abs() < 1e-6, "expected 5.0, got {}", z);
    }

    #[test]
    fn test_req_001_flat_projection_outside_mesh_returns_none() {
        let mesh = Mesh::new(make_flat_base(5.0));
        // Far outside the base + tool radius.
        let z = project_flat_tool(&mesh, 100.0, 100.0, 0.5);
        assert!(z.is_none(), "expected None off-mesh, got {:?}", z);
    }

    #[test]
    fn test_req_001_flat_projection_catches_peak_in_disc() {
        // Tool center is on the base (z=5) but disc reaches into the z=7
        // bump. The disc-sampled projection must return 7, even though
        // pointwise mesh_height_at at the tool center is only 5.
        let mesh = make_bump_mesh();
        let center_z = mesh_height_at(&mesh, 1.5, 0.0).expect("center on base");
        assert!(
            (center_z - 5.0).abs() < 1e-6,
            "precondition: pointwise center should read the base, got {}",
            center_z
        );
        // Disc of radius 0.6 from (1.5, 0) reaches x=0.9 which is inside
        // the [-1, 1] bump.
        let z = project_flat_tool(&mesh, 1.5, 0.0, 0.6).expect("disc overlaps bump");
        assert!(
            (z - 7.0).abs() < 1e-6,
            "flat disc projection must pick up the z=7 bump, got {}",
            z
        );
    }

    #[test]
    fn test_req_001_ball_projection_on_horizontal_plane() {
        // Sphere of radius r sits tangent on a horizontal plane: tool
        // center is at z_plane + r.
        let mesh = Mesh::new(make_flat_base(5.0));
        let r = 0.75;
        let z = project_ball_tool(&mesh, 0.0, 0.0, r).expect("on plane");
        assert!(
            (z - (5.0 + r)).abs() < 1e-6,
            "expected {}, got {}",
            5.0 + r,
            z
        );
    }

    #[test]
    fn test_req_001_ball_projection_higher_than_flat_over_peak() {
        // With the tool center sitting directly above the z=7 bump, the
        // sphere lifts by its radius (disc samples that stay on the bump
        // plateau contribute sz + sqrt(r^2 - d^2), max at center).
        let mesh = make_bump_mesh();
        let r = 0.4;
        let flat = project_flat_tool(&mesh, 0.0, 0.0, r).expect("flat on bump");
        let ball = project_ball_tool(&mesh, 0.0, 0.0, r).expect("ball on bump");
        assert!(
            ball > flat + 1e-6,
            "expected ball ({}) strictly greater than flat ({})",
            ball,
            flat
        );
        assert!(
            (ball - (7.0 + r)).abs() < 1e-6,
            "ball over bump plateau should equal z_peak + radius, got {}",
            ball
        );
    }

    #[test]
    fn test_req_001_zero_radius_flat_collapses_to_pointwise() {
        // Radius 0 means a degenerate disc; the projection must equal
        // mesh_height_at at the query point.
        let mesh = make_bump_mesh();
        for &(x, y) in &[(0.0, 0.0), (1.5, 0.0), (2.5, 2.5)] {
            let pointwise = mesh_height_at(&mesh, x, y);
            let projected = project_flat_tool(&mesh, x, y, 0.0);
            assert_eq!(
                pointwise, projected,
                "zero-radius projection at ({}, {}) should match pointwise",
                x, y
            );
        }
    }

    #[test]
    fn test_req_001_negative_radius_returns_none() {
        // Guard: negative radii are nonsensical and return None for both.
        let mesh = Mesh::new(make_flat_base(5.0));
        assert!(project_flat_tool(&mesh, 0.0, 0.0, -1.0).is_none());
        assert!(project_ball_tool(&mesh, 0.0, 0.0, -0.1).is_none());
    }

    #[test]
    fn test_req_001_ball_projection_outside_mesh_returns_none() {
        let mesh = Mesh::new(make_flat_base(5.0));
        let z = project_ball_tool(&mesh, 100.0, 100.0, 0.5);
        assert!(z.is_none());
    }

    #[test]
    fn test_mesh_height_at_multiple_heights() {
        // Two overlapping horizontal triangles at different heights
        let t_low = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 2.0),
            v1: Vec3::new(10.0, 0.0, 2.0),
            v2: Vec3::new(5.0, 10.0, 2.0),
        };
        let t_high = Triangle {
            normal: Vec3::new(0.0, 0.0, 1.0),
            v0: Vec3::new(0.0, 0.0, 8.0),
            v1: Vec3::new(10.0, 0.0, 8.0),
            v2: Vec3::new(5.0, 10.0, 8.0),
        };
        let mesh = Mesh::new(vec![t_low, t_high]);

        // Should return highest Z (8.0)
        let z = mesh_height_at(&mesh, 5.0, 3.0);
        assert!(z.is_some());
        assert!((z.unwrap() - 8.0).abs() < 0.001);
    }
}
