# DAG Frontend Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add live value monitoring via pubsub debug channels, hardware channel browser, undo/redo, and auto-save to the Pico 2-served DAG editor.

**Architecture:** MCU side gets three new API endpoints in `dag_handler.rs` (debug toggle, pubsub dump, channel registry). Frontend gets undo/redo state stack, localStorage persistence, polling loop for live values, and channel-aware inspector UX. All changes confined to two Rust files and three TypeScript/HTML files.

**Tech Stack:** Rust (no_std, heapless), TypeScript, SVG, localStorage API

---

### Task 1: MCU — Add SimplePubSub and debug mode to DagApiHandler

**Files:**
- Modify: `hil/board-support-pico2/src/dag_handler.rs`

- [ ] **Step 1: Add pubsub and debug fields to DagApiHandler**

In `dag_handler.rs`, add imports and new fields:

```rust
use dag_core::eval::{NullChannels, NullPubSub, PubSubWriter, PubSubReader};
```

Change the struct to:

```rust
pub struct DagApiHandler {
    dag: Option<Dag>,
    values: [f64; 128],
    tick_count: u64,
    debug_mode: bool,
    pubsub_topics: heapless::FnvIndexMap<heapless::String<32>, f64, 64>,
    known_inputs: heapless::Vec<heapless::String<32>, 16>,
    known_outputs: heapless::Vec<heapless::String<32>, 16>,
}
```

Update `new()`:

```rust
pub const fn new() -> Self {
    DagApiHandler {
        dag: None,
        values: [0.0; 128],
        tick_count: 0,
        debug_mode: false,
        pubsub_topics: heapless::FnvIndexMap::new(),
        known_inputs: heapless::Vec::new(),
        known_outputs: heapless::Vec::new(),
    }
}
```

Add channel registration methods:

```rust
pub fn register_input(&mut self, name: &str) {
    let _ = self.known_inputs.push(heapless::String::try_from(name).unwrap_or_default());
}

pub fn register_output(&mut self, name: &str) {
    let _ = self.known_outputs.push(heapless::String::try_from(name).unwrap_or_default());
}
```

- [ ] **Step 2: Implement a local PubSubWriter for debug publishing**

Add a helper struct inside `dag_handler.rs`:

```rust
struct MapPubSub<'a> {
    topics: &'a mut heapless::FnvIndexMap<heapless::String<32>, f64, 64>,
}

impl PubSubReader for MapPubSub<'_> {
    fn read(&self, topic: &str) -> f64 {
        self.topics.get(&heapless::String::<32>::try_from(topic).unwrap_or_default())
            .copied()
            .unwrap_or(0.0)
    }
}

impl PubSubWriter for MapPubSub<'_> {
    fn write(&mut self, topic: &str, value: f64) {
        if let Ok(key) = heapless::String::<32>::try_from(topic) {
            let _ = self.topics.insert(key, value);
        }
    }
}
```

- [ ] **Step 3: Update the tick handler to publish debug values**

Replace the `("POST", "/api/tick")` match arm to use the local pubsub and publish debug values:

```rust
("POST", "/api/tick") => {
    let mut resp = heapless::Vec::new();
    if let Some(dag) = &self.dag {
        let len = dag.len();
        if len <= 128 {
            let mut ps = MapPubSub { topics: &mut self.pubsub_topics };
            dag.evaluate(&NullChannels, &ps, &mut self.values[..len]);
            // Publish debug channels if enabled
            if self.debug_mode {
                for i in 0..len {
                    let mut key = heapless::String::<32>::new();
                    let _ = core::fmt::Write::write_fmt(&mut key, format_args!("_dbg/{}", i));
                    let _ = self.pubsub_topics.insert(key, self.values[i]);
                }
            }
            self.tick_count += 1;
        }
        let body_str = b"{\"ok\":true}";
        let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
        write_usize_to_vec(&mut resp, body_str.len());
        let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
        let _ = resp.extend_from_slice(body_str);
    } else {
        let body_str = b"{\"error\":\"no DAG loaded\"}";
        let _ = resp.extend_from_slice(b"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: ");
        write_usize_to_vec(&mut resp, body_str.len());
        let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
        let _ = resp.extend_from_slice(body_str);
    }
    Some(resp)
}
```

- [ ] **Step 4: Add POST /api/debug endpoint**

Add to the `handle()` match block:

```rust
("POST", "/api/debug") => {
    self.debug_mode = !self.debug_mode;
    if !self.debug_mode {
        // Clear debug topics when disabling
        self.pubsub_topics.retain(|k, _| !k.starts_with("_dbg/"));
    }
    let mut resp = heapless::Vec::new();
    let body = if self.debug_mode {
        b"{\"debug\":true}" as &[u8]
    } else {
        b"{\"debug\":false}" as &[u8]
    };
    let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
    write_usize_to_vec(&mut resp, body.len());
    let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
    let _ = resp.extend_from_slice(body);
    Some(resp)
}
```

- [ ] **Step 5: Add GET /api/pubsub endpoint**

Add to the `handle()` match block:

```rust
("GET", "/api/pubsub") => {
    let mut resp = heapless::Vec::new();
    // Build JSON manually: {"topic":value,...}
    let mut json_buf = [0u8; 480];
    let mut pos = 0;
    json_buf[pos] = b'{';
    pos += 1;
    let mut first = true;
    for (key, val) in self.pubsub_topics.iter() {
        if !first {
            json_buf[pos] = b',';
            pos += 1;
        }
        first = false;
        json_buf[pos] = b'"';
        pos += 1;
        let kb = key.as_bytes();
        if pos + kb.len() + 20 > json_buf.len() { break; } // safety
        json_buf[pos..pos + kb.len()].copy_from_slice(kb);
        pos += kb.len();
        json_buf[pos..pos + 2].copy_from_slice(b"\":");
        pos += 2;
        pos += write_f64_to_buf(&mut json_buf[pos..], *val);
    }
    json_buf[pos] = b'}';
    pos += 1;

    let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
    write_usize_to_vec(&mut resp, pos);
    let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
    let _ = resp.extend_from_slice(&json_buf[..pos]);
    Some(resp)
}
```

Add the `write_f64_to_buf` helper (simple integer/fixed-point formatter):

```rust
fn write_f64_to_buf(buf: &mut [u8], v: f64) -> usize {
    // Format as integer if whole, otherwise 4 decimal places
    if v == 0.0 {
        buf[0] = b'0';
        return 1;
    }
    let neg = v < 0.0;
    let abs = if neg { -v } else { v };
    let mut pos = 0;
    if neg {
        buf[pos] = b'-';
        pos += 1;
    }
    let int_part = abs as u64;
    let frac_part = ((abs - int_part as f64) * 10000.0) as u64;
    pos += write_usize_to_buf(&mut buf[pos..], int_part as usize);
    if frac_part > 0 {
        buf[pos] = b'.';
        pos += 1;
        // Write 4-digit frac with leading zeros
        let digits = [
            b'0' + ((frac_part / 1000) % 10) as u8,
            b'0' + ((frac_part / 100) % 10) as u8,
            b'0' + ((frac_part / 10) % 10) as u8,
            b'0' + (frac_part % 10) as u8,
        ];
        // Trim trailing zeros
        let mut end = 4;
        while end > 1 && digits[end - 1] == b'0' { end -= 1; }
        buf[pos..pos + end].copy_from_slice(&digits[..end]);
        pos += end;
    }
    pos
}
```

- [ ] **Step 6: Add GET /api/channels endpoint**

Add to the `handle()` match block:

```rust
("GET", "/api/channels") => {
    let mut resp = heapless::Vec::new();
    let mut json_buf = [0u8; 256];
    let mut pos = 0;
    // {"inputs":[...],"outputs":[...]}
    json_buf[pos..pos + 10].copy_from_slice(b"{\"inputs\":");
    pos += 10;
    pos += write_string_array(&mut json_buf[pos..], &self.known_inputs);
    json_buf[pos..pos + 11].copy_from_slice(b",\"outputs\":");
    pos += 11;
    pos += write_string_array(&mut json_buf[pos..], &self.known_outputs);
    json_buf[pos] = b'}';
    pos += 1;

    let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
    write_usize_to_vec(&mut resp, pos);
    let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
    let _ = resp.extend_from_slice(&json_buf[..pos]);
    Some(resp)
}
```

Add the `write_string_array` helper:

```rust
fn write_string_array(buf: &mut [u8], items: &heapless::Vec<heapless::String<32>, 16>) -> usize {
    let mut pos = 0;
    buf[pos] = b'[';
    pos += 1;
    for (i, item) in items.iter().enumerate() {
        if i > 0 { buf[pos] = b','; pos += 1; }
        buf[pos] = b'"';
        pos += 1;
        let bytes = item.as_bytes();
        buf[pos..pos + bytes.len()].copy_from_slice(bytes);
        pos += bytes.len();
        buf[pos] = b'"';
        pos += 1;
    }
    buf[pos] = b']';
    pos += 1;
    pos
}
```

- [ ] **Step 7: Register default channels in main.rs**

In `hil/board-support-pico2/src/main.rs`, after `DAG_HANDLER.init(...)`:

```rust
let dag = DAG_HANDLER.init(dag_handler::DagApiHandler::new());
dag.register_input("adc0");
dag.register_input("adc1");
dag.register_input("adc2");
dag.register_input("gpio0");
dag.register_input("gpio1");
dag.register_output("pwm0");
dag.register_output("pwm1");
dag.register_output("gpio2");
dag.register_output("gpio3");
```

- [ ] **Step 8: Build and verify**

```bash
EMBASSY_USB_MAX_INTERFACE_COUNT=16 EMBASSY_USB_MAX_HANDLER_COUNT=8 \
cargo build -p board-support-pico2 --target thumbv8m.main-none-eabihf --release
```

Expected: compiles without errors.

- [ ] **Step 9: Commit**

```bash
git add hil/board-support-pico2/src/dag_handler.rs hil/board-support-pico2/src/main.rs
git commit -m "feat(pico2): add pubsub debug mode, channel registry, and API endpoints"
```

---

### Task 2: Frontend — Undo/Redo State Stack + Auto-Save

**Files:**
- Modify: `www/dag/dag-types.ts`
- Modify: `www/dag/dag-editor.ts`

- [ ] **Step 1: Add SavedState interface to dag-types.ts**

Append to `www/dag/dag-types.ts`:

```typescript
export interface SavedState {
  nodes: DagNode[];
  nextId: number;
  panX: number;
  panY: number;
  scale: number;
}
```

- [ ] **Step 2: Add undo/redo infrastructure to dag-editor.ts**

Add after the `let nextId = 0;` line in `dag-editor.ts`:

```typescript
import { DagNode, DagState, NodeId, SavedState } from './dag-types.js';

// --- Undo/Redo ---
const MAX_UNDO = 50;
let undoStack: string[] = [];
let redoStack: string[] = [];

function captureState(): string {
  return JSON.stringify({
    nodes: state.nodes,
    nextId,
    panX: state.panX,
    panY: state.panY,
    scale: state.scale,
  } as SavedState);
}

function pushUndo() {
  undoStack.push(captureState());
  if (undoStack.length > MAX_UNDO) undoStack.shift();
  redoStack = [];
  scheduleAutoSave();
}

function restoreState(json: string) {
  const saved: SavedState = JSON.parse(json);
  state.nodes = saved.nodes;
  nextId = saved.nextId;
  state.panX = saved.panX;
  state.panY = saved.panY;
  state.scale = saved.scale;
  state.selectedId = null;
  hideInspector();
  render();
  updateStatus();
}

function undo() {
  if (undoStack.length === 0) return;
  redoStack.push(captureState());
  restoreState(undoStack.pop()!);
}

function redo() {
  if (redoStack.length === 0) return;
  undoStack.push(captureState());
  restoreState(redoStack.pop()!);
}
```

- [ ] **Step 3: Add auto-save to localStorage**

Add after the undo/redo code:

```typescript
// --- Auto-Save ---
const STORAGE_KEY = 'dag:state';
let saveTimer: number | null = null;

function scheduleAutoSave() {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    localStorage.setItem(STORAGE_KEY, captureState());
    saveTimer = null;
  }, 1000);
}

function loadSavedState(): boolean {
  const json = localStorage.getItem(STORAGE_KEY);
  if (!json) return false;
  try {
    restoreState(json);
    return true;
  } catch {
    return false;
  }
}

function clearSavedState() {
  localStorage.removeItem(STORAGE_KEY);
}
```

- [ ] **Step 4: Wire pushUndo into mutations**

Add `pushUndo()` call as the first line in:
- `addNode()` — before `state.nodes.push(node)`
- `removeNode()` — before `state.nodes = state.nodes.filter(...)`
- In `setupInteraction()` `mousedown` handler — before setting `destNode.a/b/src` (connection made)
- In `setupInteraction()` `mouseup` handler — after a drag ended (if dragNode was set):

```typescript
svg.addEventListener('mouseup', () => {
  if (dragNode) {
    pushUndo(); // Capture position change
  }
  dragNode = null;
});
```

- In `showInspector()` — in each input's `change` event listener, call `pushUndo()` before setting the value.

- [ ] **Step 5: Wire Ctrl+Z / Ctrl+Shift+Z**

In `setupInteraction()`, update the keydown handler:

```typescript
document.addEventListener('keydown', (e) => {
  const active = document.activeElement;
  const isInput = active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA');

  if (e.key === 'z' && (e.ctrlKey || e.metaKey) && !e.shiftKey && !isInput) {
    e.preventDefault();
    undo();
    return;
  }
  if (e.key === 'z' && (e.ctrlKey || e.metaKey) && e.shiftKey && !isInput) {
    e.preventDefault();
    redo();
    return;
  }
  if ((e.key === 'Delete' || e.key === 'Backspace') && !isInput) {
    if (state.selectedId !== null) {
      removeNode(state.selectedId);
      hideInspector();
    }
  }
});
```

- [ ] **Step 6: Wire loadSavedState into init, clear button**

In `init()`, add before `render()`:

```typescript
loadSavedState();
```

Update the clear button handler:

```typescript
document.getElementById('btn-clear')!.addEventListener('click', () => {
  pushUndo();
  state.nodes = [];
  state.selectedId = null;
  nextId = 0;
  hideInspector();
  clearSavedState();
  render();
  updateStatus();
});
```

- [ ] **Step 7: Build and verify**

```bash
cd www && npx esbuild dag/dag-editor.ts --bundle --format=esm --target=es2022 --minify --external:../pkg/rustcam.js --outfile=dag/dag-editor.js
```

Expected: builds without TypeScript errors.

- [ ] **Step 8: Commit**

```bash
git add www/dag/dag-types.ts www/dag/dag-editor.ts www/dag/dag-editor.js
git commit -m "feat(dag-editor): add undo/redo state stack and localStorage auto-save"
```

---

### Task 3: Frontend — Live Values via PubSub Polling + Debug Toggle

**Files:**
- Modify: `www/dag/dag-editor.ts`
- Modify: `www/dag/index.html`

- [ ] **Step 1: Add toolbar buttons to index.html**

In `www/dag/index.html`, after the clear button and before `</div>` closing the toolbar:

```html
  <span class="sep"></span>
  <button id="btn-debug" title="Toggle debug mode">Debug</button>
  <button id="btn-autotick" title="Auto-tick at 2Hz">Auto</button>
```

Add style for active state in the `<style>` block:

```css
  #toolbar button.active { background: #533483; border-color: #e0e0e0; }
```

- [ ] **Step 2: Add polling state and debug toggle to dag-editor.ts**

Add to module-level state in `dag-editor.ts`:

```typescript
// --- Live polling ---
let debugMode = false;
let autoTickInterval: number | null = null;
let pollInterval: number | null = null;
let lastNodeMap: Map<number, number> = new Map(); // visual id → dag node index
let knownChannels: { inputs: string[]; outputs: string[] } = { inputs: [], outputs: [] };
```

Add the debug toggle function:

```typescript
async function toggleDebug() {
  try {
    const resp = await fetch('/api/debug', { method: 'POST' });
    const data = await resp.json();
    debugMode = data.debug;
    document.getElementById('btn-debug')!.classList.toggle('active', debugMode);

    if (debugMode) {
      startPolling();
    } else {
      stopPolling();
      // Clear live results from nodes
      for (const n of state.nodes) n.result = undefined;
      render();
    }
  } catch (e) {
    document.getElementById('st-result')!.textContent = `Debug error: ${e}`;
  }
}

function startPolling() {
  if (pollInterval !== null) return;
  pollInterval = window.setInterval(pollPubSub, 500);
}

function stopPolling() {
  if (pollInterval !== null) {
    clearInterval(pollInterval);
    pollInterval = null;
  }
}
```

- [ ] **Step 3: Add pubsub polling function**

```typescript
async function pollPubSub() {
  try {
    const resp = await fetch('/api/pubsub');
    const topics: Record<string, number> = await resp.json();

    // Update node live values from _dbg/* topics
    for (const [key, val] of Object.entries(topics)) {
      if (key.startsWith('_dbg/')) {
        const dagIdx = parseInt(key.slice(5));
        // Find visual node that maps to this dag index
        for (const [visualId, mapIdx] of lastNodeMap) {
          if (mapIdx === dagIdx) {
            const node = state.nodes.find(n => n.id === visualId);
            if (node) node.result = val;
          }
        }
      }
    }
    render();

    // Update inspector if a node is selected
    if (state.selectedId !== null) {
      const sel = state.nodes.find(n => n.id === state.selectedId);
      if (sel) showInspector(sel);
    }
  } catch {
    // Silently ignore poll errors
  }
}
```

- [ ] **Step 4: Add auto-tick toggle**

```typescript
function toggleAutoTick() {
  if (autoTickInterval !== null) {
    clearInterval(autoTickInterval);
    autoTickInterval = null;
    document.getElementById('btn-autotick')!.classList.toggle('active', false);
  } else {
    autoTickInterval = window.setInterval(async () => {
      try { await fetch('/api/tick', { method: 'POST' }); } catch {}
    }, 500);
    document.getElementById('btn-autotick')!.classList.toggle('active', true);
  }
}
```

- [ ] **Step 5: Update pushToMCU to save nodeMap**

In the `pushToMCU()` function, after `buildDagHandle()`, save the mapping:

```typescript
async function pushToMCU() {
  if (!wasm) {
    document.getElementById('st-result')!.textContent = 'WASM not loaded';
    return;
  }

  try {
    const { handle, nodeMap } = buildDagHandle();
    lastNodeMap = nodeMap; // Save for live value mapping
    const cbor: Uint8Array = handle.to_cbor();
    handle.free();

    const resp = await fetch('/api/dag', {
      method: 'POST',
      headers: { 'Content-Type': 'application/cbor' },
      body: cbor,
    });

    document.getElementById('st-result')!.textContent =
      resp.ok ? `Pushed ${cbor.length}B to MCU` : `Push failed: ${resp.status}`;
  } catch (e) {
    document.getElementById('st-result')!.textContent = `Push error: ${e}`;
  }
}
```

- [ ] **Step 6: Wire buttons in init()**

Add to the `init()` function:

```typescript
document.getElementById('btn-debug')!.addEventListener('click', toggleDebug);
document.getElementById('btn-autotick')!.addEventListener('click', toggleAutoTick);
```

- [ ] **Step 7: Build and verify**

```bash
cd www && npx esbuild dag/dag-editor.ts --bundle --format=esm --target=es2022 --minify --external:../pkg/rustcam.js --outfile=dag/dag-editor.js
```

- [ ] **Step 8: Commit**

```bash
git add www/dag/dag-editor.ts www/dag/dag-editor.js www/dag/index.html
git commit -m "feat(dag-editor): add live values via pubsub debug polling and auto-tick"
```

---

### Task 4: Frontend — Hardware Panel + Channel Name UX

**Files:**
- Modify: `www/dag/index.html`
- Modify: `www/dag/dag-editor.ts`

- [ ] **Step 1: Add hardware panel to index.html**

In `www/dag/index.html`, add a hardware panel `<div>` between `#toolbar` and `#workspace`:

```html
<div id="hw-panel" style="display:none; padding:4px 8px; background:#0f3460; font-size:11px; border-bottom:1px solid #533483;">
  <strong>Channels</strong>
  <span id="hw-body"></span>
</div>
```

Add a toggle button in the toolbar (after the Auto button):

```html
  <button id="btn-hw" title="Toggle hardware panel">HW</button>
```

Add CSS for the select element in the `<style>` block:

```css
  #node-inspector select { width: 80px; background: #0f3460; border: 1px solid #533483; color: #e0e0e0; padding: 2px; font-size: 11px; }
```

- [ ] **Step 2: Add channel fetching**

Add to `dag-editor.ts`:

```typescript
async function fetchChannels() {
  try {
    const resp = await fetch('/api/channels');
    knownChannels = await resp.json();
  } catch {
    knownChannels = { inputs: [], outputs: [] };
  }
}
```

- [ ] **Step 3: Add hardware panel rendering**

```typescript
function renderHwPanel(pubsubData?: Record<string, number>) {
  const body = document.getElementById('hw-body')!;
  const parts: string[] = [];

  for (const name of knownChannels.inputs) {
    const val = pubsubData?.[name];
    parts.push(` IN:${name}${val !== undefined ? '=' + val.toFixed(2) : ''}`);
  }
  for (const name of knownChannels.outputs) {
    const val = pubsubData?.[name];
    parts.push(` OUT:${name}${val !== undefined ? '=' + val.toFixed(2) : ''}`);
  }

  // Show user pubsub topics (non-debug)
  if (pubsubData) {
    for (const [key, val] of Object.entries(pubsubData)) {
      if (!key.startsWith('_dbg/') && !knownChannels.inputs.includes(key) && !knownChannels.outputs.includes(key)) {
        parts.push(` PS:${key}=${val.toFixed(2)}`);
      }
    }
  }

  body.textContent = parts.length > 0 ? parts.join(' | ') : ' (none)';
}
```

- [ ] **Step 4: Update pollPubSub to also update hardware panel**

In `pollPubSub()`, after updating node results and before `render()`:

```typescript
renderHwPanel(topics);
```

- [ ] **Step 5: Update addNode for auto-generated output/publish names**

In `addNode()`, change the name initialization:

```typescript
if (actualOp === 'input') node.name = '';
if (actualOp === 'output') node.name = `out_${node.id}`;
if (actualOp === 'subscribe') node.name = '';
if (actualOp === 'publish') node.name = `pub_${node.id}`;
```

- [ ] **Step 6: Update showInspector for channel select**

Replace the `if (node.name !== undefined)` block in `showInspector()`:

```typescript
if (node.name !== undefined) {
  const field = document.createElement('div');
  field.className = 'field';

  if (node.op === 'input' || node.op === 'subscribe') {
    // Show select dropdown for input channels
    field.textContent = 'Channel: ';
    const sel = document.createElement('select');
    const emptyOpt = document.createElement('option');
    emptyOpt.value = '';
    emptyOpt.textContent = '(custom)';
    sel.appendChild(emptyOpt);

    const channelList = node.op === 'input' ? knownChannels.inputs : knownChannels.inputs;
    for (const ch of channelList) {
      const opt = document.createElement('option');
      opt.value = ch;
      opt.textContent = ch;
      if (ch === node.name) opt.selected = true;
      sel.appendChild(opt);
    }
    sel.addEventListener('change', () => {
      pushUndo();
      node.name = sel.value;
      render();
    });
    field.appendChild(sel);

    // Also show text input for custom names
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = node.name || '';
    inp.placeholder = 'custom';
    inp.style.marginLeft = '4px';
    inp.addEventListener('change', () => {
      pushUndo();
      node.name = inp.value;
      render();
    });
    field.appendChild(inp);
  } else {
    // Output/publish: editable text field with auto-generated name
    field.textContent = 'Name: ';
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = node.name || '';
    inp.addEventListener('change', () => {
      pushUndo();
      node.name = inp.value;
      render();
    });
    field.appendChild(inp);
  }

  body.appendChild(field);
}
```

- [ ] **Step 7: Wire HW panel toggle and fetch channels on init**

In `init()`:

```typescript
document.getElementById('btn-hw')!.addEventListener('click', () => {
  const panel = document.getElementById('hw-panel')!;
  const visible = panel.style.display !== 'none';
  panel.style.display = visible ? 'none' : 'block';
  document.getElementById('btn-hw')!.classList.toggle('active', !visible);
  if (!visible) fetchChannels().then(() => renderHwPanel());
});

// Fetch channels on startup
fetchChannels();
```

- [ ] **Step 8: Build and verify**

```bash
cd www && npx esbuild dag/dag-editor.ts --bundle --format=esm --target=es2022 --minify --external:../pkg/rustcam.js --outfile=dag/dag-editor.js
```

- [ ] **Step 9: Commit**

```bash
git add www/dag/index.html www/dag/dag-editor.ts www/dag/dag-editor.js
git commit -m "feat(dag-editor): add hardware panel, channel select, auto-named outputs"
```

---

### Task 5: Build, Flash, and Verify End-to-End

**Files:**
- No new files

- [ ] **Step 1: Build DAG frontend**

```bash
cd www && npx esbuild dag/dag-editor.ts --bundle --format=esm --target=es2022 --minify --external:../pkg/rustcam.js --outfile=dag/dag-editor.js
```

- [ ] **Step 2: Build Pico 2 firmware**

```bash
EMBASSY_USB_MAX_INTERFACE_COUNT=16 EMBASSY_USB_MAX_HANDLER_COUNT=8 \
cargo build -p board-support-pico2 --target thumbv8m.main-none-eabihf --release
```

- [ ] **Step 3: Flash to Pico 2**

```bash
probe-rs download --chip RP235x target/thumbv8m.main-none-eabihf/release/board-support-pico2
probe-rs reset --chip RP235x
```

- [ ] **Step 4: Verify API endpoints**

```bash
# Check channels
curl -s http://169.254.1.61:8080/api/channels
# Expected: {"inputs":["adc0","adc1","adc2","gpio0","gpio1"],"outputs":["pwm0","pwm1","gpio2","gpio3"]}

# Enable debug
curl -s -X POST http://169.254.1.61:8080/api/debug
# Expected: {"debug":true}

# Check pubsub (empty before any ticks)
curl -s http://169.254.1.61:8080/api/pubsub
# Expected: {}

# Push a simple DAG and tick
python3 -c "
import struct, sys
data = bytes([0x81, 0x82, 0x00, 0xFB]) + struct.pack('>d', 42.0)
sys.stdout.buffer.write(data)
" | curl -s -X POST -H 'Content-Type: application/cbor' --data-binary @- http://169.254.1.61:8080/api/dag
# Expected: {"ok":true,"nodes":1}

curl -s -X POST http://169.254.1.61:8080/api/tick
# Expected: {"ok":true}

curl -s http://169.254.1.61:8080/api/pubsub
# Expected: {"_dbg/0":42}
```

- [ ] **Step 5: Verify browser loads updated editor**

Open `http://169.254.1.61:8080/` in browser. Verify:
- Debug and Auto buttons visible in toolbar
- HW button shows channel panel
- Ctrl+Z/Ctrl+Shift+Z work for undo/redo
- Refreshing page restores last state from localStorage

- [ ] **Step 6: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix: end-to-end verification fixups"
```
