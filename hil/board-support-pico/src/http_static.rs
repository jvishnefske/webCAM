//! Board-specific static asset embedding for the HIL dashboard.
//!
//! Embeds gzip-compressed frontend assets built by Trunk during firmware
//! compilation. Provides them as a [`hil_firmware_support::http_static::StaticAssets`]
//! via the [`assets`] function.

use hil_firmware_support::http_static::StaticAssets;

/// Gzip-compressed index.html.
#[cfg(has_frontend)]
static INDEX_HTML_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.html.gz"));

/// Gzip-compressed JavaScript loader.
#[cfg(has_frontend)]
static APP_JS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.js.gz"));

/// Gzip-compressed WebAssembly binary.
#[cfg(has_frontend)]
static APP_WASM_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app_bg.wasm.gz"));

/// Gzip-compressed CSS stylesheet.
#[cfg(has_frontend)]
static STYLE_CSS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/style.css.gz"));

/// Placeholder when frontend is not built.
#[cfg(not(has_frontend))]
static INDEX_HTML_GZ: &[u8] = b"";

/// Placeholder when frontend is not built.
#[cfg(not(has_frontend))]
static APP_JS_GZ: &[u8] = b"";

/// Placeholder when frontend is not built.
#[cfg(not(has_frontend))]
static APP_WASM_GZ: &[u8] = b"";

/// Placeholder when frontend is not built.
#[cfg(not(has_frontend))]
static STYLE_CSS_GZ: &[u8] = b"";

/// Returns the static assets for the HIL dashboard frontend.
///
/// When `has_frontend` cfg is set (i.e., `hil-frontend/dist/` exists at
/// build time), returns the gzip-compressed assets. Otherwise, returns
/// empty slices that the server will respond to with HTTP 404.
pub fn assets() -> StaticAssets {
    StaticAssets {
        index_html: INDEX_HTML_GZ,
        app_js: APP_JS_GZ,
        app_wasm: APP_WASM_GZ,
        style_css: STYLE_CSS_GZ,
    }
}
