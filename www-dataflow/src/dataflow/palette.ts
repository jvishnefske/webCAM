/** Searchable block type picker for the dataflow editor. */

import type { BlockTypeInfo, BlockConfigMap } from './types.js';
import type { DataflowManager } from './graph.js';

export const DEFAULT_CONFIGS: { [K in keyof BlockConfigMap]: BlockConfigMap[K] } = {
  constant: { value: 1.0 },
  gain: { op: 'Gain', param1: 1.0, param2: 0.0 },
  clamp: { op: 'Clamp', param1: 0.0, param2: 100.0 },
  plot: { max_samples: 500 },
  udp_source: { address: '127.0.0.1:9000' },
  udp_sink: { address: '127.0.0.1:9001' },
  adc_source: { channel: 0, resolution_bits: 12 },
  pwm_sink: { channel: 0, frequency_hz: 1000 },
  gpio_out: { pin: 13 },
  gpio_in: { pin: 2 },
  uart_tx: { port: 0, baud: 115200 },
  uart_rx: { port: 0, baud: 115200 },
  pubsub_source: { topic: 'default', port_kind: 'Float' },
  pubsub_sink: { topic: 'default', port_kind: 'Float' },
  state_machine: { states: ['idle'], initial: 'idle', transitions: [], input_topics: [], output_topics: [] },
  encoder: { channel: 0 },
  ssd1306_display: { i2c_bus: 0, address: 60 },
  tmc2209_stepper: { uart_port: 0, uart_addr: 0, steps_per_rev: 200, microsteps: 16 },
  tmc2209_stallguard: { uart_port: 0, uart_addr: 0, threshold: 50 },
};

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
        const config = (DEFAULT_CONFIGS as unknown as Record<string, Record<string, unknown>>)[bt.block_type] ?? {};
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
