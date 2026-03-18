# Nodi-Style DOM/SVG Node Editor for Dataflow Simulator

## Problem

The current dataflow editor (`www/src/dataflow/editor.ts`) renders everything on a single HTML Canvas. This prevents pan/zoom, makes hit-testing manual, can't leverage native DOM events for ports, and scales poorly as graphs grow. The nodi3d/nodi project demonstrates a better approach: DOM elements for nodes, SVG paths for connections, CSS transforms for pan/zoom.

## Goal

Replace the canvas-based editor with a DOM/SVG hybrid that follows nodi's rendering architecture while keeping the existing Rust WASM backend and `DataflowManager` completely unchanged.

## What Changes

| Layer | Current (canvas) | New (DOM/SVG) |
|-------|-----------------|---------------|
| Nodes | Canvas roundRect + fillText | `<div>` elements with CSS |
| Connections | Canvas bezierCurveTo | SVG `<path>` cubic Beziers |
| Ports | Canvas arc + manual hit-test | `<div>` elements with native events |
| Pan/zoom | None | CSS `transform: translate() scale()` on container |
| Grid | None | CSS background-image repeating pattern |
| Selection | Manual coordinate check | DOM class toggle + native events |
| Palette | DOM overlay hack | Searchable DOM dropdown |
| Block info | External sidebar | Keep sidebar (unchanged) |

## What Does NOT Change

- `src/dataflow/` (Rust backend) — untouched
- `www/src/dataflow/graph.ts` (`DataflowManager`) — untouched
- `www/src/dataflow/types.ts` — untouched
- `www/src/dataflow/plot.ts` — untouched
- `www/src/dataflow/index.ts` — minor: swap `DataflowEditor` constructor (canvas → container div)
- WASM API surface (`dataflow_*` exports) — untouched
- Block types, scheduler, channels — untouched

## Architecture

### Coordinate Systems

Two coordinate spaces are used throughout:

- **Screen space**: pixel coordinates relative to the viewport (`clientX`, `clientY`)
- **World space**: logical coordinates in the graph, independent of pan/zoom

Conversion formulas:

```
worldX = (screenX - rect.left - panX) / scale
worldY = (screenY - rect.top - panY) / scale

screenX = worldX * scale + panX + rect.left
screenY = worldY * scale + panY + rect.top
```

All node positions and SVG path coordinates are in world space. The CSS transform `translate(panX, panY) scale(scale)` on the node-layer and edge-layer converts world to screen automatically. The palette is positioned in screen space (readable at any zoom level) but creates blocks in world space.

### Container Structure

```
div.df-workspace                    ← receives wheel (zoom) and middle-mouse (pan)
├── svg.df-edge-layer               ← BEHIND nodes (renders first, lower z-index)
│   │                                  pointer-events: none on <svg>, auto on <path> if needed
│   ├── path.df-edge[data-ch="0"]  ← one per channel
│   └── path.df-edge.dragging      ← wire being dragged (dashed)
├── div.df-node-layer               ← CSS transform: translate(panX, panY) scale(zoom)
│   ├── div.df-node[data-id="1"]   ← one per block
│   │   ├── div.df-node-header     ← title + type subtitle
│   │   ├── div.df-port.input[0]   ← input port circle + label
│   │   ├── div.df-port.output[0]  ← output port circle + value label
│   │   └── ...
│   └── div.df-node[data-id="2"]
│       └── ...
├── div.df-grid                     ← CSS background grid (lowest z-index via z-index: -1)
└── div.df-palette                  ← positioned in screen space at click point, searchable
```

The SVG edge layer is a sibling **before** the node layer in DOM order, so nodes paint on top of edges. The SVG element has `pointer-events: none` to avoid intercepting clicks meant for nodes/ports. Both the node layer and SVG edge layer share the same CSS transform so nodes and wires stay aligned during pan/zoom.

### File Structure

Replace `editor.ts` (430 lines, single file) with:

| File | Responsibility | Approx size |
|------|---------------|-------------|
| `editor.ts` | Workspace: container setup, pan/zoom, grid, orchestration, `onTick` wiring, `destroy()` | ~180 lines |
| `node-view.ts` | Create/update/remove node DOM elements, drag handling, right-click delete | ~150 lines |
| `edge-view.ts` | Create/update/remove SVG path elements, Bezier math | ~80 lines |
| `port-view.ts` | Port circles, drag-to-connect interaction, screen↔world conversion for wire drag | ~120 lines |
| `palette.ts` | Searchable block type picker with default configs per block type | ~100 lines |

### Orchestration (`editor.ts`)

The `DataflowEditor` class owns the workspace and coordinates sub-modules:

- Constructor: creates container DOM structure, sets up pan/zoom listeners on `.df-workspace`
- Sets `mgr.onTick` callback which triggers reconciliation
- `reconcile(snap)`: delegates to `node-view` (add/remove/update nodes) and `edge-view` (update paths)
- `destroy()`: removes all DOM elements, calls `removeEventListener` on workspace, clears `mgr.onTick`

### Pan/Zoom

Following nodi's approach:
- **Pan**: Middle-mouse or Shift+left drag updates `panX`, `panY`
- **Zoom**: Wheel event adjusts `scale` (clamped 0.2–3.0), focal point preserved under cursor
- Applied via single CSS transform on node-layer and edge-layer: `translate(${panX}px, ${panY}px) scale(${scale})`
- Grid background adjusts `background-position` and `background-size` to match

### Node Rendering (`node-view.ts`)

Each block becomes a `<div class="df-node">` with:
- Position via CSS `transform: translate(x, y)` (world coordinates, container transform handles pan/zoom)
- Selection state via `.selected` class (border highlight)
- Title: block name in `<div class="df-node-header">`
- Subtitle: block_type in `<span class="df-node-type">`
- Ports rendered as child `<div class="df-port">` elements

Node dimensions: 140px wide, height = 40 + max(inputs, outputs) * 20px (same as current).

**Drag**: `mousedown` on node header starts drag. `mousemove` updates `mgr.positions` and applies CSS transform. `mouseup` ends drag and triggers edge update for connected wires.

**Delete**: `contextmenu` on a node calls `mgr.removeBlock(nodeId)` directly (no menu, same as current behavior). Fires reconciliation to remove DOM element and connected edges.

### Node Positions

Node positions are stored in `mgr.positions` (`Map<number, NodePosition>` on DataflowManager — unchanged). On drag, `node-view.ts` writes to `mgr.positions` and applies the CSS transform. On reconciliation, new nodes read initial position from `mgr.positions` (set by `mgr.addBlock()`).

### Port Rendering (`port-view.ts`)

Each port is a `<div class="df-port input|output">`:
- Colored circle via CSS `background-color` based on port kind (Float=#4f8cff, Bytes=#ff9800, Text=#55ff88, Series=#ff55aa)
- Input ports: left side, label to the right
- Output ports: right side, value label to the left
- `mousedown` on port starts wire drag
- `mouseup` on port completes connection

**Port positions for edges**: Computed from node position + fixed offsets (same math as current canvas code: output port X = node.x + NODE_W, input port X = node.x, port Y = node.y + PORT_OFFSET_Y + portIndex * PORT_SPACING + PORT_SPACING/2). This avoids `getBoundingClientRect()` rounding issues and is the approach nodi uses.

**Wire drag coordinates**: During drag, cursor screen coordinates are converted to world space using the formula in the Coordinate Systems section, then used as the SVG path endpoint.

### Edge Rendering (`edge-view.ts`)

Connections are SVG `<path>` elements in the shared `<svg>` overlay:
- Cubic Bezier: `M x1,y1 C x1+cpX,y1 x2-cpX,y2 x2,y2`
- `cpX = max(0.5 * |x2-x1|, min(|y2-y1|, 50))` (nodi's formula for balanced curves)
- Stroke: 2px, port-kind color
- Drag preview: dashed path from source port to cursor (world space)

**Topology-aware updates**: Edge paths are only rebuilt when the channel list changes (add/remove connection) or when a node is dragged (connected edges recompute endpoints). During normal tick updates (60fps play mode), only output value labels on nodes are updated — edge paths are not touched. This avoids layout thrashing.

### Palette (`palette.ts`)

Double-click on empty space opens a searchable picker:
- Text input at top filters block types by name (stops keyboard event propagation)
- Grouped by category (Sources, Math, Sinks, I/O, Serde)
- Positioned in screen space (readable at any zoom)
- Click creates block at the world-space coordinates of the double-click
- Escape or click-outside dismisses

**Default configs per block type** (carried from current code):
```
constant:    { value: 1.0 }
gain:        { op: 'Gain', param1: 1.0, param2: 0.0 }
clamp:       { op: 'Clamp', param1: 0.0, param2: 100.0 }
plot:        { max_samples: 500 }
udp_source:  { address: '127.0.0.1:9000' }
udp_sink:    { address: '127.0.0.1:9001' }
```

### Snapshot Data Shapes

The `ChannelSnapshot` type from `types.ts` wraps IDs in newtype objects:

```typescript
interface ChannelSnapshot {
  id: { 0: number };
  from_block: { 0: number };  // access as ch.from_block[0]
  from_port: number;
  to_block: { 0: number };    // access as ch.to_block[0]
  to_port: number;
}
```

All edge rendering code must use `ch.from_block[0]` and `ch.to_block[0]` to extract the numeric block ID.

### CSS

All styles in a `<style>` block within `www/index.html` (following existing pattern). Dark theme matching current colors:

```css
.df-workspace { position: relative; overflow: hidden; background: #0f1117; }
.df-grid { position: absolute; inset: 0; z-index: 0; pointer-events: none; }
.df-edge-layer { position: absolute; inset: 0; z-index: 1; pointer-events: none; overflow: visible; }
.df-node-layer { position: absolute; inset: 0; z-index: 2; }
.df-node { position: absolute; width: 140px; background: #1a1d27; border: 1px solid #2a2d3a; border-radius: 6px; cursor: grab; user-select: none; }
.df-node.selected { border-color: #4f8cff; border-width: 2px; }
.df-port { position: absolute; width: 12px; height: 12px; border-radius: 50%; cursor: crosshair; }
.df-palette { position: fixed; z-index: 100; }
```

## Data Flow

```
User interaction (DOM events on nodes/ports/workspace)
    ↓
editor.ts (pan/zoom transforms, delegates to sub-modules)
    ↓
node-view.ts (drag) / port-view.ts (wire drag) / palette.ts (add block)
    ↓
DataflowManager (graph.ts) — unchanged
    ↓
WASM dataflow_* calls — unchanged
    ↓
GraphSnapshot JSON returned
    ↓
editor.ts reconcile():
  ├── node-view: add/remove/update node divs + output labels
  └── edge-view: update SVG paths only if topology changed
    ↓
index.ts updates sidebar info + plots — unchanged
```

### Reconciliation

On each snapshot update (from `mgr.onTick` or manual refresh):

1. Compare `snap.blocks` IDs against existing `.df-node` elements by `data-id`
2. Add DOM nodes for new blocks (read position from `mgr.positions`), remove DOM for deleted blocks
3. Update output value labels on existing nodes (fast — just `textContent` changes)
4. Compare `snap.channels` IDs against current edge set. Only rebuild SVG paths if channels added/removed.
5. During node drag: update edge endpoints for connected channels only.

This is a lightweight reconcile, not a virtual DOM. Block count is typically <50.

## Interface Contract

The new `DataflowEditor` keeps the same public API with one addition:

```typescript
class DataflowEditor {
  constructor(container: HTMLDivElement, mgr: DataflowManager);
  resize(): void;
  updateSnapshot(): void;
  destroy(): void;  // NEW: cleanup listeners and DOM
  onSelect: ((blockId: number | null, snap: GraphSnapshot | null) => void) | null;
}
```

Constructor takes a `<div>` container instead of `<canvas>`. The `index.ts` update: swap `$canvas('dataflow-canvas')` for `$('dataflow-canvas')` and change the HTML element from `<canvas>` to `<div>`. On reset, call `editor.destroy()` before creating a new editor.

## Testing

- All existing `cargo test` (142 tests) must still pass — backend untouched
- Manual validation:
  - Double-click → palette appears → type to filter → select block → node appears
  - Drag output port to input port → wire connects → snapshot confirms channel
  - Right-click node → node deleted → connected wires removed
  - Middle-mouse drag → pan works, nodes and wires move together
  - Scroll wheel → zoom works, focal point preserved under cursor
  - Play/Pause → output values update in real time on node labels
  - Batch run → plot updates
  - Reset → old editor cleaned up, new editor works

## Non-Goals

- Node grouping (nodi feature, not needed for MVP)
- Undo/redo
- Graph serialization to file (future)
- Custom node shapes or icons
- Minimap
- Click-to-select edges (edges have `pointer-events: none`)
