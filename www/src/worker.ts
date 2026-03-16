/** Web Worker for off-thread G-code generation. */

import init, { process_stl_progress, process_svg_progress } from '../pkg/rustcam.js';

let wasmReady = false;

async function boot(): Promise<void> {
  await init();
  wasmReady = true;
  self.postMessage({ type: 'ready' });
}

boot().catch(e => {
  self.postMessage({ type: 'error', error: 'Worker WASM init failed: ' + e });
});

self.onmessage = (evt: MessageEvent) => {
  const { fileData, fileType, configJson } = evt.data;
  if (!wasmReady) {
    self.postMessage({ type: 'error', error: 'WASM not ready' });
    return;
  }

  const onProgress = (completed: number, total: number): void => {
    self.postMessage({ type: 'progress', completed, total });
  };

  try {
    let gcode: string;
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
