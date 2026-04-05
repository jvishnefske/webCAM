/** Main entry point — boots WASM, wires dataflow + panel modules together. */

import init from '../pkg/rustsim.js';
import { $ } from './dom.js';
import { initDataflow, resizeDataflow, activateDataflow } from './dataflow/index.js';
import { initPanel, activatePanel } from './dataflow/panel-editor.js';

type AppMode = 'dataflow' | 'panel';

function setMode(mode: AppMode): void {
  document.querySelectorAll('#mode-switcher button').forEach(b =>
    (b as HTMLElement).classList.toggle('active', (b as HTMLElement).dataset.mode === mode));
  $('dataflow-sidebar-content').classList.toggle('hidden', mode !== 'dataflow');
  $('panel-sidebar-content').classList.toggle('hidden', mode !== 'panel');
  const app = document.querySelector('.app')!;
  app.classList.toggle('dataflow-mode', mode === 'dataflow');
  app.classList.toggle('panel-mode', mode === 'panel');
  if (mode === 'dataflow') {
    activateDataflow();
  } else if (mode === 'panel') {
    activatePanel();
  }
}

document.querySelectorAll('#mode-switcher button').forEach(btn =>
  btn.addEventListener('click', () => setMode((btn as HTMLElement).dataset.mode as AppMode)));

window.addEventListener('resize', () => {
  resizeDataflow();
});

// ── Boot ─────────────────────────────────────────────────────────────

init({ module_or_path: '/pkg/rustsim_bg.wasm' }).then(() => {
  initDataflow();
  initPanel();
  setMode('dataflow');
}).catch(console.error);
