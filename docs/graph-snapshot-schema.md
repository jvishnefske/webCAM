# GraphSnapshot IR Schema

The `GraphSnapshot` JSON is the universal contract between all frontends (block editor, Python API, Jupyter notebook) and all codegen targets. Every frontend serializes to this format; every backend consumes it.

## Schema Definition

```jsonc
{
  // Schema version — bumped on breaking changes
  "version": "1.0",

  // Ordered list of blocks in the graph
  "blocks": [
    {
      "id": 1,                          // u32, unique within this graph
      "block_type": "gain",             // factory key (see Block Types below)
      "name": "Gain",                   // human-readable display name
      "inputs": [
        { "name": "in", "kind": "Float" }
      ],
      "outputs": [
        { "name": "out", "kind": "Float" }
      ],
      "config": { "factor": 2.0 },     // block-specific JSON config
      "output_values": [                // last tick's output per port (nullable)
        { "type": "Float", "data": 10.0 }
      ]
    }
  ],

  // Directed connections between ports
  "channels": [
    {
      "id": { "0": 1 },                // ChannelId (newtype wrapper)
      "from_block": { "0": 1 },        // BlockId of source
      "from_port": 0,                   // output port index on source
      "to_block": { "0": 2 },          // BlockId of destination
      "to_port": 0                      // input port index on destination
    }
  ],

  "tick_count": 100,                    // number of ticks executed so far
  "time": 1.0,                         // accumulated simulation time (seconds)

  // Optional: UI layout data (not used by codegen)
  "layout": {
    "1": { "x": 100, "y": 200 },
    "2": { "x": 300, "y": 200 }
  }
}
```

## Field Reference

### Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | `string` | Yes | Schema version. Currently `"1.0"`. |
| `blocks` | `BlockSnapshot[]` | Yes | All blocks in the graph, sorted by id. |
| `channels` | `Channel[]` | Yes | All connections between ports. |
| `tick_count` | `u64` | Yes | Number of simulation ticks executed. |
| `time` | `f64` | Yes | Accumulated simulation time in seconds. |
| `layout` | `Map<string, {x, y}>` | No | Block positions for visual editors. Keys are block id strings. |

### BlockSnapshot

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u32` | Unique block identifier within this graph. |
| `block_type` | `string` | Factory key used by `create_block()`. |
| `name` | `string` | Human-readable name (e.g. "Constant", "Gain"). |
| `inputs` | `PortDef[]` | Input port definitions, ordered by index. |
| `outputs` | `PortDef[]` | Output port definitions, ordered by index. |
| `config` | `object` | Block-specific configuration (varies by block_type). |
| `output_values` | `(Value \| null)[]` | Last output per port. Null if port hasn't produced a value. |

### PortDef

| Field | Type | Values |
|-------|------|--------|
| `name` | `string` | Port display name (e.g. `"in"`, `"out"`, `"channel"`) |
| `kind` | `string` | `"Float"`, `"Bytes"`, `"Text"`, `"Series"`, `"Any"` |

### Value (tagged union)

```jsonc
{ "type": "Float",  "data": 3.14 }
{ "type": "Bytes",  "data": [0, 1, 255] }
{ "type": "Text",   "data": "hello" }
{ "type": "Series", "data": [1.0, 2.0, 3.0] }
```

### Channel

| Field | Type | Description |
|-------|------|-------------|
| `id` | `{ "0": u32 }` | Newtype wrapper around channel id. |
| `from_block` | `{ "0": u32 }` | Source block id (newtype wrapper). |
| `from_port` | `usize` | Output port index on the source block. |
| `to_block` | `{ "0": u32 }` | Destination block id (newtype wrapper). |
| `to_port` | `usize` | Input port index on the destination block. |

## Block Types

All block types available in the factory (`src/dataflow/blocks/mod.rs`):

| `block_type` | Name | Category | Inputs | Outputs | Config |
|-------------|------|----------|--------|---------|--------|
| `constant` | Constant | Sources | — | `out: Float` | `{ "value": f64 }` |
| `gain` | Gain | Math | `in: Float` | `out: Float` | `{ "factor": f64 }` |
| `add` | Add | Math | `a: Float`, `b: Float` | `out: Float` | — |
| `multiply` | Multiply | Math | `a: Float`, `b: Float` | `out: Float` | — |
| `clamp` | Clamp | Math | `in: Float` | `out: Float` | `{ "min": f64, "max": f64 }` |
| `plot` | Plot | Sinks | `in: Float` | — | `{ "buffer_size": usize }` |
| `json_encode` | JSON Encode | Serde | `in: Any` | `out: Text` | — |
| `json_decode` | JSON Decode | Serde | `in: Text` | `out: Any` | — |
| `udp_source` | UDP Source | I/O | — | `out: Bytes` | `{ "address": string }` |
| `udp_sink` | UDP Sink | I/O | `in: Bytes` | — | `{ "address": string }` |
| `adc_source` | ADC Source | Embedded | — | `out: Float` | `{ "channel": u32 }` |
| `pwm_sink` | PWM Sink | Embedded | `duty: Float` | — | `{ "channel": u32 }` |
| `gpio_out` | GPIO Out | Embedded | `value: Float` | — | `{ "pin": u32 }` |
| `gpio_in` | GPIO In | Embedded | — | `value: Float` | `{ "pin": u32 }` |
| `uart_tx` | UART TX | Embedded | `data: Bytes` | — | `{ "baud": u32 }` |
| `uart_rx` | UART RX | Embedded | — | `data: Bytes` | `{ "baud": u32 }` |
| `state_machine` | State Machine | Logic | `input: Float` | `state: Float` | `{ "states": [...], "transitions": [...] }` |

## Rust Source of Truth

The canonical types are defined in Rust:

- `GraphSnapshot`, `BlockSnapshot` — `src/dataflow/graph.rs`
- `Block` trait, `Value`, `PortDef`, `PortKind` — `src/dataflow/block.rs`
- `Channel`, `ChannelId` — `src/dataflow/channel.rs`
- Block factory — `src/dataflow/blocks/mod.rs`

The TypeScript mirror lives at `www/src/dataflow/types.ts`.

## Versioning Strategy

- The `version` field follows semver: `MAJOR.MINOR`.
- **Major bump**: removing fields, changing field types, changing the meaning of existing fields.
- **Minor bump**: adding new optional fields, adding new block types.
- Frontends should check `version` on load and warn on major mismatch.
- Codegen targets should reject snapshots with unknown major versions.

## Constraints

- Each input port may have at most one incoming channel (enforced by `DataflowGraph::connect`).
- Output ports may fan out to multiple channels.
- Block ids are unique within a graph but not globally stable across save/load cycles.
- `output_values` is populated only after at least one tick; a freshly-added block has all nulls.
- `layout` is frontend-only metadata — codegen ignores it entirely.
