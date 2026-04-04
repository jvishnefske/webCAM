# PubSub Telemetry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Frontend publishes graph CRUD events (block add/remove/update, connection create/delete, graph reset) over WebSocket using CBOR. An optional server subscribes and logs them. Debug mode (off by default) adds sequence numbers and timestamps.

**Architecture:** New `TelemetryPublisher` class in the frontend encodes events as CBOR (tags 50–56) and sends them over the HilClient's WebSocket. The native-server's `dispatch_cbor` function gains a new arm for tags 50–56 that logs events and stores them in a ring buffer. A new `GET /api/telemetry` endpoint exposes the buffer.

**Tech Stack:** TypeScript (cbor-x), Rust (minicbor, axum, serde_json), vitest, cargo test

**Spec:** `docs/superpowers/specs/2026-04-03-pubsub-telemetry-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `www/src/dataflow/telemetry.ts` | Create | `TelemetryPublisher` — encode + send CBOR telemetry events |
| `www/src/dataflow/telemetry.test.ts` | Create | Unit tests for TelemetryPublisher |
| `www/src/dataflow/graph.ts` | Modify | Wire publish calls into addBlock/removeBlock/connect/disconnect/updateBlock |
| `www/src/dataflow/hil-client.ts` | Modify | Expose raw WebSocket reference for telemetry attach |
| `www/src/dataflow/index.ts` | Modify | Instantiate publisher, wire to HilClient lifecycle |
| `hil/native-server/src/lib.rs` | Modify | Handle tags 50–56, ring buffer, `GET /api/telemetry` |

---

### Task 1: TelemetryPublisher — encode and send CBOR events

**Files:**
- Create: `www/src/dataflow/telemetry.ts`
- Create: `www/src/dataflow/telemetry.test.ts`

- [ ] **Step 1: Write the failing test for TelemetryPublisher.publish()**

Create `www/src/dataflow/telemetry.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { decode } from 'cbor-x';
import { TelemetryPublisher } from './telemetry';

describe('TelemetryPublisher', () => {
  let publisher: TelemetryPublisher;
  let sentMessages: Uint8Array[];
  let mockWs: { send: (data: Uint8Array) => void; readyState: number };

  beforeEach(() => {
    sentMessages = [];
    mockWs = {
      send: (data: Uint8Array) => sentMessages.push(data),
      readyState: 1, // WebSocket.OPEN
    };
    publisher = new TelemetryPublisher();
    publisher.attach(mockWs as unknown as WebSocket);
    publisher.setEnabled(true);
  });

  it('encodes block-added as tag 50', () => {
    publisher.publish({ tag: 50, blockId: 7, blockType: 'constant', config: { value: 1.0 }, x: 100, y: 200 });
    expect(sentMessages).toHaveLength(1);
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(50);
    expect(msg[1]).toBe(7);
    expect(msg[2]).toBe('constant');
    expect(msg[4]).toBe(100);
    expect(msg[5]).toBe(200);
  });

  it('encodes block-removed as tag 51', () => {
    publisher.publish({ tag: 51, blockId: 3 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(51);
    expect(msg[1]).toBe(3);
  });

  it('encodes block-updated as tag 52', () => {
    publisher.publish({ tag: 52, blockId: 5, blockType: 'gain', config: { param1: 2.0 } });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(52);
    expect(msg[1]).toBe(5);
    expect(msg[2]).toBe('gain');
  });

  it('encodes connection-created as tag 53', () => {
    publisher.publish({ tag: 53, fromBlock: 1, fromPort: 0, toBlock: 2, toPort: 0, channelId: 10 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(53);
    expect(msg[1]).toBe(1);
    expect(msg[5]).toBe(10);
  });

  it('encodes connection-removed as tag 54', () => {
    publisher.publish({ tag: 54, channelId: 10 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(54);
    expect(msg[1]).toBe(10);
  });

  it('encodes graph-reset as tag 55', () => {
    publisher.publish({ tag: 55 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(55);
  });

  it('does not send when disabled', () => {
    publisher.setEnabled(false);
    publisher.publish({ tag: 50, blockId: 1, blockType: 'constant', config: {}, x: 0, y: 0 });
    expect(sentMessages).toHaveLength(0);
  });

  it('does not send when detached', () => {
    publisher.detach();
    publisher.publish({ tag: 51, blockId: 1 });
    expect(sentMessages).toHaveLength(0);
  });

  it('wraps in debug envelope (tag 56) when debug enabled', () => {
    publisher.setDebug(true);
    publisher.publish({ tag: 51, blockId: 9 });
    const msg = decode(sentMessages[0]) as Record<number, unknown>;
    expect(msg[0]).toBe(56);
    expect(typeof msg[1]).toBe('number'); // seq
    expect(typeof msg[2]).toBe('number'); // timestampMs
    const inner = msg[3] as Record<number, unknown>;
    expect(inner[0]).toBe(51);
    expect(inner[1]).toBe(9);
  });

  it('increments sequence number in debug mode', () => {
    publisher.setDebug(true);
    publisher.publish({ tag: 55 });
    publisher.publish({ tag: 55 });
    const msg1 = decode(sentMessages[0]) as Record<number, unknown>;
    const msg2 = decode(sentMessages[1]) as Record<number, unknown>;
    expect((msg2[1] as number) - (msg1[1] as number)).toBe(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd www && npx vitest run src/dataflow/telemetry.test.ts`
Expected: FAIL — module `./telemetry` not found

- [ ] **Step 3: Implement TelemetryPublisher**

Create `www/src/dataflow/telemetry.ts`:

```typescript
/** Publishes dataflow graph CRUD events as CBOR over WebSocket. */

import { encode } from 'cbor-x';

// ── Telemetry CBOR tags (50–59 range) ────────────────────────────
export const TAG_BLOCK_ADDED       = 50;
export const TAG_BLOCK_REMOVED     = 51;
export const TAG_BLOCK_UPDATED     = 52;
export const TAG_CONNECTION_CREATED = 53;
export const TAG_CONNECTION_REMOVED = 54;
export const TAG_GRAPH_RESET       = 55;
export const TAG_DEBUG_WRAPPER     = 56;

export type TelemetryEvent =
  | { tag: 50; blockId: number; blockType: string; config: unknown; x: number; y: number }
  | { tag: 51; blockId: number }
  | { tag: 52; blockId: number; blockType: string; config: unknown }
  | { tag: 53; fromBlock: number; fromPort: number; toBlock: number; toPort: number; channelId: number }
  | { tag: 54; channelId: number }
  | { tag: 55 };

export class TelemetryPublisher {
  private ws: WebSocket | null = null;
  private enabled = false;
  private debug = false;
  private seq = 0;

  /** Bind to an open WebSocket connection. */
  attach(ws: WebSocket): void {
    this.ws = ws;
  }

  /** Detach (connection closed or telemetry disabled). */
  detach(): void {
    this.ws = null;
  }

  /** Enable/disable telemetry publishing. */
  setEnabled(on: boolean): void {
    this.enabled = on;
  }

  /** Toggle debug CRUD log (adds seq + timestamp wrapper). */
  setDebug(on: boolean): void {
    this.debug = on;
    if (on) this.seq = 0;
  }

  /** Publish a CRUD event. No-op if disabled or no WebSocket. */
  publish(event: TelemetryEvent): void {
    if (!this.enabled || !this.ws || this.ws.readyState !== WebSocket.OPEN) return;

    const payload = this.encodeEvent(event);

    if (this.debug) {
      const wrapped: Record<number, unknown> = {
        0: TAG_DEBUG_WRAPPER,
        1: this.seq++,
        2: performance.now(),
        3: payload,
      };
      this.ws.send(encode(wrapped));
    } else {
      this.ws.send(encode(payload));
    }
  }

  private encodeEvent(event: TelemetryEvent): Record<number, unknown> {
    switch (event.tag) {
      case 50:
        return { 0: 50, 1: event.blockId, 2: event.blockType, 3: event.config, 4: event.x, 5: event.y };
      case 51:
        return { 0: 51, 1: event.blockId };
      case 52:
        return { 0: 52, 1: event.blockId, 2: event.blockType, 3: event.config };
      case 53:
        return { 0: 53, 1: event.fromBlock, 2: event.fromPort, 3: event.toBlock, 4: event.toPort, 5: event.channelId };
      case 54:
        return { 0: 54, 1: event.channelId };
      case 55:
        return { 0: 55 };
    }
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd www && npx vitest run src/dataflow/telemetry.test.ts`
Expected: All 9 tests PASS

- [ ] **Step 5: Commit**

```bash
git add www/src/dataflow/telemetry.ts www/src/dataflow/telemetry.test.ts
git commit -m "feat: add TelemetryPublisher with CBOR event encoding"
```

---

### Task 2: Expose HilClient WebSocket for telemetry attach

**Files:**
- Modify: `www/src/dataflow/hil-client.ts`

- [ ] **Step 1: Add a public getter for the raw WebSocket**

In `www/src/dataflow/hil-client.ts`, add after the `connected` getter (line 70):

```typescript
  /** Raw WebSocket reference (for telemetry attach). Null if not connected. */
  get socket(): WebSocket | null {
    return this.ws;
  }
```

- [ ] **Step 2: Build to verify no errors**

Run: `cd www && npm run build`
Expected: Clean build

- [ ] **Step 3: Commit**

```bash
git add www/src/dataflow/hil-client.ts
git commit -m "feat: expose HilClient.socket for telemetry attach"
```

---

### Task 3: Wire TelemetryPublisher into DataflowManager mutations

**Files:**
- Modify: `www/src/dataflow/graph.ts`

- [ ] **Step 1: Add telemetry property and import**

At the top of `www/src/dataflow/graph.ts`, add the import:

```typescript
import type { TelemetryPublisher, TelemetryEvent } from './telemetry.js';
```

In the `DataflowManager` class, add a public property after `positions` (line 19):

```typescript
  /** Optional telemetry publisher for CRUD events. */
  telemetry: TelemetryPublisher | null = null;
```

- [ ] **Step 2: Publish events from mutation methods**

In `addBlock()` (after `this.positions.set(id, { x, y })` on line 34), add:

```typescript
    this.telemetry?.publish({ tag: 50, blockId: id, blockType: blockType, config, x, y });
```

In `updateBlock()` (after `dataflow_update_block(...)` on line 39), add:

```typescript
    this.telemetry?.publish({ tag: 52, blockId: blockId, blockType: blockType, config });
```

In `removeBlock()` (after `this.positions.delete(blockId)` on line 44), add:

```typescript
    this.telemetry?.publish({ tag: 51, blockId: blockId });
```

In `connect()` (line 47-49), capture the return value and publish:

```typescript
  connect(fromBlock: number, fromPort: number, toBlock: number, toPort: number): number {
    const id = dataflow_connect(this.graphId, fromBlock, fromPort, toBlock, toPort);
    this.telemetry?.publish({ tag: 53, fromBlock, fromPort, toBlock, toPort, channelId: id });
    return id;
  }
```

In `disconnect()` (after `dataflow_disconnect(...)` on line 52), add:

```typescript
    this.telemetry?.publish({ tag: 54, channelId: channelId });
```

- [ ] **Step 3: Build and run all tests**

Run: `cd www && npm run build && npx vitest run`
Expected: Build clean, all tests PASS

- [ ] **Step 4: Commit**

```bash
git add www/src/dataflow/graph.ts
git commit -m "feat: wire telemetry publish into DataflowManager mutations"
```

---

### Task 4: Instantiate publisher and wire to HilClient lifecycle

**Files:**
- Modify: `www/src/dataflow/index.ts`

- [ ] **Step 1: Import and instantiate TelemetryPublisher**

At the top of `www/src/dataflow/index.ts`, add the import after existing imports:

```typescript
import { TelemetryPublisher } from './telemetry.js';
```

After the `let triggerAutoSave` declaration (line 25), add:

```typescript
let telemetry: TelemetryPublisher | null = null;
```

- [ ] **Step 2: Create publisher in initDataflow() and attach to mgr**

Inside `initDataflow()`, after `editor = new DataflowEditor(container, mgr)` (line 29), add:

```typescript
  telemetry = new TelemetryPublisher();
  mgr.telemetry = telemetry;
```

Also wire it into the `resetEditor` function (after `mgr = new DataflowManager(dt)` on line 72):

```typescript
    if (telemetry) mgr.telemetry = telemetry;
```

- [ ] **Step 3: Attach/detach telemetry in the HIL connection lifecycle**

In the `setupHilConnection()` function, inside the `hilClient.onConnect` callback (after `deployBtn.disabled = false`), add:

```typescript
      if (telemetry && hilClient?.socket) {
        telemetry.attach(hilClient.socket);
        telemetry.setEnabled(true);
      }
```

Inside the `hilClient.onDisconnect` callback (after `deployBtn.disabled = true`), add:

```typescript
      telemetry?.detach();
```

- [ ] **Step 4: Publish graph-reset on resetEditor**

In the `resetEditor` function (after `mgr = new DataflowManager(dt)`), add:

```typescript
    telemetry?.publish({ tag: 55 });
```

- [ ] **Step 5: Build and test**

Run: `cd www && npm run build && npx vitest run`
Expected: Build clean, all tests PASS

- [ ] **Step 6: Commit**

```bash
git add www/src/dataflow/index.ts
git commit -m "feat: wire TelemetryPublisher into HilClient lifecycle"
```

---

### Task 5: Server-side telemetry handler and ring buffer

**Files:**
- Modify: `hil/native-server/src/lib.rs`

- [ ] **Step 1: Write the failing test for telemetry dispatch**

Add at the bottom of the `tests` module in `hil/native-server/src/lib.rs`:

```rust
    fn encode_telemetry_block_added(block_id: u32, block_type: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(3).unwrap();
        enc.u32(0).unwrap().u32(50).unwrap();
        enc.u32(1).unwrap().u32(block_id).unwrap();
        enc.u32(2).unwrap().str(block_type).unwrap();
        buf
    }

    #[test]
    fn test_telemetry_block_added_logged() {
        let state = make_state();
        let req = encode_telemetry_block_added(7, "constant");
        let resp = dispatch_cbor(&state, &req);
        // Telemetry events do not produce a response
        assert!(resp.is_none());
        // Event should be in the ring buffer
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 50);
        assert_eq!(st.telemetry_log[0].payload["blockId"], 7);
        assert_eq!(st.telemetry_log[0].payload["blockType"], "constant");
    }

    #[test]
    fn test_telemetry_graph_reset() {
        let state = make_state();
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).unwrap();
        enc.u32(0).unwrap().u32(55).unwrap();
        let resp = dispatch_cbor(&state, &buf);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 55);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p native-server test_telemetry`
Expected: FAIL — `telemetry_log` field not found on `ServerState`

- [ ] **Step 3: Add TelemetryEntry struct and ring buffer to ServerState**

In `hil/native-server/src/lib.rs`, add after the imports:

```rust
use std::collections::VecDeque;
```

Add the struct before `ServerState`:

```rust
/// A logged telemetry event from the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetryEntry {
    pub seq: u32,
    pub timestamp_ms: f64,
    pub tag: u8,
    pub payload: serde_json::Value,
}
```

Add to `ServerState` (after `i2c_buses`):

```rust
    /// Ring buffer of telemetry events from the frontend (last 256).
    pub telemetry_log: VecDeque<TelemetryEntry>,
    telemetry_seq: u32,
```

In `ServerState::new()`, add to the struct literal:

```rust
            telemetry_log: VecDeque::new(),
            telemetry_seq: 0,
```

- [ ] **Step 4: Handle tags 50–56 in dispatch_cbor**

In the `match tag` block of `dispatch_cbor()`, add before the `_ =>` arm:

```rust
        50..=56 => {
            handle_telemetry(state, tag, data);
            None // telemetry events don't produce a response
        }
```

Add the handler function:

```rust
fn handle_telemetry(state: &SharedState, tag: u32, data: &[u8]) {
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());

    // Parse the CBOR map into a JSON value for storage
    let payload = parse_telemetry_payload(tag, data);

    // For debug-wrapped events (tag 56), extract inner fields
    let (actual_tag, seq, ts) = if tag == 56 {
        let s = payload.get("seq").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let t = payload.get("timestampMs").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let inner_tag = payload.get("innerTag").and_then(|v| v.as_u64()).unwrap_or(56) as u8;
        (inner_tag, s, t)
    } else {
        let seq = st.telemetry_seq;
        st.telemetry_seq += 1;
        (tag as u8, seq, 0.0)
    };

    let entry = TelemetryEntry {
        seq,
        timestamp_ms: ts,
        tag: actual_tag,
        payload,
    };

    if st.telemetry_log.len() >= 256 {
        st.telemetry_log.pop_front();
    }
    st.telemetry_log.push_back(entry);
}

fn parse_telemetry_payload(tag: u32, data: &[u8]) -> serde_json::Value {
    let mut dec = minicbor::Decoder::new(data);
    let mut map = serde_json::Map::new();

    let n = match dec.map() {
        Ok(Some(n)) => n,
        _ => return serde_json::Value::Object(map),
    };

    for _ in 0..n {
        let key = match dec.u32() {
            Ok(k) => k,
            _ => break,
        };
        if key == 0 {
            // Skip the tag field (already known)
            let _ = dec.u32();
            continue;
        }
        let field_name = match (tag, key) {
            (50, 1) | (51, 1) | (52, 1) => "blockId",
            (50, 2) | (52, 2) => "blockType",
            (50, 3) | (52, 3) => "config",
            (50, 4) => "x",
            (50, 5) => "y",
            (53, 1) => "fromBlock",
            (53, 2) => "fromPort",
            (53, 3) => "toBlock",
            (53, 4) => "toPort",
            (53, 5) | (54, 1) => "channelId",
            (56, 1) => "seq",
            (56, 2) => "timestampMs",
            (56, 3) => "inner",
            _ => "unknown",
        };
        // Decode value based on CBOR data type
        match dec.datatype() {
            Ok(minicbor::data::Type::U8 | minicbor::data::Type::U16 | minicbor::data::Type::U32) => {
                if let Ok(v) = dec.u32() {
                    map.insert(field_name.to_string(), serde_json::json!(v));
                }
            }
            Ok(minicbor::data::Type::F32 | minicbor::data::Type::F64) => {
                if let Ok(v) = dec.f64() {
                    map.insert(field_name.to_string(), serde_json::json!(v));
                }
            }
            Ok(minicbor::data::Type::String) => {
                if let Ok(v) = dec.str() {
                    map.insert(field_name.to_string(), serde_json::json!(v));
                }
            }
            _ => break,
        }
    }
    serde_json::Value::Object(map)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p native-server`
Expected: All tests PASS (existing + 2 new telemetry tests)

- [ ] **Step 6: Commit**

```bash
git add hil/native-server/src/lib.rs
git commit -m "feat: server-side telemetry event handler with ring buffer"
```

---

### Task 6: GET /api/telemetry endpoint

**Files:**
- Modify: `hil/native-server/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[tokio::test]
    async fn test_get_telemetry_empty() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .uri("/api/telemetry")
            .body(Body::empty())
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        let body = json_body(resp).await;
        assert!(body.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_telemetry_since() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        // Inject events directly into state
        {
            let state: SharedState = Arc::new(Mutex::new(ServerState::new()));
            // We'll test via the dispatch path instead
        }
        let req = axum::http::Request::builder()
            .uri("/api/telemetry?since=999")
            .body(Body::empty())
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        let body = json_body(resp).await;
        assert!(body.as_array().unwrap().is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p native-server test_get_telemetry`
Expected: FAIL — 404 (route not registered)

- [ ] **Step 3: Add the /api/telemetry route and handler**

In the `app()` function, add after the `/api/debug` route:

```rust
        .route("/api/telemetry", get(get_telemetry))
```

Add the `axum::extract::Query` import at the top:

```rust
use axum::extract::Query;
```

Add the handler:

```rust
#[derive(serde::Deserialize)]
struct TelemetryQuery {
    since: Option<u32>,
}

/// GET /api/telemetry -- return recent telemetry events as JSON array.
async fn get_telemetry(
    State(state): State<SharedState>,
    Query(query): Query<TelemetryQuery>,
) -> Json<serde_json::Value> {
    let s = state.lock().unwrap_or_else(|e| e.into_inner());
    let since = query.since.unwrap_or(0);
    let entries: Vec<&TelemetryEntry> = s
        .telemetry_log
        .iter()
        .filter(|e| e.seq >= since)
        .collect();
    Json(serde_json::json!(entries))
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p native-server`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add hil/native-server/src/lib.rs
git commit -m "feat: add GET /api/telemetry endpoint with ?since= filter"
```

---

### Task 7: Build and integration verification

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: Zero warnings

- [ ] **Step 3: Build frontend**

Run: `cd www && npm run build`
Expected: Clean build

- [ ] **Step 4: Run frontend tests**

Run: `cd www && npx vitest run`
Expected: All tests PASS (including new telemetry tests)

- [ ] **Step 5: Verify end-to-end manually**

1. Start server: `cargo run` (native-server)
2. Open `http://localhost:3000`, switch to Dataflow mode
3. Connect to HIL (ws://localhost:8080)
4. Add a block via the sidebar palette
5. Check `http://localhost:8080/api/telemetry` — should contain the block-added event

- [ ] **Step 6: Commit any fixes, then final commit**

```bash
git add -A
git commit -m "feat: pubsub telemetry — frontend CRUD events over WebSocket"
```
