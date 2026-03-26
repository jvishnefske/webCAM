use alloc::format;
use alloc::vec::Vec;

/// Build an HTTP response for a static gzipped file.
pub fn http_response_gzipped(content_type: &str, body: &[u8]) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        content_type,
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend_from_slice(body);
    response
}

/// Build an HTTP 200 response with CBOR content type (for DAG upload responses).
pub fn http_response_ok(body: &[u8]) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/cbor\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend_from_slice(body);
    response
}

/// Build an HTTP error response with the given status code and plain-text message.
pub fn http_response_error(status: u16, message: &str) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 {} Error\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        message.len()
    );
    let mut response = header.into_bytes();
    response.extend_from_slice(message.as_bytes());
    response
}

/// Parse a minimal HTTP request to extract method and path.
///
/// Returns `(method, path)` or `None` if the request line is invalid.
pub fn parse_request_line(data: &[u8]) -> Option<(&str, &str)> {
    // Find the end of the first line (look for \r\n or \n).
    // Only parse the request line as UTF-8, not the entire body which may
    // contain binary data (e.g., CBOR).
    let line_end = data.iter().position(|&b| b == b'\n').unwrap_or(data.len());
    let line_bytes = if line_end > 0 && data.get(line_end - 1) == Some(&b'\r') {
        &data[..line_end - 1]
    } else {
        &data[..line_end]
    };
    let first_line = core::str::from_utf8(line_bytes).ok()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

/// Extract the body from an HTTP request (everything after `\r\n\r\n`).
pub fn extract_body(data: &[u8]) -> Option<&[u8]> {
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(&data[i + 4..]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn test_http_response_gzipped() {
        let body = b"\x1f\x8b fake gzip data";
        let resp = http_response_gzipped("text/html", body);
        let resp_str = String::from_utf8_lossy(&resp);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp_str.contains("Content-Encoding: gzip"));
        assert!(resp_str.contains("Content-Type: text/html"));
        assert!(resp_str.contains(&format!("Content-Length: {}", body.len())));
        // Body should appear after headers
        assert!(resp.ends_with(body));
    }

    #[test]
    fn test_http_response_ok() {
        let body = b"\xa1\x63foo\x63bar";
        let resp = http_response_ok(body);
        let resp_str = String::from_utf8_lossy(&resp);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp_str.contains("Content-Type: application/cbor"));
        assert!(resp_str.contains(&format!("Content-Length: {}", body.len())));
        assert!(resp.ends_with(body));
    }

    #[test]
    fn test_http_response_error() {
        let resp = http_response_error(400, "Bad Request");
        let resp_str = String::from_utf8_lossy(&resp);
        assert!(resp_str.starts_with("HTTP/1.1 400 Error\r\n"));
        assert!(resp_str.contains("Content-Type: text/plain"));
        assert!(resp_str.contains("Content-Length: 11"));
        assert!(resp_str.ends_with("Bad Request"));
    }

    #[test]
    fn test_parse_request_line() {
        let req = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let (method, path) = parse_request_line(req).unwrap();
        assert_eq!(method, "GET");
        assert_eq!(path, "/index.html");
    }

    #[test]
    fn test_parse_request_post() {
        let req = b"POST /api/dag HTTP/1.1\r\nContent-Type: application/cbor\r\n\r\n";
        let (method, path) = parse_request_line(req).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(path, "/api/dag");
    }

    #[test]
    fn test_extract_body() {
        let req = b"POST /api HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
        let body = extract_body(req).unwrap();
        assert_eq!(body, b"hello");
    }

    #[test]
    fn test_extract_body_none() {
        let req = b"incomplete request without body separator";
        assert!(extract_body(req).is_none());
    }
}
