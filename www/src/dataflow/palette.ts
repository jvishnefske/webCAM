/** Searchable block type picker for the dataflow editor. */

import type { BlockTypeInfo, FunctionDef } from './types.js';
import type { DataflowManager } from './graph.js';

/** Build default configs from function def param defaults.
 *  Falls back to legacy hardcoded configs for block types not in the registry. */
export function buildDefaultConfigs(defs: FunctionDef[]): Record<string, Record<string, unknown>> {
  const configs: Record<string, Record<string, unknown>> = {};
  for (const def of defs) {
    const cfg: Record<string, unknown> = {};
    for (const p of def.params) {
      switch (p.kind) {
        case 'Float': cfg[p.name] = parseFloat(p.default) || 0; break;
        case 'Int': cfg[p.name] = parseInt(p.default, 10) || 0; break;
        case 'Bool': cfg[p.name] = p.default === 'true'; break;
        case 'String': cfg[p.name] = p.default; break;
      }
    }
    configs[def.id] = cfg;
  }
  return configs;
}

/** Legacy defaults for block types not yet in the FunctionDef registry. */
const LEGACY_CONFIGS: Record<string, Record<string, unknown>> = {
  udp_source: { address: '127.0.0.1:9000' },
  udp_sink: { address: '127.0.0.1:9001' },
  adc_source: { channel: 0, resolution_bits: 12 },
  pwm_sink: { channel: 0, frequency_hz: 1000 },
  gpio_out: { pin: 13 },
  gpio_in: { pin: 2 },
  uart_tx: { port: 0, baud: 115200 },
  uart_rx: { port: 0, baud: 115200 },
  register: { initial_value: 0 },
  state_machine: { states: ['idle'], initial: 'idle', transitions: [], input_topics: [], output_topics: [] },
  encoder: { channel: 0 },
  ssd1306_display: { i2c_bus: 0, address: 60 },
  tmc2209_stepper: { uart_port: 0, uart_addr: 0, steps_per_rev: 200, microsteps: 16 },
  tmc2209_stallguard: { uart_port: 0, uart_addr: 0, threshold: 50 },
};

/** Merged default configs: schema-driven from WASM + legacy fallbacks. */
export let DEFAULT_CONFIGS: Record<string, Record<string, unknown>> = { ...LEGACY_CONFIGS };

/** Initialize DEFAULT_CONFIGS from WASM function defs.
 *  Call this once after WASM is loaded. */
export function initDefaultConfigs(defs: FunctionDef[]): void {
  DEFAULT_CONFIGS = { ...LEGACY_CONFIGS, ...buildDefaultConfigs(defs) };
}

/** Show palette at screen position, create block at world position. */
export function showPalette(
  workspace: HTMLDivElement,
  blockTypes: BlockTypeInfo[],
  mgr: DataflowManager,
  screenX: number,
  screenY: number,
  worldX: number,
  worldY: number,
  onBlockAdded: () => void,
): void {
  // Remove existing palette
  workspace.querySelector('.df-palette')?.remove();

  const palette = document.createElement('div');
  palette.className = 'df-palette';
  palette.style.left = `${screenX}px`;
  palette.style.top = `${screenY}px`;

  // Search input
  const search = document.createElement('input');
  search.className = 'df-palette-search';
  search.type = 'text';
  search.placeholder = 'Search blocks...';
  palette.appendChild(search);

  const listContainer = document.createElement('div');
  palette.appendChild(listContainer);

  function renderList(filter: string): void {
    listContainer.textContent = '';
    const lowerFilter = filter.toLowerCase();
    let lastCat = '';

    for (const bt of blockTypes) {
      if (filter && !bt.name.toLowerCase().includes(lowerFilter) && !bt.block_type.toLowerCase().includes(lowerFilter)) {
        continue;
      }
      if (bt.category !== lastCat) {
        lastCat = bt.category;
        const header = document.createElement('div');
        header.className = 'df-palette-category';
        header.textContent = bt.category;
        listContainer.appendChild(header);
      }
      const item = document.createElement('div');
      item.className = 'df-palette-item';
      item.textContent = bt.name;
      item.addEventListener('click', () => {
        const config = DEFAULT_CONFIGS[bt.block_type] ?? {};
        mgr.addBlock(bt.block_type, config, worldX, worldY);
        palette.remove();
        onBlockAdded();
      });
      listContainer.appendChild(item);
    }
  }

  renderList('');

  search.addEventListener('input', () => renderList(search.value));
  search.addEventListener('keydown', (e) => {
    e.stopPropagation(); // Prevent workspace shortcuts
    if (e.key === 'Escape') palette.remove();
    if (e.key === 'Enter') {
      const firstItem = listContainer.querySelector('.df-palette-item') as HTMLElement | null;
      firstItem?.click();
    }
  });

  // Position: use fixed positioning relative to workspace
  const wsRect = workspace.getBoundingClientRect();
  palette.style.left = `${screenX - wsRect.left}px`;
  palette.style.top = `${screenY - wsRect.top}px`;
  palette.style.position = 'absolute';
  palette.style.zIndex = '100';
  workspace.appendChild(palette);

  // Focus search
  requestAnimationFrame(() => search.focus());

  // Dismiss on click-outside
  const dismiss = (ev: MouseEvent) => {
    if (!palette.contains(ev.target as Node)) {
      palette.remove();
      document.removeEventListener('mousedown', dismiss);
    }
  };
  setTimeout(() => document.addEventListener('mousedown', dismiss), 0);
}
