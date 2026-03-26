//! Board-specific static asset embedding for the HIL dashboard and DAG editor.

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

// DAG editor frontend (served as fallback when HIL dashboard is not built)
#[cfg(has_dag_frontend)]
static DAG_INDEX_HTML_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dag_index.html.gz"));
#[cfg(has_dag_frontend)]
static DAG_EDITOR_JS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dag_editor.js.gz"));

#[cfg(not(has_dag_frontend))]
static DAG_INDEX_HTML_GZ: &[u8] = b"";
#[cfg(not(has_dag_frontend))]
static DAG_EDITOR_JS_GZ: &[u8] = b"";

pub fn assets() -> StaticAssets {
    // Prefer DAG editor frontend; fall back to HIL dashboard
    let (index, js) = if !DAG_INDEX_HTML_GZ.is_empty() {
        (DAG_INDEX_HTML_GZ, DAG_EDITOR_JS_GZ)
    } else {
        (INDEX_HTML_GZ, APP_JS_GZ)
    };

    StaticAssets {
        index_html: index,
        app_js: js,
        app_wasm: APP_WASM_GZ,
        style_css: STYLE_CSS_GZ,
    }
}
