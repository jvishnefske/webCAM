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

    #[test]
    fn test_extract_path_d_attrs() {
        let svg = r#"<path d="M 0 0 L 1 1"/><path d="M 2 2 L 3 3"/>"#;
        let attrs = extract_path_d_attrs(svg);
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[0], "M 0 0 L 1 1");
        assert_eq!(attrs[1], "M 2 2 L 3 3");
    }

    #[test]
    fn test_extract_rects() {
        let svg = r#"<rect x="1" y="2" width="10" height="5"/>"#;
        let rects = extract_rects(svg);
        assert_eq!(rects.len(), 1);
        assert!(rects[0].closed);
        assert_eq!(rects[0].points[0], Vec2::new(1.0, 2.0));
        assert_eq!(rects[0].points[1], Vec2::new(11.0, 2.0));
    }

    #[test]
    fn test_extract_circles() {
        let svg = r#"<circle cx="5" cy="5" r="3"/>"#;
        let circles = extract_circles(svg);
        assert_eq!(circles.len(), 1);
        assert!(circles[0].closed);
        assert_eq!(circles[0].points.len(), 64);
    }

    #[test]
    fn test_extract_poly_elements_polygon() {
        let svg = r#"<polygon points="0,0 10,0 10,10"/>"#;
        let polys = extract_poly_elements(svg);
        assert_eq!(polys.len(), 1);
        assert!(polys[0].closed);
        assert_eq!(polys[0].points.len(), 3);
    }

    #[test]
    fn test_extract_poly_elements_polyline() {
        let svg = r#"<polyline points="1,1 2,2 3,3"/>"#;
        let polys = extract_poly_elements(svg);
        assert_eq!(polys.len(), 1);
        assert!(!polys[0].closed);
        assert_eq!(polys[0].points.len(), 3);
    }

    #[test]
    fn test_parse_points_attr() {
        let pts = parse_points_attr("10,20 30,40").unwrap();
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0], Vec2::new(10.0, 20.0));
        assert_eq!(pts[1], Vec2::new(30.0, 40.0));
    }

    #[test]
    fn test_read_one() {
        let tokens = vec!["42.5".to_string()];
        let mut i = 0;
        assert_eq!(read_one(&tokens, &mut i).unwrap(), 42.5);
        assert_eq!(i, 1);
    }

    #[test]
    fn test_subdivide_cubic() {
        let mut out = Vec::new();
        let p0 = Vec2::new(0.0, 0.0);
        let p1 = Vec2::new(1.0, 2.0);
        let p2 = Vec2::new(3.0, 2.0);
        let p3 = Vec2::new(4.0, 0.0);
        subdivide_cubic(&mut out, p0, p1, p2, p3, 4);
        assert_eq!(out.len(), 4);
        // Last point should be p3
        assert!((out[3].x - 4.0).abs() < 1e-10);
        assert!((out[3].y - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_subdivide_quadratic() {
        let mut out = Vec::new();
        let p0 = Vec2::new(0.0, 0.0);
        let p1 = Vec2::new(2.0, 4.0);
        let p2 = Vec2::new(4.0, 0.0);
        subdivide_quadratic(&mut out, p0, p1, p2, 4);
        assert_eq!(out.len(), 4);
        assert!((out[3].x - 4.0).abs() < 1e-10);
        assert!((out[3].y - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_svg() {
        let result = parse_svg("<svg></svg>");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No paths found"));
    }

    #[test]
    fn test_path_with_h_v_commands() {
        let svg = r#"<svg><path d="M 0 0 H 10 V 10 h -10 v -10 Z"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        // M + H + V + h + v = 5 points
        assert_eq!(paths[0].points.len(), 5);
    }

    #[test]
    fn test_path_with_cubic_bezier() {
        let svg = r#"<svg><path d="M 0 0 C 1 2 3 2 4 0"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        // 1 (M) + 16 (C subdivision) = 17
        assert_eq!(paths[0].points.len(), 17);
    }

    #[test]
    fn test_path_with_relative_cubic() {
        let svg = r#"<svg><path d="M 0 0 c 1 2 3 2 4 0"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].points.len(), 17);
    }

    #[test]
    fn test_path_with_quadratic_bezier() {
        let svg = r#"<svg><path d="M 0 0 Q 2 4 4 0"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        // 1 (M) + 16 (Q subdivision) = 17
        assert_eq!(paths[0].points.len(), 17);
    }

    #[test]
    fn test_path_with_relative_quadratic() {
        let svg = r#"<svg><path d="M 0 0 q 2 4 4 0"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].points.len(), 17);
    }

    #[test]
    fn test_extract_attr_single_quotes() {
        let tag = "<path d='M 0 0 L 1 1'";
        let result = extract_attr(tag, "d");
        assert_eq!(result, Some("M 0 0 L 1 1".to_string()));
    }

    #[test]
    fn test_extract_attr_missing() {
        let tag = "<path class=\"test\"";
        let result = extract_attr(tag, "d");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_points_attr_odd_count() {
        let result = parse_points_attr("10,20 30");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Odd number"));
    }

    #[test]
    fn test_parse_points_attr_bad_number() {
        let result = parse_points_attr("10,abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_one_end_of_data() {
        let tokens: Vec<String> = vec![];
        let mut i = 0;
        let result = read_one(&tokens, &mut i);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unexpected end"));
    }

    #[test]
    fn test_read_one_not_a_number() {
        let tokens = vec!["abc".to_string()];
        let mut i = 0;
        let result = read_one(&tokens, &mut i);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected number"));
    }

    #[test]
    fn test_tokenize_d_negative_numbers() {
        let tokens = tokenize_d("M 0 0 L -5-3");
        // Should split -5 and -3 as separate tokens
        assert!(tokens.contains(&"-5".to_string()));
        assert!(tokens.contains(&"-3".to_string()));
    }

    #[test]
    fn test_zero_radius_circle_ignored() {
        let svg = r#"<circle cx="5" cy="5" r="0"/>"#;
        let circles = extract_circles(svg);
        assert!(circles.is_empty());
    }

    #[test]
    fn test_zero_size_rect_ignored() {
        let svg = r#"<rect x="0" y="0" width="0" height="10"/>"#;
        let rects = extract_rects(svg);
        assert!(rects.is_empty());
    }

    #[test]
    fn test_path_with_implicit_lineto_after_m() {
        // After M, subsequent pairs are implicit L
        let svg = r#"<svg><path d="M 0 0 5 0 5 5"/></svg>"#;
        let paths = parse_svg(svg).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].points.len(), 3);
    }

    #[test]
    fn test_is_number() {
        assert!(is_number("3.14"));
        assert!(is_number("-1"));
        assert!(is_number("0"));
        assert!(!is_number("M"));
        assert!(!is_number("abc"));
    }
}
