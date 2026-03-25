# LocalStorage Save/Load for Block Editor

## Overview

Persist dataflow projects to browser localStorage with named projects, auto-save, and a sidebar project manager.

## Save Format

```typescript
interface SavedProject {
  name: string;
  lastModified: string; // ISO 8601
  graph: {
    blocks: Array<{ id: number; blockType: string; config: Record<string, unknown> }>;
    channels: Array<{ fromBlock: number; fromPort: number; toBlock: number; toPort: number }>;
  };
  positions: Record<number, { x: number; y: number }>;
  viewport: { panX: number; panY: number; scale: number };
}
```

### Storage Keys

- `webcam:projects` — JSON array of project names
- `webcam:project:<name>` — JSON `SavedProject`
- `webcam:active` — name of last active project

## Restore Flow

1. Parse saved project JSON from localStorage
2. Stop simulation if running
3. Destroy current WASM graph, create new one via `DataflowManager`
4. Replay `addBlock()` for each saved block, building `oldId -> newId` mapping
5. Replay `connect()` for each saved channel using mapped IDs
6. Set `mgr.positions` from saved positions (using new IDs)
7. Set viewport (panX, panY, scale) from saved data
8. Reconcile UI

## Auto-Save

- Debounced: 1 second after last mutation
- Triggers on: add/remove block, connect/disconnect, node move, config change
- Saves to active project name
- On page load: restore last active project (from `webcam:active`)

## UI

### Toolbar Buttons

Three buttons added to the editor toolbar:
- **New** — auto-saves current project, creates new "Untitled" project
- **Save As** — inline text input prompt for project name
- **Projects** — toggles sidebar panel visibility

Active project name displayed in toolbar.

### Sidebar Panel (Right Side)

- Slides in/out from right edge
- Lists saved projects, each row showing:
  - Project name
  - Last modified date (relative, e.g. "2 min ago")
  - Load button
  - Delete button (with confirmation)
- Active project highlighted

### Behavior

- **New**: auto-saves current, creates empty graph named "Untitled"
- **Save As**: saves current graph under new name, switches active to it
- **Load** (sidebar): auto-saves current, loads selected project
- **Delete** (sidebar): if deleting active project, switch to new empty "Untitled"
- **Page load**: restore `webcam:active` project; if none, start with empty "Untitled"

## Edge Cases

- Duplicate names on Save As: append " (2)", " (3)", etc.
- localStorage full: `console.warn`, don't crash the editor
- ID remapping on load: WASM assigns new block IDs; channel endpoints mapped through `oldId -> newId` table
- First visit: empty "Untitled" project, sidebar hidden
- Corrupted data: `console.warn`, fall back to empty project

## Files to Create/Modify

- `www/src/dataflow/storage.ts` — new: serialization, localStorage read/write, auto-save debounce
- `www/src/dataflow/sidebar.ts` — new: sidebar panel DOM, project list, load/delete actions
- `www/src/dataflow/editor.ts` — modify: add toolbar buttons, wire up storage + sidebar, restore on init
- `www/src/dataflow/graph.ts` — modify: add `loadProject()` method (destroy + replay)
