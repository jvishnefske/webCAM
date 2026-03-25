//! Gzip-compressed static file server for embassy-net TCP sockets.
//!
//! Serves pre-compressed assets with `Content-Encoding: gzip`. The board
//! binary provides its `include_bytes!` data via [`StaticAssets`], keeping
//! the build-system coupling in the binary crate.

use embassy_net::tcp::TcpSocket;

use crate::ws_framing;

/// A set of gzip-compressed static assets to serve.
///
/// Each field holds the raw gzip bytes for one frontend file. Board
/// binaries populate this with `include_bytes!` data or empty slices
/// when the frontend is not built.
pub struct StaticAssets {
    /// Gzip-compressed index.html content.
    pub index_html: &'static [u8],
    /// Gzip-compressed JavaScript loader content.
    pub app_js: &'static [u8],
    /// Gzip-compressed WebAssembly binary content.
    pub app_wasm: &'static [u8],
    /// Gzip-compressed CSS stylesheet content.
    pub style_css: &'static [u8],
}

/// A static asset with its gzip-compressed content and MIME type.
struct Asset<'a> {
    /// Gzip-compressed file content.
    content: &'a [u8],
    /// MIME content-type header value.
    content_type: &'static str,
}

/// Matches a URL path to a static asset by file extension.
///
/// Routes by extension rather than exact filename because Trunk
/// generates content-hashed filenames that change on each build.
/// Since there is exactly one file per type, extension matching
/// is unambiguous.
fn match_asset<'a>(assets: &'a StaticAssets, path: &str) -> Option<Asset<'a>> {
    if path == "/" || path.ends_with(".html") {
        Some(Asset {
            content: assets.index_html,
            content_type: "text/html; charset=utf-8",
        })
    } else if path.ends_with(".js") {
        Some(Asset {
            content: assets.app_js,
            content_type: "application/javascript",
        })
    } else if path.ends_with(".wasm") {
        Some(Asset {
            content: assets.app_wasm,
            content_type: "application/wasm",
        })
    } else if path.ends_with(".css") {
        Some(Asset {
            content: assets.style_css,
            content_type: "text/css",
        })
    } else {
        None
    }
}

/// Writes a `usize` as decimal ASCII digits into `buf`.
///
/// Returns the number of bytes written. The caller must ensure
/// `buf` has at least 20 bytes available.
fn write_usize(mut n: usize, buf: &mut [u8]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut digits = [0u8; 20];
    let mut len = 0;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut i = 0;
    while i < len {
        buf[i] = digits[len - 1 - i];
        i += 1;
    }
    len
}

/// Builds an HTTP 200 response header with `Content-Encoding: gzip`.
///
/// Writes the complete header (status line, content-type, encoding,
/// length, and connection-close) into `buf`. Returns the number of
/// bytes written.
fn build_http_200(buf: &mut [u8], content_type: &str, content_length: usize) -> usize {
    let mut pos = 0;

    let status = b"HTTP/1.1 200 OK\r\nContent-Type: ";
    buf[pos..pos + status.len()].copy_from_slice(status);
    pos += status.len();

    let ct_bytes = content_type.as_bytes();
    buf[pos..pos + ct_bytes.len()].copy_from_slice(ct_bytes);
    pos += ct_bytes.len();

    let mid = b"\r\nContent-Encoding: gzip\r\nContent-Length: ";
    buf[pos..pos + mid.len()].copy_from_slice(mid);
    pos += mid.len();

    pos += write_usize(content_length, &mut buf[pos..]);

    let tail = b"\r\nConnection: close\r\n\r\n";
    buf[pos..pos + tail.len()].copy_from_slice(tail);
    pos += tail.len();

    pos
}

/// Builds an HTTP 404 Not Found response header into `buf`.
///
/// Returns the number of bytes written.
fn build_http_404(buf: &mut [u8]) -> usize {
    let resp = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    buf[..resp.len()].copy_from_slice(resp);
    resp.len()
}

/// Serves a gzip-compressed static file or HTTP 404 over the TCP socket.
///
/// Matches the request path by extension to one of the provided
/// [`StaticAssets`]. Sends the response with `Content-Encoding: gzip`
/// so the browser decompresses transparently.
///
/// # Errors
///
/// Returns [`ws_framing::WsError`] if any TCP write fails.
pub async fn serve_static(
    socket: &mut TcpSocket<'_>,
    path: &str,
    assets: &StaticAssets,
) -> Result<(), ws_framing::WsError> {
    let mut header_buf = [0u8; 256];

    match match_asset(assets, path) {
        Some(asset) if !asset.content.is_empty() => {
            let n = build_http_200(&mut header_buf, asset.content_type, asset.content.len());
            ws_framing::write_all_to_socket(socket, &header_buf[..n]).await?;
            ws_framing::write_all_to_socket(socket, asset.content).await?;
        }
        _ => {
            let n = build_http_404(&mut header_buf);
            ws_framing::write_all_to_socket(socket, &header_buf[..n]).await?;
        }
    }
    Ok(())
}
