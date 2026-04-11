/// STL file parser — binary and ASCII formats.
///
/// Swiss-cheese layer: **Geometry Input**
/// Plug a different parser here (STEP, OBJ, 3MF, …) by producing a `Mesh`.
use crate::geometry::{Mesh, Triangle, Vec3};

/// Detect format and parse an STL file from raw bytes.
pub fn parse_stl(data: &[u8]) -> Result<Mesh, String> {
    if data.len() < 84 {
        // Too short for binary; try ASCII
        return parse_ascii_stl(data);
    }
    // ASCII STL starts with "solid " (but some binary files also do)
    // Heuristic: if it starts with "solid" and the declared triangle count
    // doesn't match the file size, treat as ASCII.
    if data.starts_with(b"solid") {
        let tri_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        let expected_len = 84 + tri_count * 50;
        if expected_len != data.len() {
            return parse_ascii_stl(data);
        }
    }
    parse_binary_stl(data)
}

fn read_f32_le(data: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_vec3(data: &[u8], offset: usize) -> Vec3 {
    Vec3::new(
        read_f32_le(data, offset) as f64,
        read_f32_le(data, offset + 4) as f64,
        read_f32_le(data, offset + 8) as f64,
    )
}

fn parse_binary_stl(data: &[u8]) -> Result<Mesh, String> {
    if data.len() < 84 {
        return Err("Binary STL too short".into());
    }
    let tri_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    let expected = 84 + tri_count * 50;
    if data.len() < expected {
        return Err(format!(
            "Binary STL truncated: expected {} bytes, got {}",
            expected,
            data.len()
        ));
    }
    let mut triangles = Vec::with_capacity(tri_count);
    for i in 0..tri_count {
        let base = 84 + i * 50;
        let normal = read_vec3(data, base);
        let v0 = read_vec3(data, base + 12);
        let v1 = read_vec3(data, base + 24);
        let v2 = read_vec3(data, base + 36);
        triangles.push(Triangle { normal, v0, v1, v2 });
    }
    Ok(Mesh::new(triangles))
}

fn parse_ascii_stl(data: &[u8]) -> Result<Mesh, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("Invalid UTF-8: {e}"))?;
    let mut triangles = Vec::new();
    let mut lines = text.lines().map(str::trim).peekable();

    // Skip "solid <name>"
    if let Some(first) = lines.peek() {
        if first.starts_with("solid") {
            lines.next();
        }
    }

    while let Some(line) = lines.next() {
        if line.starts_with("facet normal") {
            let normal = parse_ascii_vec3(line, "facet normal")?;
            // expect "outer loop"
            lines.next();
            let v0 = parse_vertex_line(lines.next())?;
            let v1 = parse_vertex_line(lines.next())?;
            let v2 = parse_vertex_line(lines.next())?;
            // expect "endloop" then "endfacet"
            lines.next();
            lines.next();
            triangles.push(Triangle { normal, v0, v1, v2 });
        }
    }

    if triangles.is_empty() {
        return Err("No triangles found in ASCII STL".into());
    }
    Ok(Mesh::new(triangles))
}

fn parse_ascii_vec3(line: &str, prefix: &str) -> Result<Vec3, String> {
    let rest = line
        .strip_prefix(prefix)
        .ok_or_else(|| format!("Expected '{prefix}', got '{line}'"))?
        .trim();
    let nums: Vec<f64> = rest
        .split_whitespace()
        .map(|s| s.parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Float parse error: {e}"))?;
    if nums.len() != 3 {
        return Err(format!("Expected 3 floats, got {}", nums.len()));
    }
    Ok(Vec3::new(nums[0], nums[1], nums[2]))
}

fn parse_vertex_line(line: Option<&str>) -> Result<Vec3, String> {
    let line = line.ok_or("Unexpected end of STL data")?.trim();
    parse_ascii_vec3(line, "vertex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_stl_single_triangle() {
        let mut data = vec![0u8; 84 + 50];
        // header: 80 bytes of zero
        // triangle count = 1
        data[80] = 1;
        // normal (0,0,1)
        let nz: [u8; 4] = 1.0f32.to_le_bytes();
        data[84 + 8..84 + 12].copy_from_slice(&nz);
        // v0 = (0,0,0) — already zeros
        // v1 = (1,0,0)
        let one: [u8; 4] = 1.0f32.to_le_bytes();
        data[84 + 12..84 + 16].copy_from_slice(&one);
        // v2 = (0,1,0)
        data[84 + 28..84 + 32].copy_from_slice(&one);

        let mesh = parse_stl(&data).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
    }

    #[test]
    fn test_ascii_stl() {
        let stl = b"solid test
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 0 1 0
    endloop
  endfacet
endsolid test";
        let mesh = parse_stl(stl).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.triangles[0].v1.x, 1.0);
    }

    #[test]
    fn test_parse_ascii_stl_directly() {
        let stl = b"solid cube
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 1 1 0
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 1 1 0
      vertex 0 1 0
    endloop
  endfacet
endsolid cube";
        let mesh = parse_ascii_stl(stl).unwrap();
        assert_eq!(mesh.triangles.len(), 2);
        assert_eq!(mesh.triangles[0].normal.z, -1.0);
    }

    #[test]
    fn test_parse_ascii_vec3() {
        let v = parse_ascii_vec3("vertex 1.5 2.5 3.5", "vertex").unwrap();
        assert_eq!(v.x, 1.5);
        assert_eq!(v.y, 2.5);
        assert_eq!(v.z, 3.5);
    }

    // ── Error-path coverage tests ───────────────────────────────────

    #[test]
    fn test_truncated_binary_stl() {
        // Build a binary STL header that claims 2 triangles but only has data for 1
        let mut data = vec![0u8; 84 + 50]; // enough for 1 triangle
        // triangle count = 2 (but file only has 134 bytes, needs 184)
        data[80] = 2;
        data[81] = 0;
        data[82] = 0;
        data[83] = 0;
        let result = parse_binary_stl(&data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("truncated"),
            "Error should mention truncation, got: {err}"
        );
    }

    #[test]
    fn test_invalid_utf8_ascii_stl() {
        // Invalid UTF-8 byte sequence
        let data: &[u8] = &[0x80, 0x81, 0x82, 0xFF, 0xFE];
        let result = parse_ascii_stl(data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Invalid UTF-8"),
            "Error should mention UTF-8, got: {err}"
        );
    }

    #[test]
    fn test_no_triangles_ascii_stl() {
        // Valid ASCII STL with header but no facets
        let data = b"solid empty\nendsolid empty";
        let result = parse_ascii_stl(data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("No triangles found"),
            "Error should mention no triangles, got: {err}"
        );
    }
}
