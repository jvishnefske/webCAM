//! Board-specific static asset embedding for the HIL dashboard.

use hil_firmware_support::http_static::StaticAssets;

#[cfg(has_frontend)]
static INDEX_HTML_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.html.gz"));
#[cfg(has_frontend)]
static APP_JS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.js.gz"));
#[cfg(has_frontend)]
static APP_WASM_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app_bg.wasm.gz"));
#[cfg(has_frontend)]
static STYLE_CSS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/style.css.gz"));

#[cfg(not(has_frontend))]
static INDEX_HTML_GZ: &[u8] = b"";
#[cfg(not(has_frontend))]
static APP_JS_GZ: &[u8] = b"";
#[cfg(not(has_frontend))]
static APP_WASM_GZ: &[u8] = b"";
#[cfg(not(has_frontend))]
static STYLE_CSS_GZ: &[u8] = b"";

pub fn assets() -> StaticAssets {
    StaticAssets {
        index_html: INDEX_HTML_GZ,
        app_js: APP_JS_GZ,
        app_wasm: APP_WASM_GZ,
        style_css: STYLE_CSS_GZ,
    }
}
