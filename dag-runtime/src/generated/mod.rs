//! Placeholder for auto-generated embedded web assets.
//! Run `tools/embed-assets.sh` to generate real assets.

/// Placeholder: empty gzipped index.html
pub const INDEX_HTML: &[u8] = &[];

/// Placeholder: empty gzipped editor JS
pub const EDITOR_JS: &[u8] = &[];

/// Map URL path to embedded asset
pub fn lookup(path: &str) -> Option<(&'static [u8], &'static str)> {
    match path {
        "/" | "/index.html" if !INDEX_HTML.is_empty() => {
            Some((INDEX_HTML, "text/html; charset=utf-8"))
        }
        "/dag-editor.js" if !EDITOR_JS.is_empty() => {
            Some((EDITOR_JS, "application/javascript"))
        }
        _ => None,
    }
}
