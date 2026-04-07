#!/usr/bin/env bash
# Hot-reloading dev server: rebuilds WASM + JS/CSS on source changes.
#
# Usage: ./scripts/dev.sh [--port 3000]
#
# Watches:
#   crates/rustsim/src/**  → wasm-pack build → www-dataflow/pkg/
#   www-dataflow/src/**    → esbuild + tailwind (via npm run watch)
#
# Serves via native-server with auto-restart on WASM rebuild.

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

PORT="${1:-3000}"
WWW_DIR="$(pwd)/www-dataflow"

cleanup() {
  kill "$WATCH_PID" "$TS_PID" "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

# 1. Initial build
echo "▸ Building WASM..."
wasm-pack build crates/rustsim --target web --out-dir ../../www-dataflow/pkg --release 2>&1 | tail -1
echo "▸ Building frontend..."
(cd www-dataflow && npm run build 2>&1 | tail -1)

# 2. Start JS/CSS file watcher (esbuild + tailwind in watch mode)
(cd www-dataflow && npm run watch 2>&1) &
TS_PID=$!

# 3. Start native-server
start_server() {
  cargo run -p native-server -- --www-dir "$WWW_DIR" --port "$PORT" --no-open 2>&1 &
  SERVER_PID=$!
}
start_server

# 4. Watch Rust sources → rebuild WASM → restart server
cargo watch \
  -w crates/rustsim/src \
  -w module-traits/src \
  -w dag-core/src \
  -s "echo '▸ Rebuilding WASM...' && \
      wasm-pack build crates/rustsim --target web --out-dir ../../www-dataflow/pkg --release 2>&1 | tail -1 && \
      echo '▸ WASM rebuilt. Restart server...' && \
      kill $SERVER_PID 2>/dev/null; sleep 0.5 && \
      cargo run -p native-server -- --www-dir $WWW_DIR --port $PORT --no-open 2>&1 &
      SERVER_PID=\$!" \
  2>&1 &
WATCH_PID=$!

echo ""
echo "═══════════════════════════════════════════"
echo "  Dev server: http://localhost:$PORT"
echo "  WASM watch: crates/rustsim/src/**"
echo "  TS watch:   www-dataflow/src/**"
echo "  Ctrl+C to stop"
echo "═══════════════════════════════════════════"
echo ""

wait
