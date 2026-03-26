#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$ROOT/dag-runtime/src/generated"

mkdir -p "$OUT_DIR"

echo "=== Building WASM module ==="
cd "$ROOT"
wasm-pack build --target web --release 2>/dev/null || echo "WARN: wasm-pack build failed, using existing pkg/ if available"

echo "=== Building DAG editor JS ==="
cd "$ROOT/www"
npx esbuild dag/dag-editor.ts --bundle --format=esm --target=es2022 --minify --external:../pkg/rustcam.js --outfile=dag/dag-editor.js 2>/dev/null || echo "WARN: esbuild failed"

echo "=== Gzipping assets ==="
cd "$ROOT"

# Collect source files
declare -A ASSETS
ASSETS[index_html]="www/dag/index.html"
ASSETS[editor_js]="www/dag/dag-editor.js"

# WASM files are optional (may not exist if wasm-pack failed)
if [ -f "pkg/rustcam_bg.wasm" ]; then
    ASSETS[wasm_bg]="pkg/rustcam_bg.wasm"
fi
if [ -f "pkg/rustcam.js" ]; then
    ASSETS[wasm_js]="pkg/rustcam.js"
fi

echo "=== Generating Rust source ==="

cat > "$OUT_DIR/mod.rs" << 'HEADER'
//! Auto-generated embedded web assets (gzipped).
//! Do not edit manually — regenerate with `tools/embed-assets.sh`.

HEADER

for key in "${!ASSETS[@]}"; do
    src="${ASSETS[$key]}"
    if [ ! -f "$src" ]; then
        echo "WARN: $src not found, skipping"
        continue
    fi

    gz_file=$(mktemp)
    gzip -9 -c "$src" > "$gz_file"

    size=$(wc -c < "$gz_file")
    orig_size=$(wc -c < "$src")

    echo "  $key: $orig_size -> $size bytes ($(( size * 100 / (orig_size > 0 ? orig_size : 1) ))%)"

    # Convert to Rust byte array
    echo "/// $src ($orig_size bytes original, $size bytes gzipped)" >> "$OUT_DIR/mod.rs"
    echo "pub const ${key^^}: &[u8] = &[" >> "$OUT_DIR/mod.rs"
    xxd -i < "$gz_file" >> "$OUT_DIR/mod.rs"
    echo "];" >> "$OUT_DIR/mod.rs"
    echo "" >> "$OUT_DIR/mod.rs"

    rm "$gz_file"
done

echo "=== Content types ==="
cat >> "$OUT_DIR/mod.rs" << 'CONTENT_TYPES'
/// Map file extension to content type
pub fn content_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".css") {
        "text/css"
    } else {
        "application/octet-stream"
    }
}

/// Look up an embedded asset by URL path
pub fn lookup(path: &str) -> Option<(&'static [u8], &'static str)> {
    match path {
        "/" | "/index.html" => Some((INDEX_HTML, "text/html; charset=utf-8")),
        "/dag-editor.js" => Some((EDITOR_JS, "application/javascript")),
        #[cfg(feature = "wasm")]
        "/rustcam_bg.wasm" => Some((WASM_BG, "application/wasm")),
        #[cfg(feature = "wasm")]
        "/rustcam.js" => Some((WASM_JS, "application/javascript")),
        _ => None,
    }
}
CONTENT_TYPES

echo "=== Done: $OUT_DIR/mod.rs ==="
