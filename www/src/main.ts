/** Main entry point — boots WASM, wires modules together. */

import init from '../pkg/rustcam.js';
import { $ } from './dom.js';
import * as cam from './cam.js';
import { loadSim, resizeSim } from './sim.js';
import {
  sketchShapes, sketchToSvg, getCurrentMode, setCurrentMode,
  resizeSketchCanvas, redrawSketch as baseRedraw,
} from './sketch.js';
import { drawConstraintOverlay } from './constraints.js';
import { initDataflow, resizeDataflow, activateDataflow } from './dataflow/index.js';
import { initPanel, activatePanel } from './dataflow/panel-editor.js';

// ── Wire cross-module callbacks ──────────────────────────────────────

cam.setResizeSim(resizeSim);
cam.setLoadSim(loadSim);

type AppMode = 'cam' | 'sketch' | 'dataflow' | 'panel';

// ── Mode switcher ────────────────────────────────────────────────────

function setMode(mode: AppMode): void {
  setCurrentMode(mode as 'cam' | 'sketch');
  document.querySelectorAll('#mode-switcher button').forEach(b =>
    (b as HTMLElement).classList.toggle('active', (b as HTMLElement).dataset.mode === mode));
  $('cam-sidebar-content').classList.toggle('hidden', mode !== 'cam');
  $('sketch-sidebar-content').classList.toggle('hidden', mode !== 'sketch');
  $('dataflow-sidebar-content').classList.toggle('hidden', mode !== 'dataflow');
  $('panel-sidebar-content').classList.toggle('hidden', mode !== 'panel');
  document.getElementById('preview-canvas')!.classList.toggle('hidden', mode !== 'cam');
  $('preview-header').classList.toggle('hidden', mode !== 'cam');
  $('sketch-canvas-wrap').style.display = mode === 'sketch' ? 'flex' : 'none';
  const app = document.querySelector('.app')!;
  app.classList.toggle('sketch-mode', mode === 'sketch');
  app.classList.toggle('dataflow-mode', mode === 'dataflow');
  app.classList.toggle('panel-mode', mode === 'panel');
  if (mode === 'sketch') {
    // Double-rAF ensures grid layout is applied before measuring canvas
    requestAnimationFrame(() => requestAnimationFrame(() => {
      resizeSketchCanvas();
      redrawSketch();
    }));
  } else if (mode === 'dataflow') {
    activateDataflow();
  } else if (mode === 'panel') {
    activatePanel();
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
  resizeDataflow();
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
  $('filename').textContent = 'sketch.svg';
  (document.getElementById('generate-btn') as HTMLButtonElement).disabled = !cam.wasmReady;
  setMode('cam');
  cam.tryPreview();
  $('status').textContent = 'Sketch loaded — configure and generate.';
  $('status').className = 'text-xs mt-2 min-h-4 text-success';
});

// ── Boot WASM ────────────────────────────────────────────────────────

async function boot(): Promise<void> {
  try {
    await init();
    cam.setWasmReady(true);
    initDataflow();
    initPanel();
    $('status').textContent = 'WASM loaded — drop a file to begin.';
    $('status').className = 'text-xs mt-2 min-h-4 text-success';
  } catch (e) {
    $('status').textContent = 'Failed to load WASM: ' + e;
    $('status').className = 'text-xs mt-2 min-h-4 text-danger';
  }
}

boot();
