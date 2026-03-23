# Frontend 1: Block Editor (DOM/SVG Visual Editor)

**URL:** `/cam/blocks/`

## Status

~70% built on the `feature-dataflow` branch. The core graph engine, block palette, node rendering, edge routing, and WASM integration are functional. Remaining work is UI polish and workflow features.

## Target Audience

- Hardware engineers designing control systems visually
- Educators teaching signal processing and embedded concepts
- Visual thinkers who prefer drag-and-drop over code

## Architecture

The block editor is a DOM/SVG hybrid that renders dataflow graphs as draggable nodes with SVG edge connections. It runs entirely in the browser with no server — the Rust dataflow engine compiles to WASM.

```
┌─────────────────────────────────────────────┐
│  Browser                                     │
│                                              │
│  ┌──────────────┐    ┌───────────────────┐  │
│  │ DataflowEditor│───▶│ DataflowManager   │  │
│  │ (orchestrator)│◀───│ (WASM wrapper)    │  │
│  └──────┬───────┘    └────────┬──────────┘  │
│         │                     │              │
│    ┌────┴────┐          ┌─────┴─────┐       │
│    │ DOM     │          │ Rust/WASM │       │
│    │ Nodes   │          │ DataflowGraph│    │
│    │ + SVG   │          │ + Scheduler │    │
│    │ Edges   │          │ + Codegen   │    │
│    └─────────┘          └───────────┘       │
└─────────────────────────────────────────────┘
```

### Layer Breakdown

| Layer | File | Responsibility |
|-------|------|----------------|
| Orchestrator | `www/src/dataflow/editor.ts` | Owns workspace DOM, coordinates all subsystems, handles pan/zoom |
| Node rendering | `www/src/dataflow/node-view.ts` | Creates/updates/removes DOM nodes, manages selection state |
| Edge rendering | `www/src/dataflow/edge-view.ts` | SVG path generation for connections, hit-testing |
| Port interaction | `www/src/dataflow/port-view.ts` | Wire drag from output port to input port |
| Block palette | `www/src/dataflow/palette.ts` | Double-click popup listing available block types by category |
| WASM wrapper | `www/src/dataflow/graph.ts` | `DataflowManager` class wrapping `wasm-bindgen` exports |
| Type definitions | `www/src/dataflow/types.ts` | TypeScript mirror of `GraphSnapshot` and related Rust types |
| Signal plotting | `www/src/dataflow/plot.ts` | Real-time sparkline rendering for Plot blocks |

### WASM Integration

`DataflowManager` wraps the `wasm-bindgen` exports from `src/lib.rs`:

```
dataflow_new(dt)           → graph_id
dataflow_add_block(id, type, config) → block_id
dataflow_remove_block(id, block_id)
dataflow_update_block(id, block_id, type, config)
dataflow_connect(id, from, from_port, to, to_port) → channel_id
dataflow_disconnect(id, channel_id)
dataflow_advance(id, elapsed) → GraphSnapshot JSON
dataflow_run(id, steps, dt)  → GraphSnapshot JSON
dataflow_snapshot(id)        → GraphSnapshot JSON
dataflow_codegen(id, dt)     → generated files JSON
dataflow_codegen_multi(id, dt, targets) → generated workspace JSON
dataflow_block_types()       → BlockTypeInfo[] JSON
```

All data crosses the WASM boundary as JSON strings, parsed on the TypeScript side into typed interfaces from `types.ts`.

### Rendering Model

The editor uses a reconciliation pattern (similar to React's virtual DOM):

1. Any mutation (add block, connect, tick) goes through `DataflowManager`
2. Manager returns an updated `GraphSnapshot`
3. `reconcileNodes()` diffs DOM nodes against `snapshot.blocks`
4. `reconcileEdges()` diffs SVG paths against `snapshot.channels`
5. Only changed elements are created/updated/removed

Node positions are stored client-side in `DataflowManager.positions` (a `Map<number, {x, y}>`), separate from the graph state. The planned `layout` field in `GraphSnapshot` will persist these for save/load.

## What Exists

- Graph engine with 17 block types across 6 categories (Sources, Math, Sinks, Serde, I/O, Embedded, Logic)
- Realtime simulation with configurable tick rate and speed multiplier
- DOM node rendering with input/output port indicators showing type via color
- SVG cubic bezier edge routing between ports
- Pan (middle-click/shift-drag) and zoom (scroll wheel) with focal-point preservation
- Double-click palette popup for adding blocks
- Drag-to-connect wire drawing from output ports to input ports
- Node selection and keyboard delete (Delete/Backspace) for nodes and edges
- Edge selection with click-on-path
- Real-time signal plotting for Plot blocks
- Multi-target codegen (host, RP2040, STM32F4, ESP32-C3)

## What Remains

| Feature | Priority | Description |
|---------|----------|-------------|
| Properties panel | High | Edit block config (gain factor, constant value, etc.) via sidebar form |
| Save/load | High | Serialize `GraphSnapshot` + `layout` to JSON file, load back |
| Codegen UI | High | Button to trigger codegen, display/download generated workspace |
| Undo/redo | Medium | Command stack for add/remove/connect/disconnect/move operations |
| Copy/paste | Medium | Duplicate selected blocks with offset |
| Minimap | Low | Thumbnail overview for large graphs |
| Block groups | Low | Collapse subgraphs into a single "macro" block |
| Touch support | Low | Tablet-friendly pan/zoom/drag |

## Shared IR Contract

The block editor produces and consumes `GraphSnapshot` JSON as defined in [graph-snapshot-schema.md](graph-snapshot-schema.md). Any graph built in the block editor can be exported as JSON and consumed by the Python API or Jupyter notebook frontend.
