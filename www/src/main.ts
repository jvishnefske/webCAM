/** Main entry point — boots WASM, wires modules together. */

import init from '../pkg/rustcam.js';
import { $, $canvas } from './dom.js';
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

// ── Mode switcher ────────────────────────────────────────────────────

function setMode(mode: 'cam' | 'sketch'): void {
  setCurrentMode(mode);
  document.querySelectorAll('#mode-switcher button').forEach(b =>
    (b as HTMLElement).classList.toggle('active', (b as HTMLElement).dataset.mode === mode));
  $('cam-sidebar-content').style.display = mode === 'cam' ? 'block' : 'none';
  $('sketch-sidebar-content').style.display = mode === 'sketch' ? 'block' : 'none';
  $canvas('preview-canvas').style.display = mode === 'cam' ? 'block' : 'none';
  $('preview-header').style.display = mode === 'cam' ? 'block' : 'none';
  $('sketch-canvas-wrap').style.display = mode === 'sketch' ? 'flex' : 'none';
  document.querySelector('.app')!.classList.toggle('sketch-mode', mode === 'sketch');
  if (mode === 'sketch') {
    requestAnimationFrame(() => { resizeSketchCanvas(); redrawSketch(); });
  } else {
    cam.tryPreview();
  }
}

document.querySelectorAll('#mode-switcher button').forEach(btn =>
  btn.addEventListener('click', () => setMode((btn as HTMLElement).dataset.mode as 'cam' | 'sketch')));

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
    $('sketch-status').className = 'status error';
    return;
  }
  const svgText = sketchToSvg();
  cam.setFileData(svgText, 'svg');
  $('filename').textContent = 'sketch.svg';
  (document.getElementById('generate-btn') as HTMLButtonElement).disabled = !cam.wasmReady;
  setMode('cam');
  cam.tryPreview();
  $('status').textContent = 'Sketch loaded — configure and generate.';
  $('status').className = 'status ok';
});

// ── Boot WASM ────────────────────────────────────────────────────────

async function boot(): Promise<void> {
  try {
    await init();
    cam.setWasmReady(true);
    $('status').textContent = 'WASM loaded — drop a file to begin.';
    $('status').className = 'status ok';
  } catch (e) {
    $('status').textContent = 'Failed to load WASM: ' + e;
    $('status').className = 'status error';
  }
}

boot();
