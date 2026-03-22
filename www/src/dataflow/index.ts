/** Dataflow mode: wires up the graph manager, editor, and plot display. */

import { dataflow_codegen, dataflow_codegen_multi } from '../../pkg/rustcam.js';
import { $, $btn, $input } from '../dom.js';
import { DataflowManager } from './graph.js';
import { DataflowEditor } from './editor.js';
import { drawPlot } from './plot.js';
import { createZip } from './zip.js';
import type { GraphSnapshot, Value } from './types.js';

let mgr: DataflowManager | null = null;
let editor: DataflowEditor | null = null;

export function initDataflow(): void {
  mgr = new DataflowManager(0.01);
  const container = $('dataflow-workspace') as HTMLDivElement;
  editor = new DataflowEditor(container, mgr);

  editor.onSelect = (blockId, snap) => {
    updateBlockInfo(blockId, snap);
  };

  editor.onEdgeSelect = (channelId, snap) => {
    updateEdgeInfo(channelId, snap);
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
    editor?.destroy();
    mgr.destroy();
    const dt = parseFloat($input('df-dt').value) || 0.01;
    mgr = new DataflowManager(dt);
    editor = new DataflowEditor(container, mgr);
    editor.onSelect = (blockId, snap) => updateBlockInfo(blockId, snap);
    editor.onEdgeSelect = (channelId, snap) => updateEdgeInfo(channelId, snap);
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

  // Export Rust crate (legacy single-target)
  $btn('df-export-rust').addEventListener('click', () => {
    if (!mgr) return;
    const statusEl = $('df-export-status');
    const dt = parseFloat($input('df-dt').value) || 0.01;
    try {
      // Check if multi-target checkboxes exist and any are checked
      const targetChecks = document.querySelectorAll<HTMLInputElement>('.df-target-check');
      if (targetChecks.length > 0) {
        const selectedTargets = Array.from(targetChecks)
          .filter(cb => cb.checked)
          .map(cb => ({
            target: cb.dataset.target!,
            binding: { target: cb.dataset.target!, pins: [] },
          }));
        if (selectedTargets.length === 0) {
          statusEl.textContent = 'Select at least one target.';
          statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
          return;
        }
        const json = dataflow_codegen_multi(
          mgr.graphId,
          dt,
          JSON.stringify(selectedTargets),
        );
        const files: Array<[string, string]> = JSON.parse(json);
        downloadAsZip(files);
        statusEl.textContent = `Exported workspace: ${files.length} files.`;
        statusEl.className = 'text-xs mt-2 min-h-4 text-success';
      } else {
        const json = dataflow_codegen(mgr.graphId, dt);
        const files: Array<[string, string]> = JSON.parse(json);
        downloadAsZip(files);
        statusEl.textContent = `Exported ${files.length} files.`;
        statusEl.className = 'text-xs mt-2 min-h-4 text-success';
      }
    } catch (e) {
      statusEl.textContent = `Export error: ${e}`;
      statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
    }
  });

  // Set up target selection checkboxes
  setupTargetCheckboxes();
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
    // Safe: static content only
    infoEl.textContent = '';
    const span = document.createElement('span');
    span.className = 'text-text-dim';
    span.textContent = 'Select a block to view details';
    infoEl.appendChild(span);
    return;
  }
  const block = snap.blocks.find(b => b.id === blockId);
  if (!block) return;

  // Build info DOM safely
  infoEl.textContent = '';
  const nameEl = document.createElement('b');
  nameEl.textContent = block.name;
  infoEl.appendChild(nameEl);
  const idSpan = document.createElement('span');
  idSpan.className = 'text-text-dim';
  idSpan.textContent = ` #${block.id}`;
  infoEl.appendChild(idSpan);
  infoEl.appendChild(document.createElement('br'));
  const typeSpan = document.createElement('span');
  typeSpan.className = 'text-text-dim text-[11px]';
  typeSpan.textContent = block.block_type;
  infoEl.appendChild(typeSpan);
  infoEl.appendChild(document.createElement('br'));

  if (block.output_values.length > 0) {
    const valDiv = document.createElement('div');
    valDiv.className = 'mt-1.5 text-xs';
    for (let i = 0; i < block.outputs.length; i++) {
      const val = block.output_values[i];
      const row = document.createElement('div');
      row.textContent = `${block.outputs[i].name}: ${formatValue(val)}`;
      valDiv.appendChild(row);
    }
    infoEl.appendChild(valDiv);
  }

  // Config section
  const configKeys = Object.keys(block.config ?? {});
  if (configKeys.length > 0) {
    const configDiv = document.createElement('div');
    configDiv.className = 'mt-2 text-xs';
    const configLabel = document.createElement('b');
    configLabel.textContent = 'Config';
    configDiv.appendChild(configLabel);

    const inputs: Record<string, HTMLInputElement> = {};
    for (const key of configKeys) {
      const row = document.createElement('div');
      row.className = 'mt-1';
      const label = document.createElement('label');
      label.className = 'block text-text-dim text-[11px]';
      label.textContent = key;
      row.appendChild(label);
      const input = document.createElement('input');
      const val = block.config[key];
      if (typeof val === 'number') {
        input.type = 'number';
        input.step = 'any';
        input.value = String(val);
      } else {
        input.type = 'text';
        input.value = String(val ?? '');
      }
      input.className = 'w-full bg-bg border border-border rounded text-text text-xs px-2 py-1 mt-0.5 outline-none';
      row.appendChild(input);
      configDiv.appendChild(row);
      inputs[key] = input;
    }

    const applyBtn = document.createElement('button');
    applyBtn.className = 'btn btn-primary btn-sm mt-2';
    applyBtn.textContent = 'Apply';
    applyBtn.addEventListener('click', () => {
      if (!mgr || !editor) return;
      const newConfig: Record<string, unknown> = {};
      for (const key of configKeys) {
        const raw = inputs[key].value;
        const origVal = block.config[key];
        newConfig[key] = typeof origVal === 'number' ? parseFloat(raw) || 0 : raw;
      }
      mgr.updateBlock(blockId, block.block_type, newConfig);
      const newSnap = mgr.snapshot();
      editor.updateSnapshot();
      updateBlockInfo(blockId, newSnap);
    });
    configDiv.appendChild(applyBtn);
    infoEl.appendChild(configDiv);
  }

  // Delete block button
  const deleteBtn = document.createElement('button');
  deleteBtn.className = 'btn btn-sm mt-2';
  deleteBtn.style.backgroundColor = 'var(--color-danger)';
  deleteBtn.style.color = 'white';
  deleteBtn.textContent = 'Delete Block';
  deleteBtn.addEventListener('click', () => {
    if (!mgr || !editor) return;
    mgr.removeBlock(blockId);
    editor.clearSelection();
    editor.updateSnapshot();
    updateBlockInfo(null, null);
  });
  infoEl.appendChild(deleteBtn);

  // If it's a plot block, update the plot canvas
  updatePlots(snap);
}

function updateEdgeInfo(channelId: number | null, snap: GraphSnapshot | null): void {
  const infoEl = $('df-block-info');
  if (channelId === null || !snap) {
    infoEl.textContent = '';
    const span = document.createElement('span');
    span.className = 'text-text-dim';
    span.textContent = 'Select a block or edge to view details';
    infoEl.appendChild(span);
    return;
  }
  const ch = snap.channels.find(c => c.id[0] === channelId);
  if (!ch) return;

  const fromBlock = snap.blocks.find(b => b.id === ch.from_block[0]);
  const toBlock = snap.blocks.find(b => b.id === ch.to_block[0]);

  infoEl.textContent = '';
  const title = document.createElement('b');
  title.textContent = 'Channel';
  infoEl.appendChild(title);
  const idSpan = document.createElement('span');
  idSpan.className = 'text-text-dim';
  idSpan.textContent = ` #${channelId}`;
  infoEl.appendChild(idSpan);
  infoEl.appendChild(document.createElement('br'));

  const detailDiv = document.createElement('div');
  detailDiv.className = 'mt-1.5 text-xs';

  const fromName = fromBlock ? `${fromBlock.name}` : `Block ${ch.from_block[0]}`;
  const fromPortName = fromBlock?.outputs[ch.from_port]?.name ?? `port ${ch.from_port}`;
  const toName = toBlock ? `${toBlock.name}` : `Block ${ch.to_block[0]}`;
  const toPortName = toBlock?.inputs[ch.to_port]?.name ?? `port ${ch.to_port}`;

  const fromRow = document.createElement('div');
  fromRow.textContent = `From: ${fromName} → ${fromPortName}`;
  detailDiv.appendChild(fromRow);

  const toRow = document.createElement('div');
  toRow.textContent = `To: ${toName} → ${toPortName}`;
  detailDiv.appendChild(toRow);

  infoEl.appendChild(detailDiv);

  const disconnectBtn = document.createElement('button');
  disconnectBtn.className = 'btn btn-sm mt-2';
  disconnectBtn.style.backgroundColor = 'var(--color-danger)';
  disconnectBtn.style.color = 'white';
  disconnectBtn.textContent = 'Disconnect';
  disconnectBtn.addEventListener('click', () => {
    if (!mgr || !editor) return;
    mgr.disconnect(channelId);
    editor.clearSelection();
    editor.updateSnapshot();
    updateEdgeInfo(null, null);
  });
  infoEl.appendChild(disconnectBtn);
}

function formatValue(val: Value | null): string {
  if (!val) return '\u2014';
  switch (val.type) {
    case 'Float': return val.data.toFixed(4);
    case 'Text': return `"${val.data.slice(0, 30)}"`;
    case 'Bytes': return `[${val.data.length} bytes]`;
    case 'Series': return `[${val.data.length} samples]`;
  }
}

const TARGET_OPTIONS = [
  { id: 'Host', label: 'Host (Simulation)', checked: true },
  { id: 'Rp2040', label: 'RP2040 (Pico)', checked: false },
  { id: 'Stm32f4', label: 'STM32F4', checked: false },
  { id: 'Esp32c3', label: 'ESP32-C3', checked: false },
] as const;

function setupTargetCheckboxes(): void {
  const container = document.getElementById('df-target-select');
  if (!container) return;

  container.textContent = '';
  const label = document.createElement('b');
  label.textContent = 'Targets';
  label.className = 'text-xs';
  container.appendChild(label);

  for (const target of TARGET_OPTIONS) {
    const row = document.createElement('label');
    row.className = 'flex items-center gap-1.5 text-xs mt-1 cursor-pointer';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = target.checked;
    cb.className = 'df-target-check';
    cb.dataset.target = target.id;
    row.appendChild(cb);
    const span = document.createElement('span');
    span.textContent = target.label;
    row.appendChild(span);
    container.appendChild(row);
  }
}

/** Download generated crate files as a ZIP archive. */
function downloadAsZip(files: Array<[string, string]>): void {
  const blob = createZip(files);
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'dataflow-generated.zip';
  a.click();
  URL.revokeObjectURL(a.href);
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
