# PubSub Telemetry: Frontend → Server

## Context

The web frontend is the dataflow graph editor (static, CDN-served). An optional server can connect via WebSocket and passively monitor graph mutations. This enables diagnostics, auditing, and integration tooling without the server being in the editing loop.

## Requirements

- Frontend publishes CRUD events (block add/remove/update, connection create/delete, graph reset) over WebSocket
- Server subscribes and receives events as a passive monitor
- Debug mode (disabled by default) wraps events with sequence number + timestamp for CRUD event logging
- Uses existing WebSocket + CBOR protocol (hil-client.ts ↔ native-server)
- Zero overhead when no server is connected or telemetry is disabled

## Wire Format

CBOR-encoded binary WebSocket frames. Tags 50–59 reserved for telemetry (outside existing 1–43 I2C/DAG range).

### Event Messages

| Tag | Event | CBOR Map Keys |
|-----|-------|---------------|
| 50 | Block added | `{0:50, 1:blockId, 2:"blockType", 3:config, 4:x, 5:y}` |
| 51 | Block removed | `{0:51, 1:blockId}` |
| 52 | Block updated | `{0:52, 1:blockId, 2:"blockType", 3:newConfig}` |
| 53 | Connection created | `{0:53, 1:fromBlock, 2:fromPort, 3:toBlock, 4:toPort, 5:channelId}` |
| 54 | Connection removed | `{0:54, 1:channelId}` |
| 55 | Graph reset | `{0:55}` |

### Debug Wrapper (when debug enabled)

Tag 56 wraps any event 50–55 with ordering metadata:

```
{0:56, 1:sequenceNumber, 2:timestampMs, 3:{...innerEvent...}}
```

- `sequenceNumber`: monotonic u32, resets on page reload
- `timestampMs`: `performance.now()` milliseconds (float)
- Key 3 contains the full inner event map

When debug is off, events 50–55 are sent unwrapped.

## Components

### Frontend: `www/src/dataflow/telemetry.ts` (new)

```typescript
export class TelemetryPublisher {
  private ws: WebSocket | null = null;
  private enabled = false;
  private debug = false;
  private seq = 0;

  /** Bind to an open WebSocket connection. */
  attach(ws: WebSocket): void;

  /** Detach (connection closed or telemetry disabled). */
  detach(): void;

  /** Enable/disable telemetry publishing. */
  setEnabled(on: boolean): void;

  /** Toggle debug CRUD log (adds seq + timestamp wrapper). */
  setDebug(on: boolean): void;

  /** Publish a CRUD event. No-op if disabled or no WebSocket. */
  publish(event: TelemetryEvent): void;
}

type TelemetryEvent =
  | { tag: 50; blockId: number; blockType: string; config: unknown; x: number; y: number }
  | { tag: 51; blockId: number }
  | { tag: 52; blockId: number; blockType: string; config: unknown }
  | { tag: 53; fromBlock: number; fromPort: number; toBlock: number; toPort: number; channelId: number }
  | { tag: 54; channelId: number }
  | { tag: 55 };
```

Encoding uses the existing `cbor-x` dependency (already in package.json for hil-client).

### Frontend: `www/src/dataflow/graph.ts` (modify)

Wire `TelemetryPublisher.publish()` calls into existing mutation methods:

- `addBlock()` → after WASM call succeeds, publish tag 50
- `removeBlock()` → publish tag 51
- `updateBlock()` → publish tag 52
- `connect()` → publish tag 53
- `disconnect()` → publish tag 54
- `restoreProject()` and reset paths → publish tag 55

The publisher is injected via a new `telemetry` property on `DataflowManager`. Default: null (no telemetry). Set when HilClient connects.

### Frontend: `www/src/dataflow/index.ts` (modify)

- Create `TelemetryPublisher` instance during `initDataflow()`
- Attach to HilClient's WebSocket on connect, detach on disconnect
- Wire debug toggle to existing `POST /api/debug` button or a new sidebar toggle

### Server: `hil/native-server/src/lib.rs` (modify)

Add telemetry event handling to the WebSocket dispatch:

- Tags 50–56: log via `tracing::info!` with structured fields
- Store last 256 events in a `VecDeque<TelemetryEntry>` ring buffer on `ServerState`
- New REST endpoint `GET /api/telemetry` returns the ring buffer as JSON array

```rust
struct TelemetryEntry {
    seq: u32,
    timestamp_ms: f64,
    tag: u8,
    payload: serde_json::Value,
}
```

The WebSocket handler decodes CBOR tags 50–56, converts to `TelemetryEntry`, and pushes to the ring buffer. If the event is already debug-wrapped (tag 56), extracts seq/timestamp from the wrapper. If unwrapped, assigns server-side seq/timestamp.

### Server: `GET /api/telemetry` (new endpoint)

Returns the event ring buffer:

```json
[
  {"seq": 1, "timestamp_ms": 1042.5, "tag": 50, "payload": {"blockId": 3, "blockType": "constant", "config": {"value": 1.0}, "x": 200, "y": 150}},
  {"seq": 2, "timestamp_ms": 1105.2, "tag": 53, "payload": {"fromBlock": 3, "fromPort": 0, "toBlock": 4, "toPort": 0, "channelId": 1}}
]
```

Query parameter `?since=<seq>` returns only events after the given sequence number (for incremental polling by secondary consumers).

## Data Flow

```
Browser (editor)                    Server (monitor)
─────────────────                   ────────────────
User edits graph
       │
DataflowManager.addBlock()
       │
TelemetryPublisher.publish({tag:50, ...})
       │
   [CBOR encode]
       │
   WebSocket ──────────────────────► WS handler
                                         │
                                    decode CBOR tag 50
                                         │
                                    tracing::info!(...)
                                         │
                                    ring_buffer.push(entry)
                                         │
                                    GET /api/telemetry ──► JSON
```

## Debug Mode

- Disabled by default (zero overhead: `publish()` returns immediately if `!this.enabled`)
- Toggled via `TelemetryPublisher.setDebug(true)`
- When enabled, events are wrapped in tag 56 with monotonic sequence + `performance.now()` timestamp
- Server logs debug events with `tracing::debug!` level (vs `tracing::info!` for unwrapped)

## Testing

- **Unit**: `TelemetryPublisher` encodes correct CBOR for each event type (vitest, mock WebSocket)
- **Unit**: Server parses tags 50–56 and populates ring buffer (Rust test with mock CBOR input)
- **Integration**: Frontend addBlock → server ring buffer contains matching entry (native-server + browser)
- **Debug toggle**: Enable debug → events arrive with seq/timestamp wrapper → disable → events arrive unwrapped

## Files to Create/Modify

| File | Action |
|------|--------|
| `www/src/dataflow/telemetry.ts` | Create — TelemetryPublisher class |
| `www/src/dataflow/graph.ts` | Modify — wire publish calls into mutations |
| `www/src/dataflow/index.ts` | Modify — instantiate publisher, attach to HilClient |
| `hil/native-server/src/lib.rs` | Modify — handle tags 50–56, ring buffer, `/api/telemetry` |
| `www/src/dataflow/hil-client.ts` | Modify — expose raw WebSocket for telemetry attach |
