# DAG Frontend Enhancements — Design Spec

**Date**: 2026-03-26
**Branch**: `feature-embedded-targets`
**Status**: Approved

## Overview

Add live value monitoring, hardware channel browsing, undo/redo, and auto-save to the Pico 2-served DAG editor. Live values flow through the existing pubsub system with auto-generated debug channels. Channel names for Input nodes come from a dynamic runtime registry on the MCU.

## Features

### 1. Live Values via PubSub Debug Channels

**MCU side (`dag_handler.rs`):**
- Add `debug_mode: bool` field to `DagApiHandler` (default: false)
- `POST /api/debug` — toggles debug mode, returns `{"debug":true/false}`
- When `debug_mode` is true, after each `POST /api/tick` evaluation, publish every node's computed value to `_dbg/<index>` topics in `SimplePubSub`
- Add `SimplePubSub` field to `DagApiHandler` (or accept as parameter)
- `GET /api/pubsub` — returns all pubsub topics as JSON object: `{"_dbg/0":-4.0,"_dbg/1":2.0,"user/topic":3.14}`

**Frontend side (`dag-editor.ts`):**
- When debug mode is active, poll `GET /api/pubsub` every 500ms
- Parse `_dbg/N` topics: extract N as the DAG node index, map back to visual node ID via the `nodeMap` from last build
- Update `node.result` and re-render to show live values on each SVG node
- Non-`_dbg` topics displayed in the hardware panel
- Add toggle button in toolbar: "Debug" (toggles `POST /api/debug`)
- Auto-tick button: triggers `POST /api/tick` at regular intervals alongside polling

### 2. Hardware Panel + Dynamic Channel Registry

**MCU side (`dag_handler.rs`):**
- Add `known_inputs: heapless::Vec<heapless::String<32>, 16>` to `DagApiHandler`
- Add `known_outputs: heapless::Vec<heapless::String<32>, 16>` to `DagApiHandler`
- `pub fn register_input(&mut self, name: &str)` — called by HAL drivers at init
- `pub fn register_output(&mut self, name: &str)` — called by HAL drivers at init
- `GET /api/channels` — returns `{"inputs":["adc0","adc1"],"outputs":["pwm0","pwm1"]}`
- For now, Pico 2 main.rs registers some default channels: `adc0-adc3`, `gpio0`, `gpio1`, `pwm0`, `pwm1`

**Frontend side:**
- Collapsible hardware panel between toolbar and workspace (or at bottom of inspector)
- Fetches `/api/channels` on init and caches the list
- Shows channel names grouped by input/output
- For channels that also appear in pubsub, shows live values next to them

### 3. Channel Name UX in Inspector

**Input / Subscribe nodes:**
- Inspector shows a `<select>` dropdown populated from cached `/api/channels` inputs list
- First option is empty (custom), followed by known channel names
- User can also type a custom name in a text input alongside the select
- Selecting from dropdown sets `node.name` and re-renders

**Output / Publish nodes:**
- When created, `node.name` auto-populates to `out_<id>` or `pub_<id>`
- Inspector shows an editable text input pre-filled with the auto-generated name
- User can change it freely

### 4. State Stack + Auto-Save to LocalStorage

**Undo/Redo stack:**
- Module-level `undoStack: string[]` (JSON-serialized DagState snapshots)
- Module-level `redoStack: string[]`
- Max 50 entries in undoStack
- `pushUndo()` captures current state as JSON, pushes to undoStack, clears redoStack
- Called before every mutation: addNode, removeNode, connect, move (on mouseup), config change
- `undo()`: pop undoStack, push current to redoStack, restore popped state
- `redo()`: pop redoStack, push current to undoStack, restore popped state
- Keyboard: Ctrl+Z = undo, Ctrl+Shift+Z = redo

**Auto-save:**
- `autoSave()` debounced 1000ms after last mutation
- Writes to `localStorage["dag:state"]` — JSON of `{ nodes, nextId, panX, panY, scale }`
- `loadState()` on init — reads from localStorage, restores state if present
- Clear button also removes `localStorage["dag:state"]`

## MCU API Changes Summary

| Endpoint | Method | Description |
|----------|--------|-------------|
| `POST /api/debug` | POST | Toggle debug mode on/off |
| `GET /api/pubsub` | GET | All pubsub topics as JSON object |
| `GET /api/channels` | GET | Known input/output channel names |

Existing endpoints unchanged:
- `POST /api/dag` — load CBOR DAG
- `GET /api/status` — executor status
- `POST /api/tick` — evaluate one tick

## Frontend Changes Summary

| File | Changes |
|------|---------|
| `dag-types.ts` | Add `SavedState` interface |
| `dag-editor.ts` | Undo/redo stack, auto-save/load, poll loop, debug toggle, hardware panel, channel select in inspector, auto-name for output/publish |
| `index.html` | Add debug toggle button, hardware panel div, auto-tick button |

## Data Flow

```
Browser                          MCU (Pico 2)
  |                                |
  |-- POST /api/dag (CBOR) ------->|  Load DAG
  |-- POST /api/debug ------------>|  Enable debug mode
  |                                |
  |-- POST /api/tick -------------->|  Evaluate + publish _dbg/* topics
  |<- GET /api/pubsub -------------|  {"_dbg/0":-4.0, "user/x":1.0}
  |                                |
  |   [update node.result]         |
  |   [render live values]         |
  |                                |
  |<- GET /api/channels -----------|  {"inputs":["adc0"],"outputs":["pwm0"]}
  |   [populate <select>]          |
```

## Constraints

- All pubsub data fits in a single HTTP response (heapless::Vec<u8, 512>)
- Max 128 nodes = max 128 `_dbg/*` topics
- Channel registry max 16 inputs + 16 outputs (heapless)
- Frontend polling at 500ms — acceptable for CDC ECM latency (~1ms RTT)
- localStorage auto-save is fire-and-forget (no error handling needed)
