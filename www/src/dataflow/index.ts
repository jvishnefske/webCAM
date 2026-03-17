/** Dataflow mode: wires up the graph manager, editor, and plot display. */

import { $, $canvas, $btn, $input } from '../dom.js';
import { DataflowManager } from './graph.js';
import { DataflowEditor } from './editor.js';
import { drawPlot } from './plot.js';
import type { GraphSnapshot, Value } from './types.js';

let mgr: DataflowManager | null = null;
let editor: DataflowEditor | null = null;

export function initDataflow(): void {
  mgr = new DataflowManager(0.01);
  const canvas = $canvas('dataflow-canvas');
  editor = new DataflowEditor(canvas, mgr);

  editor.onSelect = (blockId, snap) => {
    updateBlockInfo(blockId, snap);
  };

  // Transport controls
  $btn('df-play').addEventListener('click', () => {
    if (!mgr) return;
    if (mgr.running) {
      mgr.stop();
      $btn('df-play').textContent = 'Play';
    } else {
      mgr.start();
      $btn('df-play').textContent = 'Pause';
    }
  });

  $btn('df-reset').addEventListener('click', () => {
    if (!mgr) return;
    mgr.stop();
    $btn('df-play').textContent = 'Play';
    mgr.destroy();
    const dt = parseFloat($input('df-dt').value) || 0.01;
    mgr = new DataflowManager(dt);
    editor = new DataflowEditor(canvas, mgr);
    editor.onSelect = (blockId, snap) => updateBlockInfo(blockId, snap);
    editor.resize();
  });

  $input('df-speed').addEventListener('input', () => {
    if (!mgr) return;
    mgr.setSpeed(parseFloat($input('df-speed').value) || 1.0);
  });

  $btn('df-batch').addEventListener('click', () => {
    if (!mgr) return;
    const steps = parseInt($input('df-batch-steps').value) || 100;
    const dt = parseFloat($input('df-dt').value) || 0.01;
    const snap = mgr.runBatch(steps, dt);
    updatePlots(snap);
    editor?.updateSnapshot();
  });
}

export function resizeDataflow(): void {
  editor?.resize();
}

export function activateDataflow(): void {
  requestAnimationFrame(() => editor?.resize());
}

function updateBlockInfo(blockId: number | null, snap: GraphSnapshot | null): void {
  const infoEl = $('df-block-info');
  if (blockId === null || !snap) {
    infoEl.innerHTML = '<span style="color:#8888a0">Select a block to view details</span>';
    return;
  }
  const block = snap.blocks.find(b => b.id === blockId);
  if (!block) return;

  let html = `<b>${block.name}</b> <span style="color:#8888a0">#${block.id}</span><br>`;
  html += `<span style="color:#8888a0;font-size:11px">${block.block_type}</span><br>`;

  if (block.output_values.length > 0) {
    html += '<div style="margin-top:6px;font-size:12px">';
    for (let i = 0; i < block.outputs.length; i++) {
      const val = block.output_values[i];
      html += `<div>${block.outputs[i].name}: ${formatValue(val)}</div>`;
    }
    html += '</div>';
  }

  infoEl.innerHTML = html;

  // If it's a plot block, update the plot canvas
  updatePlots(snap);
}

function formatValue(val: Value | null): string {
  if (!val) return '<span style="color:#8888a0">—</span>';
  switch (val.type) {
    case 'Float': return val.data.toFixed(4);
    case 'Text': return `"${val.data.slice(0, 30)}"`;
    case 'Bytes': return `[${val.data.length} bytes]`;
    case 'Series': return `[${val.data.length} samples]`;
  }
}

function updatePlots(snap: GraphSnapshot): void {
  const plotCanvas = document.getElementById('df-plot-canvas') as HTMLCanvasElement | null;
  if (!plotCanvas) return;

  // Find the first plot block with series data
  for (const block of snap.blocks) {
    if (block.block_type === 'plot') {
      const val = block.output_values[0];
      if (val && val.type === 'Series') {
        drawPlot(plotCanvas, val.data, `Plot #${block.id}`);
        return;
      }
    }
  }
}
