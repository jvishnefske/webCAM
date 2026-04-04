/** Main entry point — boots WASM, wires CAM + sketch modules together. */

import init from '../pkg/rustcam.js';
import { $ } from './dom.js';
import * as cam from './cam.js';
import { loadSim, resizeSim } from './sim.js';
import {
  sketchShapes, sketchToSvg, getCurrentMode, setCurrentMode,
  resizeSketchCanvas, redrawSketch as baseRedraw,
} from './sketch.js';
import { drawConstraintOverlay } from './constraints.js';

// ── Wire cross-module callbacks ──────────────────────────────────────

cam.setResizeSim(resizeSim);
cam.setLoadSim(loadSim);

type AppMode = 'cam' | 'sketch';

// ── Mode switcher ────────────────────────────────────────────────────

function setMode(mode: AppMode): void {
  setCurrentMode(mode);
  document.querySelectorAll('#mode-switcher button').forEach(b =>
    (b as HTMLElement).classList.toggle('active', (b as HTMLElement).dataset.mode === mode));
  $('cam-sidebar-content').classList.toggle('hidden', mode !== 'cam');
  $('sketch-sidebar-content').classList.toggle('hidden', mode !== 'sketch');
  document.getElementById('preview-canvas')!.classList.toggle('hidden', mode !== 'cam');
  $('preview-header').classList.toggle('hidden', mode !== 'cam');
  $('sketch-canvas-wrap').style.display = mode === 'sketch' ? 'flex' : 'none';
  const app = document.querySelector('.app')!;
  app.classList.toggle('sketch-mode', mode === 'sketch');
  if (mode === 'sketch') {
    requestAnimationFrame(() => requestAnimationFrame(() => {
      resizeSketchCanvas();
      redrawSketch();
    }));
  } else {
    cam.tryPreview();
  }
}

document.querySelectorAll('#mode-switcher button').forEach(btn =>
  btn.addEventListener('click', () => setMode((btn as HTMLElement).dataset.mode as AppMode)));

// ── Patched redraw that includes constraint overlay ──────────────────

function redrawSketch(): void {
  baseRedraw();
  drawConstraintOverlay();
}

window.addEventListener('resize', () => {
  cam.tryPreview();
  resizeSim();
  if (getCurrentMode() === 'sketch') { resizeSketchCanvas(); redrawSketch(); }
});

new ResizeObserver(() => {
  if (getCurrentMode() === 'sketch') { resizeSketchCanvas(); redrawSketch(); }
}).observe($('sketch-canvas-wrap'));

// ── Sketch → CAM ────────────────────────────────────────────────────

$('sketch-to-cam').addEventListener('click', () => {
  if (sketchShapes.length === 0) {
    $('sketch-status').textContent = 'Draw at least one shape first.';
    $('sketch-status').className = 'text-xs mt-2 min-h-4 text-danger';
    return;
  }
  const svgText = sketchToSvg();
  cam.setFileData(svgText, 'svg');
  setMode('cam');
});

// ── Boot ─────────────────────────────────────────────────────────────

init().then(() => {
  cam.setWasmReady(true);
  setMode('cam');
}).catch(console.error);
