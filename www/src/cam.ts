/** CAM pipeline UI: file handling, config, preview, G-code generation. */

import {
  process_stl, process_svg,
  preview_stl, preview_svg,
} from '../pkg/rustcam.js';
import { $, $input, $select, $canvas, $textarea, $btn } from './dom.js';
import type { CamConfig, WorkerOutMsg } from './types.js';
import { theme } from './theme.js';

// ── State ────────────────────────────────────────────────────────────

export let wasmReady = false;
export let fileData: Uint8Array | string | null = null;
export let fileType: 'stl' | 'svg' | null = null;

export function setWasmReady(v: boolean): void { wasmReady = v; }
export function setFileData(d: Uint8Array | string | null, t: 'stl' | 'svg' | null): void {
  fileData = d;
  fileType = t;
}

// ── DOM refs ─────────────────────────────────────────────────────────

const dropZone    = $('drop-zone');
const filenameEl  = $('filename');
const generateBtn = $btn('generate-btn');
const statusEl    = $('status');
const gcodeOut    = $textarea('gcode-output');
const canvas      = $canvas('preview-canvas');
const copyBtn     = $btn('copy-btn');
const downloadBtn = $btn('download-btn');

const toolTypeSelect        = $select('tool-type');
const effectiveDiameterRow  = $('effective-diameter-row');
const cornerRadiusRow       = $('corner-radius-row');
const machineTypeSelect     = $select('machine-type');
const cncParamsSection      = $('cnc-params');
const laserParamsSection    = $('laser-params');
const strategySelect        = $select('strategy');
const perimeterOptions      = $('perimeter-options');
const zigzagOptions         = $('zigzag-options');
const fileInput             = $input('file-input');

const cncStrategies   = ['contour', 'pocket', 'slice', 'zigzag', 'perimeter'];
const laserStrategies = ['contour', 'pocket', 'perimeter', 'laser_cut', 'laser_engrave'];

// ── Tool type UI ─────────────────────────────────────────────────────

function updateToolTypeUI(): void {
  const toolType = toolTypeSelect.value;
  effectiveDiameterRow.classList.toggle('hidden', toolType !== 'face_mill');
  cornerRadiusRow.classList.toggle('hidden', toolType !== 'ball_end');
  if (toolType === 'ball_end') {
    const diameter = parseFloat($input('tool-diameter').value);
    $input('corner-radius').value = (diameter / 2).toFixed(2);
  }
}

toolTypeSelect.addEventListener('change', updateToolTypeUI);
$input('tool-diameter').addEventListener('change', updateToolTypeUI);

// ── Machine type UI ──────────────────────────────────────────────────

function updateStrategyUI(): void {
  const strategy = strategySelect.value;
  perimeterOptions.classList.toggle('hidden', strategy !== 'perimeter');
  zigzagOptions.classList.toggle('hidden', strategy !== 'zigzag');
}

function updateMachineTypeUI(): void {
  const isLaser = machineTypeSelect.value === 'laser_cutter';
  cncParamsSection.classList.toggle('hidden', isLaser);
  laserParamsSection.classList.toggle('hidden', !isLaser);
  const current = strategySelect.value;
  const allowed = isLaser ? laserStrategies : cncStrategies;
  for (const opt of Array.from(strategySelect.options)) {
    (opt as HTMLOptionElement).hidden = !allowed.includes(opt.value);
  }
  if (!allowed.includes(current)) {
    strategySelect.value = isLaser ? 'laser_cut' : 'contour';
  }
  updateStrategyUI();
}

machineTypeSelect.addEventListener('change', () => { updateMachineTypeUI(); tryPreview(); });
strategySelect.addEventListener('change', () => { updateStrategyUI(); tryPreview(); });
$select('scan-direction').addEventListener('change', tryPreview);
updateMachineTypeUI();

// ── Tabs ─────────────────────────────────────────────────────────────

export let resizeSimFn: () => void = () => {};
export function setResizeSim(fn: () => void): void { resizeSimFn = fn; }

document.querySelectorAll('.tab-bar button').forEach(btn => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.tab-bar button').forEach(b => b.classList.remove('active'));
    document.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'));
    (btn as HTMLElement).classList.add('active');
    $((btn as HTMLElement).dataset.tab!).classList.add('active');
    if ((btn as HTMLElement).dataset.tab === 'sim-tab') resizeSimFn();
  });
});

// ── File handling ────────────────────────────────────────────────────

dropZone.addEventListener('click', () => fileInput.click());
dropZone.addEventListener('dragover', e => { e.preventDefault(); dropZone.classList.add('drag-over'); });
dropZone.addEventListener('dragleave', () => dropZone.classList.remove('drag-over'));
dropZone.addEventListener('drop', e => {
  e.preventDefault();
  dropZone.classList.remove('drag-over');
  if (e.dataTransfer?.files.length) handleFile(e.dataTransfer.files[0]);
});
fileInput.addEventListener('change', () => { if (fileInput.files?.length) handleFile(fileInput.files[0]); });

function handleFile(file: File): void {
  const ext = file.name.split('.').pop()?.toLowerCase();
  if (ext === 'stl') {
    fileType = 'stl';
    file.arrayBuffer().then(buf => {
      fileData = new Uint8Array(buf);
      filenameEl.textContent = file.name;
      generateBtn.disabled = !wasmReady;
      tryPreview();
    });
  } else if (ext === 'svg') {
    fileType = 'svg';
    file.text().then(txt => {
      fileData = txt;
      filenameEl.textContent = file.name;
      generateBtn.disabled = !wasmReady;
      tryPreview();
    });
  } else {
    statusEl.textContent = 'Unsupported file type: .' + ext;
    statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
  }
}

// ── Config ───────────────────────────────────────────────────────────

export function getConfig(): string {
  const toolType = $select('tool-type').value;
  const isLaser = machineTypeSelect.value === 'laser_cutter';
  const config: CamConfig = {
    tool_diameter:  parseFloat($input('tool-diameter').value),
    step_over:      parseFloat($input('step-over').value),
    strategy:       strategySelect.value,
    tool_type:      toolType,
    machine_type:   machineTypeSelect.value,
  };

  if (isLaser) {
    config.feed_rate = parseFloat($input('laser-feed-rate').value);
    config.plunge_rate = config.feed_rate;
    config.spindle_speed = 0;
    config.safe_z = 0;
    config.cut_depth = 0;
    config.step_down = 1;
    config.laser_power = parseFloat($input('laser-power').value);
    config.passes = parseInt($input('laser-passes').value) || 1;
    config.air_assist = ($input('air-assist') as HTMLInputElement).checked;
  } else {
    config.step_down = parseFloat($input('step-down').value);
    config.feed_rate = parseFloat($input('feed-rate').value);
    config.plunge_rate = parseFloat($input('plunge-rate').value);
    config.spindle_speed = parseFloat($input('spindle-speed').value);
    config.safe_z = parseFloat($input('safe-z').value);
    config.cut_depth = parseFloat($input('cut-depth').value);
  }
  if (toolType === 'ball_end') {
    config.corner_radius = parseFloat($input('corner-radius').value) || 0;
  } else if (toolType === 'face_mill') {
    config.effective_diameter = parseFloat($input('effective-diameter').value) || config.tool_diameter;
  }
  if (config.strategy === 'zigzag') {
    config.scan_direction = $select('scan-direction').value;
  }
  if (config.strategy === 'perimeter') {
    config.climb_cut = ($input('climb-cut') as HTMLInputElement).checked;
    config.perimeter_passes = parseInt($input('perimeter-passes').value) || 1;
  }
  return JSON.stringify(config);
}

// ── Preview ──────────────────────────────────────────────────────────

export function tryPreview(): void {
  if (!wasmReady || !fileData) return;
  try {
    let json: string;
    if (fileType === 'stl') json = preview_stl(fileData as Uint8Array, getConfig());
    else json = preview_svg(fileData as string);
    drawPreview(JSON.parse(json));
  } catch (e) { console.warn('Preview error:', e); }
}

function drawPreview(paths: number[][][]): void {
  const ctx = canvas.getContext('2d')!;
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width  = rect.width  * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, rect.width, rect.height);
  if (!paths.length) return;

  const has3D = paths.some(p => p.length > 0 && p[0].length >= 3);

  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  let minZ = Infinity, maxZ = -Infinity;
  for (const p of paths) for (const pt of p) {
    const [x, y, z] = pt;
    if (x < minX) minX = x; if (y < minY) minY = y;
    if (x > maxX) maxX = x; if (y > maxY) maxY = y;
    if (has3D && z !== undefined) {
      if (z < minZ) minZ = z; if (z > maxZ) maxZ = z;
    }
  }
  let w = maxX - minX, h = maxY - minY;
  if (w === 0 && h === 0) { w = 1; h = 1; }
  else if (w === 0) { w = h; minX -= w / 2; maxX += w / 2; }
  else if (h === 0) { h = w; minY -= h / 2; maxY += h / 2; }
  const zRange = (maxZ - minZ) || 1;
  const pad = Math.max(8, Math.min(30, Math.min(rect.width, rect.height) * 0.08));
  const availW = Math.max(rect.width - pad * 2, 1);
  const availH = Math.max(rect.height - pad * 2, 1);
  const scale = Math.min(availW / w, availH / h);
  const offX = pad + (availW - w * scale) / 2;
  const offY = pad + (availH - h * scale) / 2;
  const tx = (x: number) => offX + (x - minX) * scale;
  const ty = (y: number) => offY + (maxY - y) * scale;

  const isLaserPreview = machineTypeSelect.value === 'laser_cutter';

  const zColor = (z?: number): string => {
    if (isLaserPreview) return theme.colors.camLaser;
    if (!has3D || z === undefined) return theme.colors.camZDefault;
    const t = (z - minZ) / zRange;
    const hue = 240 - t * 180;
    return `hsl(${hue}, 80%, 55%)`;
  };

  ctx.strokeStyle = theme.colors.surface; ctx.lineWidth = 0.5;
  const gs = Math.pow(10, Math.floor(Math.log10(Math.max(w, h))));
  for (let x = Math.floor(minX / gs) * gs; x <= maxX; x += gs) {
    ctx.beginPath(); ctx.moveTo(tx(x), 0); ctx.lineTo(tx(x), rect.height); ctx.stroke();
  }
  for (let y = Math.floor(minY / gs) * gs; y <= maxY; y += gs) {
    ctx.beginPath(); ctx.moveTo(0, ty(y)); ctx.lineTo(rect.width, ty(y)); ctx.stroke();
  }

  ctx.lineWidth = 1.2; ctx.lineJoin = 'round';
  for (const p of paths) {
    if (p.length < 2) continue;
    if (has3D) {
      for (let i = 1; i < p.length; i++) {
        ctx.strokeStyle = zColor(p[i][2]);
        ctx.beginPath();
        ctx.moveTo(tx(p[i - 1][0]), ty(p[i - 1][1]));
        ctx.lineTo(tx(p[i][0]), ty(p[i][1]));
        ctx.stroke();
      }
    } else {
      ctx.strokeStyle = isLaserPreview ? theme.colors.camLaser : theme.colors.camZDefault;
      ctx.beginPath();
      ctx.moveTo(tx(p[0][0]), ty(p[0][1]));
      for (let i = 1; i < p.length; i++) ctx.lineTo(tx(p[i][0]), ty(p[i][1]));
      ctx.stroke();
    }
  }
}

// ── Generate (Web Worker) ────────────────────────────────────────────

let genWorker: Worker | null = null;
let genStartTime = 0;
let loadSimFn: () => void = () => {};

export function setLoadSim(fn: () => void): void { loadSimFn = fn; }

function initWorker(): void {
  genWorker = new Worker('./dist/worker.js', { type: 'module' });
  genWorker.onmessage = (evt: MessageEvent<WorkerOutMsg>) => {
    const msg = evt.data;
    if (msg.type === 'progress') {
      const elapsed = ((performance.now() - genStartTime) / 1000).toFixed(1);
      statusEl.textContent = `Generating... layer ${msg.completed} / ${msg.total}  (${elapsed}s)`;
      statusEl.className = 'text-xs mt-2 min-h-4';
    } else if (msg.type === 'done') {
      gcodeOut.value = msg.gcode;
      const elapsed = ((performance.now() - genStartTime) / 1000).toFixed(1);
      statusEl.textContent = `Done — ${msg.gcode.split('\n').length} lines of G-code in ${elapsed}s.`;
      statusEl.className = 'text-xs mt-2 min-h-4 text-success';
      generateBtn.disabled = false;
      tryPreview();
      loadSimFn();
    } else if (msg.type === 'error') {
      statusEl.textContent = 'Error: ' + msg.error;
      statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
      generateBtn.disabled = false;
    }
  };
  genWorker.onerror = (e) => {
    statusEl.textContent = 'Worker error: ' + e.message;
    statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
    generateBtn.disabled = false;
  };
}

let workerSupported = true;
try {
  initWorker();
} catch {
  workerSupported = false;
  console.warn('Module workers not supported, using main-thread generation');
}

generateBtn.addEventListener('click', () => {
  if (!wasmReady || !fileData) return;
  generateBtn.disabled = true;
  statusEl.textContent = 'Generating... layer 0 / ?';
  statusEl.className = 'text-xs mt-2 min-h-4';
  genStartTime = performance.now();

  const cfg = getConfig();

  if (workerSupported && genWorker) {
    genWorker.postMessage({ fileData, fileType, configJson: cfg });
  } else {
    try {
      let gcode: string;
      if (fileType === 'stl') gcode = process_stl(fileData as Uint8Array, cfg);
      else gcode = process_svg(fileData as string, cfg);
      gcodeOut.value = gcode;
      const elapsed = ((performance.now() - genStartTime) / 1000).toFixed(1);
      statusEl.textContent = `Done — ${gcode.split('\n').length} lines of G-code in ${elapsed}s.`;
      statusEl.className = 'text-xs mt-2 min-h-4 text-success';
      tryPreview();
      loadSimFn();
    } catch (e) {
      statusEl.textContent = 'Error: ' + e;
      statusEl.className = 'text-xs mt-2 min-h-4 text-danger';
    }
    generateBtn.disabled = false;
  }
});

// ── Copy / Download ──────────────────────────────────────────────────

copyBtn.addEventListener('click', () => {
  navigator.clipboard.writeText(gcodeOut.value).then(() => {
    copyBtn.textContent = 'Copied!';
    setTimeout(() => copyBtn.textContent = 'Copy', 1500);
  });
});
downloadBtn.addEventListener('click', () => {
  const blob = new Blob([gcodeOut.value], { type: 'text/plain' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = (filenameEl.textContent || 'output').replace(/\.\w+$/, '') + '.nc';
  a.click();
  URL.revokeObjectURL(a.href);
});

// ── Exports for main ─────────────────────────────────────────────────

export { statusEl, filenameEl, generateBtn };
