/// SVG parser — extracts paths and basic shapes into polylines.
///
/// Swiss-cheese layer: **Geometry Input (2-D)**
/// Extension point: add more SVG element types or a full XML parser.
use crate::geometry::{Polyline, Vec2};

/// Parse an SVG string and return all paths as polylines.
pub fn parse_svg(svg: &str) -> Result<Vec<Polyline>, String> {
    let mut polylines = Vec::new();

    // Extract <path d="..."/> elements
    for d_attr in extract_path_d_attrs(svg) {
        let pl = parse_path_d(&d_attr)?;
        if !pl.points.is_empty() {
            polylines.push(pl);
        }
    }

    // Extract <rect> elements
    for rect in extract_rects(svg) {
        polylines.push(rect);
    }

    // Extract <circle> elements
    for circ in extract_circles(svg) {
        polylines.push(circ);
    }

    // Extract <polygon> and <polyline> elements
    for pl in extract_poly_elements(svg) {
        polylines.push(pl);
    }

    if polylines.is_empty() {
        return Err("No paths found in SVG".into());
    }
    Ok(polylines)
}

// ── Path d-attribute parsing ─────────────────────────────────────────

fn extract_path_d_attrs(svg: &str) -> Vec<String> {
    let mut results = Vec::new();
    let lower = svg.to_lowercase();
    let mut search = svg;
    while let Some(idx) = search.to_lowercase().find("<path") {
        let rest = &search[idx..];
        if let Some(end) = rest.find("/>").or_else(|| rest.find('>')) {
            let tag = &rest[..end];
            if let Some(d) = extract_attr(tag, "d") {
                results.push(d);
            }
            search = &search[idx + end + 1..];
        } else {
            break;
        }
    }
    let _ = lower; // used implicitly above
    results
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let pattern = format!("{}=\"", name);
    if let Some(start) = tag.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = tag[val_start..].find('"') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    // Also handle single-quoted attributes
    let pattern_sq = format!("{}='", name);
    if let Some(start) = tag.find(&pattern_sq) {
        let val_start = start + pattern_sq.len();
        if let Some(end) = tag[val_start..].find('\'') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    None
}

fn parse_path_d(d: &str) -> Result<Polyline, String> {
    let mut points = Vec::new();
    let mut closed = false;
    let mut cursor = Vec2::new(0.0, 0.0);
    let mut start = Vec2::new(0.0, 0.0);

    let tokens = tokenize_d(d);
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].as_str() {
            "M" => {
                i += 1;
                let (x, y) = read_pair(&tokens, &mut i)?;
                cursor = Vec2::new(x, y);
                start = cursor;
                points.push(cursor);
                // Subsequent coordinate pairs after M are implicit L
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (x, y) = read_pair(&tokens, &mut i)?;
                    cursor = Vec2::new(x, y);
                    points.push(cursor);
                }
            }
            "m" => {
                i += 1;
                let (dx, dy) = read_pair(&tokens, &mut i)?;
                cursor = Vec2::new(cursor.x + dx, cursor.y + dy);
                start = cursor;
                points.push(cursor);
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (dx, dy) = read_pair(&tokens, &mut i)?;
                    cursor = Vec2::new(cursor.x + dx, cursor.y + dy);
                    points.push(cursor);
                }
            }
            "L" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (x, y) = read_pair(&tokens, &mut i)?;
                    cursor = Vec2::new(x, y);
                    points.push(cursor);
                }
            }
            "l" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (dx, dy) = read_pair(&tokens, &mut i)?;
                    cursor = Vec2::new(cursor.x + dx, cursor.y + dy);
                    points.push(cursor);
                }
            }
            "H" => {
                i += 1;
                let x = read_one(&tokens, &mut i)?;
                cursor = Vec2::new(x, cursor.y);
                points.push(cursor);
            }
            "h" => {
                i += 1;
                let dx = read_one(&tokens, &mut i)?;
                cursor = Vec2::new(cursor.x + dx, cursor.y);
                points.push(cursor);
            }
            "V" => {
                i += 1;
                let y = read_one(&tokens, &mut i)?;
                cursor = Vec2::new(cursor.x, y);
                points.push(cursor);
            }
            "v" => {
                i += 1;
                let dy = read_one(&tokens, &mut i)?;
                cursor = Vec2::new(cursor.x, cursor.y + dy);
                points.push(cursor);
            }
            "C" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (x1, y1) = read_pair(&tokens, &mut i)?;
                    let (x2, y2) = read_pair(&tokens, &mut i)?;
                    let (x, y) = read_pair(&tokens, &mut i)?;
                    let p0 = cursor;
                    let p1 = Vec2::new(x1, y1);
                    let p2 = Vec2::new(x2, y2);
                    let p3 = Vec2::new(x, y);
                    subdivide_cubic(&mut points, p0, p1, p2, p3, 16);
                    cursor = p3;
                }
            }
            "c" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (dx1, dy1) = read_pair(&tokens, &mut i)?;
                    let (dx2, dy2) = read_pair(&tokens, &mut i)?;
                    let (dx, dy) = read_pair(&tokens, &mut i)?;
                    let p0 = cursor;
                    let p1 = Vec2::new(cursor.x + dx1, cursor.y + dy1);
                    let p2 = Vec2::new(cursor.x + dx2, cursor.y + dy2);
                    let p3 = Vec2::new(cursor.x + dx, cursor.y + dy);
                    subdivide_cubic(&mut points, p0, p1, p2, p3, 16);
                    cursor = p3;
                }
            }
            "Q" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (x1, y1) = read_pair(&tokens, &mut i)?;
                    let (x, y) = read_pair(&tokens, &mut i)?;
                    let p0 = cursor;
                    let p1 = Vec2::new(x1, y1);
                    let p2 = Vec2::new(x, y);
                    subdivide_quadratic(&mut points, p0, p1, p2, 16);
                    cursor = p2;
                }
            }
            "q" => {
                i += 1;
                while i < tokens.len() && is_number(&tokens[i]) {
                    let (dx1, dy1) = read_pair(&tokens, &mut i)?;
                    let (dx, dy) = read_pair(&tokens, &mut i)?;
                    let p0 = cursor;
                    let p1 = Vec2::new(cursor.x + dx1, cursor.y + dy1);
                    let p2 = Vec2::new(cursor.x + dx, cursor.y + dy);
                    subdivide_quadratic(&mut points, p0, p1, p2, 16);
                    cursor = p2;
                }
            }
            "Z" | "z" => {
                closed = true;
                cursor = start;
                i += 1;
            }
            _ => {
                // Skip unknown commands
                i += 1;
            }
        }
    }

    Ok(Polyline::new(points, closed))
}

fn tokenize_d(d: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut buf = String::new();

    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !buf.is_empty() {
                tokens.push(buf.clone());
                buf.clear();
            }
            tokens.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !buf.is_empty() {
                tokens.push(buf.clone());
                buf.clear();
            }
        } else if ch == '-' && !buf.is_empty() && !buf.ends_with('e') && !buf.ends_with('E') {
            tokens.push(buf.clone());
            buf.clear();
            buf.push(ch);
        } else {
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        tokens.push(buf);
    }
    tokens
}

fn is_number(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

fn read_one(tokens: &[String], i: &mut usize) -> Result<f64, String> {
    if *i >= tokens.len() {
        return Err("Unexpected end of path data".into());
    }
    let val = tokens[*i]
        .parse::<f64>()
        .map_err(|_| format!("Expected number, got '{}'", tokens[*i]))?;
    *i += 1;
    Ok(val)
}

fn read_pair(tokens: &[String], i: &mut usize) -> Result<(f64, f64), String> {
    let a = read_one(tokens, i)?;
    let b = read_one(tokens, i)?;
    Ok((a, b))
}

fn subdivide_cubic(out: &mut Vec<Vec2>, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, steps: usize) {
    for s in 1..=steps {
        let t = s as f64 / steps as f64;
        let u = 1.0 - t;
        let x =
            u * u * u * p0.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * p3.x;
        let y =
            u * u * u * p0.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * p3.y;
        out.push(Vec2::new(x, y));
    }
}

fn subdivide_quadratic(out: &mut Vec<Vec2>, p0: Vec2, p1: Vec2, p2: Vec2, steps: usize) {
    for s in 1..=steps {
        let t = s as f64 / steps as f64;
        let u = 1.0 - t;
        let x = u * u * p0.x + 2.0 * u * t * p1.x + t * t * p2.x;
        let y = u * u * p0.y + 2.0 * u * t * p1.y + t * t * p2.y;
        out.push(Vec2::new(x, y));
    }
}

// ── <rect> extraction ────────────────────────────────────────────────

fn extract_rects(svg: &str) -> Vec<Polyline> {
    let mut results = Vec::new();
    let mut search = svg;
    while let Some(idx) = search.to_lowercase().find("<rect") {
        let rest = &search[idx..];
        if let Some(end) = rest.find("/>").or_else(|| rest.find('>')) {
            let tag = &rest[..end];
            let x = extract_attr(tag, "x")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let y = extract_attr(tag, "y")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let w: f64 = extract_attr(tag, "width")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let h: f64 = extract_attr(tag, "height")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            if w > 0.0 && h > 0.0 {
                results.push(Polyline::new(
                    vec![
                        Vec2::new(x, y),
                        Vec2::new(x + w, y),
                        Vec2::new(x + w, y + h),
                        Vec2::new(x, y + h),
                    ],
                    true,
                ));
            }
            search = &search[idx + end + 1..];
        } else {
            break;
        }
    }
    results
}

// ── <circle> extraction ──────────────────────────────────────────────

fn extract_circles(svg: &str) -> Vec<Polyline> {
    let mut results = Vec::new();
    let mut search = svg;
    while let Some(idx) = search.to_lowercase().find("<circle") {
        let rest = &search[idx..];
        if let Some(end) = rest.find("/>").or_else(|| rest.find('>')) {
            let tag = &rest[..end];
            let cx: f64 = extract_attr(tag, "cx")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let cy: f64 = extract_attr(tag, "cy")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let r: f64 = extract_attr(tag, "r")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            if r > 0.0 {
                let segments = 64;
                let points: Vec<Vec2> = (0..segments)
                    .map(|i| {
                        let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
                        Vec2::new(cx + r * angle.cos(), cy + r * angle.sin())
                    })
                    .collect();
                results.push(Polyline::new(points, true));
            }
            search = &search[idx + end + 1..];
        } else {
            break;
        }
    }
    results
}

// ── <polygon> / <polyline> extraction ────────────────────────────────

fn extract_poly_elements(svg: &str) -> Vec<Polyline> {
    let mut results = Vec::new();
    for (tag_name, is_closed) in &[("polygon", true), ("polyline", false)] {
        let needle = format!("<{}", tag_name);
        let mut search = svg;
        while let Some(idx) = search.to_lowercase().find(&needle) {
            let rest = &search[idx..];
            if let Some(end) = rest.find("/>").or_else(|| rest.find('>')) {
                let tag = &rest[..end];
                if let Some(pts_str) = extract_attr(tag, "points") {
                    if let Ok(pts) = parse_points_attr(&pts_str) {
                        if !pts.is_empty() {
                            results.push(Polyline::new(pts, *is_closed));
                        }
                    }
                }
                search = &search[idx + end + 1..];
            } else {
                break;
            }
        }
    }
    results
}

fn parse_points_attr(s: &str) -> Result<Vec<Vec2>, String> {
    let mut points = Vec::new();
    let nums: Vec<f64> = s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("points parse error: {e}"))?;
    if !nums.len().is_multiple_of(2) {
        return Err("Odd number of coordinates in points attribute".into());
    }
    for pair in nums.chunks(2) {
        points.push(Vec2::new(pair[0], pair[1]));
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_path() {
        let svg = r#"<svg><path d="M 0 0 L 10 0 L 10 10 L 0 10 Z"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert_eq!(paths[0].points.len(), 4);
    }

    #[test]
    fn test_rect() {
        let svg = r#"<svg><rect x="5" y="5" width="20" height="10"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert_eq!(paths[0].points.len(), 4);
    }

    #[test]
    fn test_circle() {
        let svg = r#"<svg><circle cx="50" cy="50" r="25"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert_eq!(paths[0].points.len(), 64);
    }

    #[test]
    fn test_relative_path() {
        let svg = r#"<svg><path d="m 10 10 l 5 0 l 0 5 z"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].points[0], Vec2::new(10.0, 10.0));
        assert_eq!(paths[0].points[1], Vec2::new(15.0, 10.0));
    }
}
