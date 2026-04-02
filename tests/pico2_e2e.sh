#!/usr/bin/env bash
# End-to-end test suite for the Pico2 DAG/PubSub HTTP API.
# Requires a flashed Pico2 at 169.254.1.61:8080.
#
# Usage: ./tests/pico2_e2e.sh [host:port]

set -euo pipefail

BASE="${1:-http://169.254.1.61:8080}"
PASS=0
FAIL=0

# --- helpers ---

req() {
  curl -s -m 5 -H "Content-Length: ${3:-0}" "$@"
}

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label"
    echo "    expected: $expected"
    echo "    actual:   $actual"
    FAIL=$((FAIL + 1))
  fi
}

assert_contains() {
  local label="$1" needle="$2" haystack="$3"
  if echo "$haystack" | grep -q "$needle"; then
    echo "  PASS: $label"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $label (missing '$needle' in '$haystack')"
    FAIL=$((FAIL + 1))
  fi
}

pause() { sleep 0.5; }

# --- CBOR DAGs (pre-encoded to temp files) ---

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Const(42.0) → Publish("sensor", 0)
printf '\x82\x82\x00\xfb\x40\x45\x00\x00\x00\x00\x00\x00\x83\x0b\x66\x73\x65\x6e\x73\x6f\x72\x00' > "$TMPDIR/dag_simple.cbor"

# Const(10.0) → Publish("alpha", 0), Subscribe("alpha"), Publish("beta", 2)
printf '\x84\x82\x00\xfb\x40\x24\x00\x00\x00\x00\x00\x00\x83\x0b\x65\x61\x6c\x70\x68\x61\x00\x82\x0a\x65\x61\x6c\x70\x68\x61\x83\x0b\x64\x62\x65\x74\x61\x02' > "$TMPDIR/dag_roundtrip.cbor"

# --- Test 1: Deploy simple DAG (also resets tick counter) ---

echo "Test 1: Deploy Const(42) → Publish(sensor)"
RESP=$(req -X POST --data-binary "@$TMPDIR/dag_simple.cbor" "$BASE/api/dag")
assert_contains "deploy ok" '"ok":true' "$RESP"
assert_contains "deploy nodes" '"nodes":2' "$RESP"
pause

# --- Test 2: Tick ---

echo "Test 2: Tick"
RESP=$(req -X POST "$BASE/api/tick")
assert_contains "tick ok" '"ok":true' "$RESP"
pause

# --- Test 3: PubSub value ---

echo "Test 3: PubSub sensor=42"
RESP=$(req "$BASE/api/pubsub")
assert_contains "sensor value" '"sensor":42' "$RESP"
pause

# --- Test 4: Status after tick ---

echo "Test 4: Status after tick"
RESP=$(req "$BASE/api/status")
assert_contains "loaded true" '"loaded":true' "$RESP"
assert_contains "ticks 1" '"ticks":1' "$RESP"
pause

# --- Test 5: Multiple ticks ---

echo "Test 5: Multiple ticks"
req -X POST "$BASE/api/tick" > /dev/null; pause
req -X POST "$BASE/api/tick" > /dev/null; pause
RESP=$(req "$BASE/api/status")
assert_contains "3 ticks total" '"ticks":3' "$RESP"
pause

# --- Test 6: Round-trip DAG ---

echo "Test 6: Deploy round-trip DAG"
RESP=$(req -X POST --data-binary "@$TMPDIR/dag_roundtrip.cbor" "$BASE/api/dag")
assert_contains "roundtrip deploy" '"nodes":4' "$RESP"
pause

# --- Test 7: Tick 1 of round-trip ---

echo "Test 7: Round-trip tick 1 (alpha=10, beta=0)"
req -X POST "$BASE/api/tick" > /dev/null; pause
RESP=$(req "$BASE/api/pubsub")
assert_contains "alpha=10" '"alpha":10' "$RESP"
assert_contains "beta=0" '"beta":0' "$RESP"
pause

# --- Test 8: Tick 2 of round-trip ---

echo "Test 8: Round-trip tick 2 (beta=10)"
req -X POST "$BASE/api/tick" > /dev/null; pause
RESP=$(req "$BASE/api/pubsub")
assert_contains "beta=10" '"beta":10' "$RESP"
pause

# --- Test 9: Debug toggle ---

echo "Test 9: Debug toggle"
RESP=$(req -X POST "$BASE/api/debug")
assert_contains "debug toggle" '"debug":' "$RESP"
pause

# Turn debug off again
req -X POST "$BASE/api/debug" > /dev/null; pause

# --- Test 10: HTTP serves frontend ---

echo "Test 10: Frontend served"
RESP=$(req --compressed "$BASE/")
assert_contains "html served" "MCU Control Panel" "$RESP"
pause

# --- Test 11: WASM served ---

echo "Test 11: WASM binary served"
# Just check the JS loader is served (contains the WASM import)
RESP=$(curl -s -m 5 --compressed "$BASE/" | grep -o "combined-frontend" | head -1)
assert_eq "wasm reference" "combined-frontend" "$RESP"

# --- Summary ---

echo ""
echo "=============================="
echo "  Results: $PASS passed, $FAIL failed"
echo "=============================="

[ "$FAIL" -eq 0 ] && exit 0 || exit 1
