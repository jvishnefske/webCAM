// Web Worker for G-code generation.
// Runs WASM off the main thread so the UI stays responsive and can
// display a live progress counter.

import init, {
  process_stl_progress, process_svg_progress,
} from './pkg/rustcam.js';

let wasmReady = false;

async function boot() {
  await init();
  wasmReady = true;
  self.postMessage({ type: 'ready' });
}

boot().catch(e => {
  self.postMessage({ type: 'error', error: 'Worker WASM init failed: ' + e });
});

self.onmessage = (evt) => {
  const { fileData, fileType, configJson } = evt.data;
  if (!wasmReady) {
    self.postMessage({ type: 'error', error: 'WASM not ready' });
    return;
  }

  const onProgress = (completed, total) => {
    self.postMessage({ type: 'progress', completed, total });
  };

  try {
    let gcode;
    if (fileType === 'stl') {
      gcode = process_stl_progress(fileData, configJson, onProgress);
    } else {
      gcode = process_svg_progress(fileData, configJson, onProgress);
    }
    self.postMessage({ type: 'done', gcode });
  } catch (e) {
    self.postMessage({ type: 'error', error: String(e) });
  }
};
