/** 2D sketch drawing engine. */

import { $, $input, $canvas } from './dom.js';
import type { SketchShape, DraftShape } from './types.js';
import { theme } from './theme.js';

// ── State ────────────────────────────────────────────────────────────

export const sketchShapes: SketchShape[] = [];
export let sketchTool = 'line';
export let sketchDraft: DraftShape | SketchShape | null = null;
export let sketchPolyPts: Array<{ x: number; y: number }> = [];

let _currentMode: 'cam' | 'sketch' = 'cam';
export function getCurrentMode(): 'cam' | 'sketch' { return _currentMode; }
export function setCurrentMode(m: 'cam' | 'sketch'): void { _currentMode = m; }

const sketchCvs = $canvas('sketch-canvas');
const sketchCtx2d = sketchCvs.getContext('2d')!;
const sketchCursorEl = $('sketch-cursor-pos');

export { sketchCvs, sketchCtx2d };

// ── Tool selection ───────────────────────────────────────────────────

document.querySelectorAll('.btn-tool').forEach(btn => {
  btn.addEventListener('click', () => {
    finishPolyline();
    sketchDraft = null;
    sketchTool = (btn as HTMLElement).dataset.tool!;
    document.querySelectorAll('.btn-tool').forEach(b => b.classList.remove('active'));
    (btn as HTMLElement).classList.add('active');
    redrawSketch();
  });
});

// ── Coordinate helpers ───────────────────────────────────────────────

export function sketchCanvasSize(): number {
  return parseFloat($input('canvas-size').value) || 100;
}

export function sketchGridSnap(): number {
  return parseFloat($input('grid-snap').value) || 0;
}

export function sketchScreenToWorld(clientX: number, clientY: number): { x: number; y: number } {
  const rect = sketchCvs.getBoundingClientRect();
  const size = sketchCanvasSize();
  const pad = 20;
  const avail = Math.min(rect.width, rect.height) - pad * 2;
  const scale = avail / size;
  const offX = (rect.width - size * scale) / 2;
  const offY = (rect.height - size * scale) / 2;
  let x = (clientX - rect.left - offX) / scale;
  let y = (clientY - rect.top - offY) / scale;
  const snap = sketchGridSnap();
  if (snap > 0) {
    x = Math.round(x / snap) * snap;
    y = Math.round(y / snap) * snap;
  }
  return { x: Math.round(x * 100) / 100, y: Math.round(y * 100) / 100 };
}

export function resizeSketchCanvas(): void {
  const dpr = window.devicePixelRatio || 1;
  const rect = sketchCvs.getBoundingClientRect();
  if (rect.width < 1) return;
  sketchCvs.width = rect.width * dpr;
  sketchCvs.height = rect.height * dpr;
}

// ── Drawing ──────────────────────────────────────────────────────────

export function redrawSketch(): void {
  const ctx = sketchCtx2d;
  const dpr = window.devicePixelRatio || 1;
  const rect = sketchCvs.getBoundingClientRect();
  if (rect.width < 1) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, rect.width, rect.height);

  const size = sketchCanvasSize();
  const pad = 20;
  const avail = Math.min(rect.width, rect.height) - pad * 2;
  const scale = avail / size;
  const offX = (rect.width - size * scale) / 2;
  const offY = (rect.height - size * scale) / 2;
  const tx = (v: number) => offX + v * scale;
  const ty = (v: number) => offY + v * scale;

  // Grid
  ctx.strokeStyle = theme.colors.surface;
  ctx.lineWidth = 0.5;
  const snap = sketchGridSnap() || (size / 10);
  const maxLines = 200;
  if (size / snap <= maxLines) {
    for (let v = 0; v <= size; v += snap) {
      ctx.beginPath(); ctx.moveTo(tx(v), ty(0)); ctx.lineTo(tx(v), ty(size)); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(tx(0), ty(v)); ctx.lineTo(tx(size), ty(v)); ctx.stroke();
    }
  }

  // Canvas border
  ctx.strokeStyle = theme.colors.border;
  ctx.lineWidth = 1;
  ctx.strokeRect(tx(0), ty(0), size * scale, size * scale);

  // Origin label
  ctx.fillStyle = theme.colors.textDim;
  ctx.font = '10px monospace';
  ctx.fillText('0,0', tx(0) + 2, ty(0) - 4);
  ctx.fillText(`${size},${size}`, tx(size) - 40, ty(size) + 12);

  // Committed shapes
  for (const s of sketchShapes) drawShape(ctx, s, theme.colors.accent, tx, ty, scale);

  // Draft shape
  if (sketchDraft) drawShape(ctx, sketchDraft as SketchShape, theme.colors.success, tx, ty, scale);

  // Polyline in-progress points
  if (sketchTool === 'polyline' && sketchPolyPts.length > 0) {
    ctx.strokeStyle = theme.colors.success;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(tx(sketchPolyPts[0].x), ty(sketchPolyPts[0].y));
    for (let i = 1; i < sketchPolyPts.length; i++)
      ctx.lineTo(tx(sketchPolyPts[i].x), ty(sketchPolyPts[i].y));
    if (sketchDraft && '_cursor' in sketchDraft && sketchDraft._cursor)
      ctx.lineTo(tx(sketchDraft._cursor.x), ty(sketchDraft._cursor.y));
    ctx.stroke();
    ctx.fillStyle = theme.colors.success;
    for (const p of sketchPolyPts) {
      ctx.beginPath(); ctx.arc(tx(p.x), ty(p.y), 3, 0, Math.PI * 2); ctx.fill();
    }
  }

  ctx.restore();
}

function drawShape(
  ctx: CanvasRenderingContext2D,
  s: SketchShape | DraftShape,
  color: string,
  tx: (v: number) => number,
  ty: (v: number) => number,
  scale: number,
): void {
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  const shape = s as SketchShape;
  if (!('type' in shape)) return;
  switch (shape.type) {
    case 'line':
      ctx.beginPath();
      ctx.moveTo(tx(shape.p1.x), ty(shape.p1.y));
      ctx.lineTo(tx(shape.p2.x), ty(shape.p2.y));
      ctx.stroke();
      break;
    case 'rect':
      ctx.strokeRect(
        tx(Math.min(shape.x, shape.x + shape.w)),
        ty(Math.min(shape.y, shape.y + shape.h)),
        Math.abs(shape.w) * scale, Math.abs(shape.h) * scale,
      );
      break;
    case 'circle':
      ctx.beginPath();
      ctx.arc(tx(shape.cx), ty(shape.cy), shape.r * scale, 0, Math.PI * 2);
      ctx.stroke();
      break;
    case 'polyline':
      if (shape.points.length < 2) break;
      ctx.beginPath();
      ctx.moveTo(tx(shape.points[0].x), ty(shape.points[0].y));
      for (let i = 1; i < shape.points.length; i++)
        ctx.lineTo(tx(shape.points[i].x), ty(shape.points[i].y));
      ctx.stroke();
      break;
  }
}

// ── Mouse interaction ────────────────────────────────────────────────

let sketchMouseDown = false;
let sketchStart: { x: number; y: number } | null = null;

sketchCvs.addEventListener('mousedown', (e: MouseEvent) => {
  if (getCurrentMode() !== 'sketch') return;
  if (sketchTool === 'polyline') return;
  const p = sketchScreenToWorld(e.clientX, e.clientY);
  sketchMouseDown = true;
  sketchStart = p;
  if (sketchTool === 'line') sketchDraft = { type: 'line', p1: p, p2: p };
  else if (sketchTool === 'rect') sketchDraft = { type: 'rect', x: p.x, y: p.y, w: 0, h: 0 };
  else if (sketchTool === 'circle') sketchDraft = { type: 'circle', cx: p.x, cy: p.y, r: 0 };
});

sketchCvs.addEventListener('mousemove', (e: MouseEvent) => {
  if (getCurrentMode() !== 'sketch') return;
  const p = sketchScreenToWorld(e.clientX, e.clientY);
  sketchCursorEl.textContent = `${p.x}, ${p.y} mm`;
  if (sketchTool === 'polyline' && sketchPolyPts.length > 0) {
    sketchDraft = { _cursor: p } as DraftShape;
    redrawSketch();
    return;
  }
  if (!sketchMouseDown || !sketchDraft) return;
  if (sketchTool === 'line' && 'p2' in sketchDraft) (sketchDraft as any).p2 = p;
  else if (sketchTool === 'rect' && 'w' in sketchDraft) {
    (sketchDraft as any).w = p.x - sketchStart!.x;
    (sketchDraft as any).h = p.y - sketchStart!.y;
  }
  else if (sketchTool === 'circle' && 'r' in sketchDraft) {
    const dx = p.x - sketchStart!.x, dy = p.y - sketchStart!.y;
    (sketchDraft as any).r = Math.round(Math.sqrt(dx * dx + dy * dy) * 100) / 100;
  }
  redrawSketch();
});

sketchCvs.addEventListener('mouseup', () => {
  if (getCurrentMode() !== 'sketch' || !sketchMouseDown) return;
  sketchMouseDown = false;
  if (!sketchDraft || !('type' in sketchDraft)) return;
  const d = sketchDraft as SketchShape;
  if (d.type === 'line') {
    if (d.p1.x === d.p2.x && d.p1.y === d.p2.y) { sketchDraft = null; return; }
  } else if (d.type === 'rect') {
    if (d.w === 0 || d.h === 0) { sketchDraft = null; return; }
  } else if (d.type === 'circle') {
    if (d.r === 0) { sketchDraft = null; return; }
  }
  sketchShapes.push(d);
  sketchDraft = null;
  updateShapeList();
  redrawSketch();
});

// Polyline tool
sketchCvs.addEventListener('click', (e: MouseEvent) => {
  if (getCurrentMode() !== 'sketch' || sketchTool !== 'polyline') return;
  const p = sketchScreenToWorld(e.clientX, e.clientY);
  sketchPolyPts.push(p);
  redrawSketch();
});

sketchCvs.addEventListener('dblclick', () => {
  if (getCurrentMode() !== 'sketch' || sketchTool !== 'polyline') return;
  if (sketchPolyPts.length > 0) sketchPolyPts.pop();
  finishPolyline();
});

export function finishPolyline(): void {
  if (sketchPolyPts.length >= 2) {
    sketchShapes.push({ type: 'polyline', points: [...sketchPolyPts] });
    updateShapeList();
  }
  sketchPolyPts = [];
  sketchDraft = null;
  redrawSketch();
}

window.addEventListener('keydown', (e: KeyboardEvent) => {
  if (getCurrentMode() !== 'sketch') return;
  if (e.key === 'Escape') { sketchPolyPts = []; sketchDraft = null; redrawSketch(); }
  if (e.key === 'Enter' && sketchTool === 'polyline') finishPolyline();
});

// ── Shape list ───────────────────────────────────────────────────────

// Allow constraint module to hook into shape list updates
let onShapeListUpdate: (() => void) | null = null;
export function setOnShapeListUpdate(fn: () => void): void { onShapeListUpdate = fn; }

export function updateShapeList(): void {
  const list = $('shape-list');
  const count = $('shape-count');
  count.textContent = String(sketchShapes.length);
  list.innerHTML = sketchShapes.map((s, i) => {
    let desc = '';
    if (s.type === 'line') desc = `Line (${s.p1.x},${s.p1.y}) → (${s.p2.x},${s.p2.y})`;
    else if (s.type === 'rect') desc = `Rect ${Math.abs(s.w)}×${Math.abs(s.h)} at (${Math.min(s.x, s.x + s.w)},${Math.min(s.y, s.y + s.h)})`;
    else if (s.type === 'circle') desc = `Circle r=${s.r} at (${s.cx},${s.cy})`;
    else if (s.type === 'polyline') desc = `Polyline ${s.points.length} pts`;
    return `<div>${i + 1}. ${desc}</div>`;
  }).join('');
  if (onShapeListUpdate) onShapeListUpdate();
}

// ── Undo / Clear ─────────────────────────────────────────────────────

$('sketch-undo').addEventListener('click', () => {
  sketchShapes.pop();
  updateShapeList();
  redrawSketch();
});
$('sketch-clear').addEventListener('click', () => {
  sketchShapes.length = 0;
  updateShapeList();
  redrawSketch();
});

// ── Canvas size change ───────────────────────────────────────────────

$input('canvas-size').addEventListener('change', redrawSketch);
$input('grid-snap').addEventListener('change', redrawSketch);

// ── Sketch → SVG → CAM ──────────────────────────────────────────────

export function sketchToSvg(): string {
  const size = sketchCanvasSize();
  let elements = '';
  for (const s of sketchShapes) {
    switch (s.type) {
      case 'line':
        elements += `<path d="M ${s.p1.x} ${s.p1.y} L ${s.p2.x} ${s.p2.y}"/>`;
        break;
      case 'rect': {
        const rx = Math.min(s.x, s.x + s.w), ry = Math.min(s.y, s.y + s.h);
        elements += `<rect x="${rx}" y="${ry}" width="${Math.abs(s.w)}" height="${Math.abs(s.h)}"/>`;
        break;
      }
      case 'circle':
        elements += `<circle cx="${s.cx}" cy="${s.cy}" r="${s.r}"/>`;
        break;
      case 'polyline': {
        const pts = s.points.map(p => `${p.x},${p.y}`).join(' ');
        elements += `<polyline points="${pts}"/>`;
        break;
      }
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}">${elements}</svg>`;
}
