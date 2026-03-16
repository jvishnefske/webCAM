/** Constraint engine — bridges sketch shapes to WASM sketch_actor. */

import {
  sketch_reset, sketch_add_point, sketch_add_constraint,
  sketch_solve, sketch_remove_constraint,
} from '../pkg/rustcam.js';
import { $ } from './dom.js';
import {
  sketchShapes, sketchCvs, sketchCtx2d, sketchGridSnap,
  sketchCanvasSize, sketchScreenToWorld, getCurrentMode,
  redrawSketch as baseRedraw, setOnShapeListUpdate,
} from './sketch.js';
import type { SketchSnapshot, SketchPoint, DofStatus } from './types.js';

// ── State ────────────────────────────────────────────────────────────

let cstMode: string | null = null;
let cstPicks: number[] = [];
let cstPointMap: Record<number, SketchPoint> = {};
let cstLastSnap: SketchSnapshot | null = null;

const cstStatusEl = $('cst-status');
const cstDofEl    = $('cst-dof');
const cstListEl   = $('cst-list');

const CST_PICK_COUNT: Record<string, number> = {
  coincident: 2, horizontal: 2, vertical: 2, distance: 2,
  fixed: 1, perpendicular: 4, parallel: 4, equal_length: 4,
};

// Shape → point id mapping
interface ShapePoints { shapeIdx: number; pointIds: number[] }
let shapePointIds: ShapePoints[] = [];

// ── Constraint tool buttons ──────────────────────────────────────────

document.querySelectorAll('.btn-cst').forEach(btn => {
  btn.addEventListener('click', () => {
    const type = (btn as HTMLElement).dataset.cst!;
    if (cstMode === type) { cancelCstPick(); return; }
    cstMode = type;
    cstPicks = [];
    document.querySelectorAll('.btn-cst').forEach(b => b.classList.remove('active'));
    (btn as HTMLElement).classList.add('active');
    cstStatusEl.textContent = `Pick ${CST_PICK_COUNT[type]} point(s) on canvas…`;
  });
});

function cancelCstPick(): void {
  cstMode = null;
  cstPicks = [];
  document.querySelectorAll('.btn-cst').forEach(b => b.classList.remove('active'));
  cstStatusEl.textContent = '';
}

// ── Sync shapes → WASM actor ─────────────────────────────────────────

function syncShapesToActor(): void {
  sketch_reset();
  shapePointIds = [];

  for (let i = 0; i < sketchShapes.length; i++) {
    const s = sketchShapes[i];
    const ids: number[] = [];
    if (s.type === 'line') {
      ids.push(JSON.parse(sketch_add_point(s.p1.x, s.p1.y)).id);
      ids.push(JSON.parse(sketch_add_point(s.p2.x, s.p2.y)).id);
    } else if (s.type === 'rect') {
      const rx = Math.min(s.x, s.x + s.w), ry = Math.min(s.y, s.y + s.h);
      const w = Math.abs(s.w), h = Math.abs(s.h);
      ids.push(JSON.parse(sketch_add_point(rx, ry)).id);
      ids.push(JSON.parse(sketch_add_point(rx + w, ry)).id);
      ids.push(JSON.parse(sketch_add_point(rx + w, ry + h)).id);
      ids.push(JSON.parse(sketch_add_point(rx, ry + h)).id);
    } else if (s.type === 'circle') {
      ids.push(JSON.parse(sketch_add_point(s.cx, s.cy)).id);
      ids.push(JSON.parse(sketch_add_point(s.cx + s.r, s.cy)).id);
    } else if (s.type === 'polyline') {
      for (const p of s.points) {
        ids.push(JSON.parse(sketch_add_point(p.x, p.y)).id);
      }
    }
    shapePointIds.push({ shapeIdx: i, pointIds: ids });
  }
  solveAndRender();
}

function solveAndRender(): void {
  try {
    const json = sketch_solve();
    cstLastSnap = JSON.parse(json);
    cstPointMap = {};
    for (const [id, pt] of cstLastSnap!.points) {
      cstPointMap[id] = pt;
    }
    // DOF display
    const d = cstLastSnap!.dof;
    const status = cstLastSnap!.dof_status;
    const color = status === 'FullyConstrained' ? '#4caf50'
                : status === 'OverConstrained' ? '#f44336' : '#ff9800';
    cstDofEl.style.color = color;
    cstDofEl.textContent = `DOF: ${d} (${status.replace(/([A-Z])/g, ' $1').trim()})`;
    // Constraint list
    cstListEl.innerHTML = cstLastSnap!.constraints.map(([id, c]) => {
      const type = Object.keys(c)[0] || 'unknown';
      return `<div>${id}. ${type} <button onclick="window.__removeCst(${id})" style="font-size:10px;cursor:pointer;background:none;border:none;color:#f44336">✕</button></div>`;
    }).join('');
    baseRedraw();
  } catch (e) {
    cstStatusEl.textContent = `Solve error: ${e}`;
  }
}

// Expose remove to onclick
(window as any).__removeCst = (id: number) => {
  sketch_remove_constraint(id);
  solveAndRender();
};

// ── Point picking ────────────────────────────────────────────────────

function findNearestPoint(wx: number, wy: number, maxDist: number): number | null {
  let best: number | null = null, bestD = maxDist;
  for (const entry of shapePointIds) {
    for (const pid of entry.pointIds) {
      const pt = cstPointMap[pid];
      if (!pt) continue;
      const d = Math.sqrt((pt.x - wx) ** 2 + (pt.y - wy) ** 2);
      if (d < bestD) { bestD = d; best = pid; }
    }
  }
  return best;
}

sketchCvs.addEventListener('click', (e: MouseEvent) => {
  if (getCurrentMode() !== 'sketch' || !cstMode) return;
  const p = sketchScreenToWorld(e.clientX, e.clientY);
  const snapDist = (sketchGridSnap() || 1) * 2;
  const pid = findNearestPoint(p.x, p.y, snapDist);
  if (pid == null) {
    cstStatusEl.textContent = 'No point nearby — click closer to a shape vertex.';
    return;
  }
  cstPicks.push(pid);
  const needed = CST_PICK_COUNT[cstMode];
  cstStatusEl.textContent = `Picked ${cstPicks.length}/${needed} points…`;
  if (cstPicks.length >= needed) {
    applyConstraint();
  }
});

function applyConstraint(): void {
  let value = 0, value2 = 0;

  if (cstMode === 'distance') {
    const a = cstPointMap[cstPicks[0]], b = cstPointMap[cstPicks[1]];
    const cur = Math.sqrt((b.x - a.x) ** 2 + (b.y - a.y) ** 2);
    const input = prompt(`Distance (current: ${cur.toFixed(2)} mm):`, cur.toFixed(2));
    if (input == null) { cancelCstPick(); return; }
    value = parseFloat(input);
    if (isNaN(value) || value <= 0) { cancelCstPick(); return; }
  } else if (cstMode === 'fixed') {
    const pt = cstPointMap[cstPicks[0]];
    value = pt.x;
    value2 = pt.y;
  }

  try {
    sketch_add_constraint(cstMode!, JSON.stringify(cstPicks), value, value2);
    solveAndRender();
    cstStatusEl.textContent = `Added ${cstMode} constraint.`;
  } catch (e) {
    cstStatusEl.textContent = `Error: ${e}`;
  }
  cstMode = null;
  cstPicks = [];
  document.querySelectorAll('.btn-cst').forEach(b => b.classList.remove('active'));
}

// ── Hook into shape list updates ─────────────────────────────────────

setOnShapeListUpdate(syncShapesToActor);

// ── Overlay drawing (constraint points + picks) ──────────────────────

export function drawConstraintOverlay(): void {
  if (!cstLastSnap || getCurrentMode() !== 'sketch') return;
  const ctx = sketchCtx2d;
  const dpr = window.devicePixelRatio || 1;
  const rect = sketchCvs.getBoundingClientRect();
  if (rect.width < 1) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const size = sketchCanvasSize();
  const pad = 20;
  const avail = Math.min(rect.width, rect.height) - pad * 2;
  const scale = avail / size;
  const offX = (rect.width - size * scale) / 2;
  const offY = (rect.height - size * scale) / 2;
  const tx = (v: number) => offX + v * scale;
  const ty = (v: number) => offY + v * scale;

  for (const [id, pt] of cstLastSnap.points) {
    const status: DofStatus = cstLastSnap.point_status[id] || 'UnderConstrained';
    const color = status === 'FullyConstrained' ? '#4caf50'
                : status === 'OverConstrained' ? '#f44336' : '#ff9800';
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(tx(pt.x), ty(pt.y), 4, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#8888a0';
    ctx.font = '9px monospace';
    ctx.fillText(String(id), tx(pt.x) + 5, ty(pt.y) - 5);
  }

  ctx.strokeStyle = '#ffeb3b';
  ctx.lineWidth = 2;
  for (const pid of cstPicks) {
    const pt = cstPointMap[pid];
    if (!pt) continue;
    ctx.beginPath();
    ctx.arc(tx(pt.x), ty(pt.y), 7, 0, Math.PI * 2);
    ctx.stroke();
  }

  ctx.restore();
}
