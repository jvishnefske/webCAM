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
}
