/** Tool simulation module. */

import { sim_moves_stl, sim_moves_svg } from '../pkg/rustcam.js';
import { $, $input, $canvas, $btn } from './dom.js';
import { fileData, fileType, getConfig } from './cam.js';
import type { SimMove, SimBounds } from './types.js';

const simCanvas = $canvas('sim-canvas');
const simPlay   = $btn('sim-play');
const simReset  = $btn('sim-reset');
const simSpeed  = $input('sim-speed');
const simScrub  = $input('sim-scrub');
const simInfo   = $('sim-info');

let simMoves: SimMove[] = [];
let simIdx = 0;
let simRunning = false;
let simRaf: number | null = null;
let simBounds: SimBounds | null = null;
let simScale = 1;
let simOffX = 0;
let simOffY = 0;

let matCanvas: HTMLCanvasElement | null = null;
let matCtx: CanvasRenderingContext2D | null = null;

export function loadSim(): void {
  try {
    const cfg = getConfig();
    let json: string;
    if (fileType === 'stl') json = sim_moves_stl(fileData as Uint8Array, cfg);
    else json = sim_moves_svg(fileData as string, cfg);
    simMoves = JSON.parse(json);
  } catch (e) { simMoves = []; console.warn('sim_moves error:', e); }
  simIdx = 0;
  (simScrub as HTMLInputElement).max = String(Math.max(simMoves.length - 1, 1));
  simScrub.value = '0';
  simInfo.textContent = `0 / ${simMoves.length}`;
  simRunning = false;
  simPlay.textContent = 'Play';
  computeSimBounds();
  initMatCanvas();
  drawSimFrame();
}

function computeSimBounds(): void {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const m of simMoves) {
    if (m.x < minX) minX = m.x; if (m.y < minY) minY = m.y;
    if (m.x > maxX) maxX = m.x; if (m.y > maxY) maxY = m.y;
  }
  if (!simMoves.length) { minX = 0; minY = 0; maxX = 10; maxY = 10; }
  let w = maxX - minX, h = maxY - minY;
  if (w === 0 && h === 0) { w = 1; h = 1; }
  else if (w === 0) { w = h; minX -= w / 2; maxX += w / 2; }
  else if (h === 0) { h = w; minY -= h / 2; maxY += h / 2; }
  simBounds = { minX, minY, maxX, maxY, w, h };
}

export function resizeSim(): void {
  if (!simBounds) return;
  const rect = simCanvas.getBoundingClientRect();
  if (rect.width < 1 || rect.height < 1) return;
  const dpr = window.devicePixelRatio || 1;
  simCanvas.width = rect.width * dpr;
  simCanvas.height = rect.height * dpr;
  const pad = Math.max(8, Math.min(40, Math.min(rect.width, rect.height) * 0.08));
  const availW = Math.max(rect.width - pad * 2, 1);
  const availH = Math.max(rect.height - pad * 2, 1);
  simScale = Math.min(availW / simBounds.w, availH / simBounds.h);
  simOffX = pad + (availW - simBounds.w * simScale) / 2;
  simOffY = pad + (availH - simBounds.h * simScale) / 2;
  initMatCanvas();
  replayMat(simIdx);
  drawSimFrame();
}

function initMatCanvas(): void {
  const rect = simCanvas.getBoundingClientRect();
  if (rect.width < 1) return;
  const dpr = window.devicePixelRatio || 1;
  matCanvas = document.createElement('canvas');
  matCanvas.width = rect.width * dpr;
  matCanvas.height = rect.height * dpr;
  matCtx = matCanvas.getContext('2d');
}

function simTx(x: number): number { return simOffX + (x - simBounds!.minX) * simScale; }
function simTy(y: number): number { return simOffY + (simBounds!.maxY - y) * simScale; }

function stampMat(ax: number, ay: number, bx: number, by: number, toolR: number): void {
  if (!matCtx) return;
  const dpr = window.devicePixelRatio || 1;
  matCtx.save();
  matCtx.scale(dpr, dpr);
  matCtx.strokeStyle = 'rgba(255,80,80,0.35)';
  matCtx.lineWidth = toolR * simScale * 2;
  matCtx.lineCap = 'round';
  matCtx.beginPath();
  matCtx.moveTo(simTx(ax), simTy(ay));
  matCtx.lineTo(simTx(bx), simTy(by));
  matCtx.stroke();
  matCtx.restore();
}

function replayMat(n: number): void {
  if (!matCtx || !matCanvas) return;
  matCtx.clearRect(0, 0, matCanvas.width, matCanvas.height);
  const toolR = parseFloat($input('tool-diameter').value) / 2;
  const safeZ = parseFloat($input('safe-z').value);
  for (let i = 1; i <= n && i < simMoves.length; i++) {
    const prev = simMoves[i - 1];
    const cur = simMoves[i];
    if (!cur.rapid && cur.z < safeZ - 0.01) {
      stampMat(prev.x, prev.y, cur.x, cur.y, toolR);
    }
  }
}

function drawSimFrame(): void {
  const ctx = simCanvas.getContext('2d')!;
  const dpr = window.devicePixelRatio || 1;
  const rect = simCanvas.getBoundingClientRect();
  if (rect.width < 1) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, rect.width, rect.height);

  if (simBounds) {
    ctx.strokeStyle = '#2a2d3a';
    ctx.lineWidth = 1;
    ctx.strokeRect(
      simTx(simBounds.minX) - 4, simTy(simBounds.maxY) - 4,
      simBounds.w * simScale + 8, simBounds.h * simScale + 8,
    );
  }

  if (matCanvas) ctx.drawImage(matCanvas, 0, 0, matCanvas.width / dpr, matCanvas.height / dpr);

  if (simMoves.length > 1 && simIdx > 0) {
    ctx.lineWidth = 0.8;
    ctx.lineJoin = 'round';
    for (let i = 1; i <= simIdx && i < simMoves.length; i++) {
      const prev = simMoves[i - 1];
      const cur = simMoves[i];
      ctx.strokeStyle = cur.rapid ? 'rgba(255,255,100,0.25)' : 'rgba(79,140,255,0.6)';
      ctx.beginPath();
      ctx.moveTo(simTx(prev.x), simTy(prev.y));
      ctx.lineTo(simTx(cur.x), simTy(cur.y));
      ctx.stroke();
    }
  }

  if (simIdx < simMoves.length) {
    const m = simMoves[simIdx];
    const toolR = parseFloat($input('tool-diameter').value) / 2;
    const r = toolR * simScale;
    const cx = simTx(m.x), cy = simTy(m.y);
    ctx.beginPath(); ctx.arc(cx, cy, r + 2, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(0,0,0,0.3)'; ctx.fill();
    ctx.beginPath(); ctx.arc(cx, cy, r, 0, Math.PI * 2);
    const cutting = !m.rapid && m.z < parseFloat($input('safe-z').value) - 0.01;
    ctx.fillStyle = cutting ? 'rgba(255,80,80,0.7)' : 'rgba(100,200,100,0.5)';
    ctx.strokeStyle = cutting ? '#ff5555' : '#55ff88';
    ctx.lineWidth = 1.5; ctx.fill(); ctx.stroke();
    ctx.beginPath(); ctx.arc(cx, cy, 2, 0, Math.PI * 2);
    ctx.fillStyle = '#fff'; ctx.fill();
    ctx.fillStyle = '#8888a0'; ctx.font = '11px monospace';
    ctx.fillText(`Z${m.z.toFixed(2)}`, cx + r + 6, cy + 4);
  }

  ctx.restore();
  simInfo.textContent = `${simIdx} / ${simMoves.length}`;
}

// ── Transport controls ───────────────────────────────────────────────

simPlay.addEventListener('click', () => {
  if (!simMoves.length) return;
  simRunning = !simRunning;
  simPlay.textContent = simRunning ? 'Pause' : 'Play';
  if (simRunning) simTick();
});

simReset.addEventListener('click', () => {
  simRunning = false;
  simPlay.textContent = 'Play';
  if (simRaf) cancelAnimationFrame(simRaf);
  simIdx = 0;
  simScrub.value = '0';
  initMatCanvas();
  drawSimFrame();
});

simScrub.addEventListener('input', () => {
  simIdx = parseInt(simScrub.value);
  replayMat(simIdx);
  drawSimFrame();
});

function simTick(): void {
  if (!simRunning) return;
  const speed = parseInt(simSpeed.value);
  const safeZ = parseFloat($input('safe-z').value);
  const toolR = parseFloat($input('tool-diameter').value) / 2;
  const steps = Math.max(1, Math.round(speed / 10));
  for (let s = 0; s < steps; s++) {
    if (simIdx >= simMoves.length - 1) {
      simRunning = false;
      simPlay.textContent = 'Play';
      break;
    }
    simIdx++;
    const prev = simMoves[simIdx - 1];
    const cur = simMoves[simIdx];
    if (!cur.rapid && cur.z < safeZ - 0.01) {
      stampMat(prev.x, prev.y, cur.x, cur.y, toolR);
    }
  }
  simScrub.value = String(simIdx);
  drawSimFrame();
  if (simRunning) simRaf = requestAnimationFrame(simTick);
}
