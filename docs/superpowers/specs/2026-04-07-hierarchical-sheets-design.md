# Hierarchical Sheet Management

## Overview

Replace the flat project model with a hierarchical one: a **project** contains multiple **sheets**. Every project has a mandatory top-level `"main"` dataflow sheet and can have sub-sheets (additional dataflow sub-graphs, BSP pin configs, etc.). The sidebar lists all projects with expandable sheet trees.

## Data Model

### Project

```typescript
interface Project {
  name: string;
  lastModified: string;         // ISO 8601
  activeSheet: string;          // id of currently displayed sheet
  sheets: Record<string, Sheet>;
}
```

### Sheet

```typescript
interface Sheet {
  id: string;                   // unique within project (e.g. "main", "motor-ctrl-bsp")
  label: string;                // display name
  type: "dataflow" | "bsp";    // determines which editor view renders it
  parentId: string | null;      // null for top-level, sheet id for sub-sheets
  data: DataflowSheetData | BspSheetData;
}

interface DataflowSheetData {
  graph: {
    blocks: SavedBlock[];
    channels: SavedChannel[];
  };
  positions: Record<number, { x: number; y: number }>;
  viewport: { panX: number; panY: number; scale: number };
}

interface BspSheetData {
  mcuFamily: string;            // e.g. "Rp2040"
  pinAssignments: PinAssignment[];
  // detailed shape defined in BSP generator spec
}
```

### Storage Keys (localStorage)

| Key | Value |
|-----|-------|
| `webcam:projects` | `string[]` — project names |
| `webcam:project:{name}` | `Project` JSON |
| `webcam:active` | `string` — active project name |

No change to key naming — the project blob now contains sheets internally.

## Migration

On `loadProject(name)`:
1. Parse JSON from localStorage.
2. If `sheets` field is missing (old format), wrap in new structure:
   - Create `sheets: { main: { id: "main", label: "Main", type: "dataflow", parentId: null, data: { graph, positions, viewport } } }`
   - Set `activeSheet: "main"`
3. Return the `Project` object.

Old projects auto-upgrade on first load. No migration script needed.

## Sidebar UI

### Layout

```
┌─────────────────────────┐
│ Untitled (4)   Projects │
│ [New]          [Save As]│
├─────────────────────────┤
│ ▸ Motor Controller      │  ← project (click to expand)
│ ▾ Sensor Hub            │  ← expanded project
│   ● Main                │  ← active sheet (highlighted)
│     Sub-graph A         │  ← sub-sheet
│     Pico BSP            │  ← BSP sub-sheet
│   [+ Add Sheet]         │  ← add sheet button
│ ▸ Test Project          │
└─────────────────────────┘
```

### Interactions

| Action | Result |
|--------|--------|
| Click project name | Expand/collapse sheet list |
| Click sheet | Switch editor to that sheet |
| "New" button | Create new project with one "main" sheet |
| "Save As" | Clone current project under new name |
| "+ Add Sheet" | Prompt for sheet type (dataflow/bsp) and name |
| Delete (on sheet) | Remove sub-sheet (cannot delete "main") |
| Delete (on project) | Remove entire project and all sheets |

### Active State

- One project is active at a time
- One sheet within the active project is displayed
- Switching sheets swaps the editor view (dataflow editor ↔ BSP editor)
- Auto-save triggers on the active sheet only

## Editor Integration

### Sheet Switching

When the user clicks a sheet:
1. Save current sheet state (graph snapshot + positions + viewport)
2. Set `project.activeSheet = newSheetId`
3. If sheet type is `"dataflow"`: render DataflowEditor with sheet's graph
4. If sheet type is `"bsp"`: render BSP configurator (separate component, defined in BSP spec)
5. Update sidebar highlight

### Auto-Save

- `createAutoSave()` debounce unchanged (1s)
- On trigger: serialize active sheet into `project.sheets[activeSheet].data`
- Save full project blob to `webcam:project:{name}`

## Files to Create/Modify

| File | Change |
|------|--------|
| `www-dataflow/src/dataflow/storage.ts` | Rewrite: `Project`, `Sheet` types, migration, sheet CRUD |
| `www-dataflow/src/dataflow/sidebar.ts` | Rewrite: hierarchical project+sheet list UI |
| `www-dataflow/src/dataflow/index.ts` | Update: sheet switching, auto-save per sheet |
| `www-dataflow/src/dataflow/types.ts` | Add: `Sheet`, `Project` types if shared |

## Testing

- Unit test: migration from old flat format → new sheet format
- Unit test: add/remove sheet, cannot delete "main"
- Unit test: serialize/deserialize round-trip with multiple sheets
- Integration test: switch sheets preserves graph state
- Manual: create project, add sub-sheet, switch between them, reload page
