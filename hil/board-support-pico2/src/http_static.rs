//! Board-specific static asset embedding for the combined, HIL, and DAG frontends.

use hil_firmware_support::http_static::StaticAssets;

// -- Combined frontend (Leptos: HIL + DAG + Deploy) --
#[cfg(has_combined_frontend)]
static COMBINED_INDEX_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/combined_index.html.gz"));
#[cfg(has_combined_frontend)]
static COMBINED_JS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/combined_app.js.gz"));
#[cfg(has_combined_frontend)]
static COMBINED_WASM_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/combined_app_wasm.gz"));
#[cfg(has_combined_frontend)]
static COMBINED_CSS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/combined_style.css.gz"));

#[cfg(not(has_combined_frontend))]
static COMBINED_INDEX_GZ: &[u8] = b"";
#[cfg(not(has_combined_frontend))]
static COMBINED_JS_GZ: &[u8] = b"";
#[cfg(not(has_combined_frontend))]
static COMBINED_WASM_GZ: &[u8] = b"";
#[cfg(not(has_combined_frontend))]
static COMBINED_CSS_GZ: &[u8] = b"";

// -- HIL dashboard frontend (Leptos, original) --
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

// -- DAG editor frontend (plain JS fallback) --
#[cfg(has_dag_frontend)]
static DAG_INDEX_HTML_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dag_index.html.gz"));
#[cfg(has_dag_frontend)]
static DAG_EDITOR_JS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dag_editor.js.gz"));

#[cfg(not(has_dag_frontend))]
static DAG_INDEX_HTML_GZ: &[u8] = b"";
#[cfg(not(has_dag_frontend))]
static DAG_EDITOR_JS_GZ: &[u8] = b"";

/// Select the best available frontend assets.
///
/// Priority: combined > DAG > HIL.
pub fn assets() -> StaticAssets {
    if !COMBINED_INDEX_GZ.is_empty() {
        // Combined Leptos frontend (HIL + DAG + Deploy in one)
        StaticAssets {
            index_html: COMBINED_INDEX_GZ,
            app_js: COMBINED_JS_GZ,
            app_wasm: COMBINED_WASM_GZ,
            style_css: COMBINED_CSS_GZ,
        }
    } else if !DAG_INDEX_HTML_GZ.is_empty() {
        // DAG editor (plain JS, no WASM)
        StaticAssets {
            index_html: DAG_INDEX_HTML_GZ,
            app_js: DAG_EDITOR_JS_GZ,
            app_wasm: APP_WASM_GZ,
            style_css: STYLE_CSS_GZ,
        }
    } else {
        // HIL dashboard only
        StaticAssets {
            index_html: INDEX_HTML_GZ,
            app_js: APP_JS_GZ,
            app_wasm: APP_WASM_GZ,
            style_css: STYLE_CSS_GZ,
        }
    }
}
