# Fix Wire Connection: Systematic Plan

## Root Cause Analysis

The drag-to-connect feature fails because:

1. `onPointerDown` on a `.df-port` calls `e.preventDefault()` (port-view.ts:142)
2. This triggers **implicit pointer capture** — the browser routes ALL subsequent
   pointer events to the originating element (the source port dot)
3. On `pointerup`, `e.target` is always the source port (not the target port)
4. `elementFromPoint(e.clientX, e.clientY)` during a pointer-captured event
   may return the captured element or be unreliable
5. The `.df-edge.dragging` SVG path with `pointer-events: stroke` also
   interferes with hit testing

## Evidence

Playwright E2E test shows:
- `Pre-release hit` (called via page.evaluate before mouse.up): `{cls: "df-port", side: "input"}` ✓
- Actual pointerup trace: `{targetElement: "DIV"}` with NO `toBlock` — hit wrong element
- WASM `dataflow_connect()` called directly: `success` ✓

## Fix Strategy: Release Pointer Capture + Robust Hit Testing

### Step 1: Release pointer capture in onPointerDown

```typescript
function onPointerDown(e: PointerEvent): void {
    // ... existing port detection code ...
    
    e.preventDefault();
    e.stopPropagation();
    
    // Release implicit pointer capture so pointerup fires on the
    // element under the cursor, not the source port
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
}
```

This is the **primary fix**. By releasing pointer capture, the browser
will correctly dispatch pointerup to whatever element is under the cursor.

### Step 2: Keep elementFromPoint as fallback

The `elementFromPoint` + `closest('.df-port')` logic stays as a safety net
for cases where the direct target isn't a port.

### Step 3: Keep .df-edge.dragging { pointer-events: none }

The CSS fix preventing the drag wire from intercepting clicks stays.

### Step 4: Verify with E2E test

The Playwright test should show:
- Wire trace with `result: "success"` 
- `Edges after connect attempt: >= 1`

### Step 5: Add assertion test

New E2E test that:
1. Adds Constant + Gain blocks
2. Drags from Constant output to Gain input
3. Asserts `edges > 0` in the snapshot
4. Asserts wire trace shows `result: "success"`

## Files to Change

1. `www-dataflow/src/dataflow/port-view.ts` — add releasePointerCapture
2. `www/src/dataflow/port-view.ts` — same
3. `tests/e2e_connect.spec.ts` — add success assertion

## Verification

1. `npx playwright test` — E2E shows edges created
2. Manual: open http://localhost:3000, drag blue dot to blue dot, see solid line
3. Console shows `[wire-trace] {... result: "success"}`
