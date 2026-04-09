/** Searchable block type picker for the dataflow editor. */

import type { BlockTypeInfo } from './types.js';
import type { DataflowManager } from './graph.js';
import { dataflow_block_defaults } from '../../pkg/rustsim.js';

/** Get default config for a block type from Rust Default impls via WASM. */
export function getBlockDefaults(blockType: string): Record<string, unknown> {
    try {
        return JSON.parse(dataflow_block_defaults(blockType));
    } catch {
        return {};
    }
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
        const config = getBlockDefaults(bt.block_type);
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
