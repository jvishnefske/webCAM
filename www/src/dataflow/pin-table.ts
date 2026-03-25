/**
 * 16-column pin table renderer.
 * Displays up to 256 pins in a 16×16 grid with direction badges.
 */

import type { PinEntry } from './hil-client.js';

const COLS = 16;

export type PinEditCallback = (index: number, pin: PinEntry) => void;

/**
 * Render a compact pin table into `container`.
 * Calls `onEdit` when a pin's direction is toggled or name is edited.
 */
export function renderPinTable(
  container: HTMLElement,
  pins: PinEntry[],
  onEdit: PinEditCallback,
): void {
  container.textContent = '';

  if (pins.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'text-xs text-text-dim p-2';
    empty.textContent = 'No pin configuration received.';
    container.appendChild(empty);
    return;
  }

  const table = document.createElement('table');
  table.className = 'pin-table';

  // Header row with column indices
  const thead = document.createElement('tr');
  const corner = document.createElement('th');
  corner.textContent = '';
  thead.appendChild(corner);
  for (let c = 0; c < COLS; c++) {
    const th = document.createElement('th');
    th.textContent = String(c);
    thead.appendChild(th);
  }
  table.appendChild(thead);

  // Show last 16 rows (most recent 256 pins)
  const totalRows = Math.ceil(pins.length / COLS);
  const startRow = Math.max(0, totalRows - 16);

  for (let r = startRow; r < totalRows; r++) {
    const tr = document.createElement('tr');
    const rowLabel = document.createElement('th');
    rowLabel.textContent = String(r);
    tr.appendChild(rowLabel);

    for (let c = 0; c < COLS; c++) {
      const idx = r * COLS + c;
      const td = document.createElement('td');
      td.className = 'pin-cell';

      if (idx < pins.length) {
        const pin = pins[idx];
        td.classList.add(pin.direction === 'I' ? 'pin-cell-input' : 'pin-cell-output');

        const nameSpan = document.createElement('span');
        nameSpan.className = 'pin-name';
        nameSpan.textContent = pin.name || String(idx);
        nameSpan.title = `Pin ${idx}: ${pin.name}`;
        td.appendChild(nameSpan);

        const badge = document.createElement('span');
        badge.className = 'pin-badge';
        badge.textContent = `[${pin.direction}]`;
        td.appendChild(badge);

        // Click to toggle direction
        td.addEventListener('click', () => {
          const newDir = pin.direction === 'I' ? 'O' : 'I';
          onEdit(idx, { name: pin.name, direction: newDir });
        });

        // Double-click to edit name inline
        td.addEventListener('dblclick', (e) => {
          e.stopPropagation();
          const input = document.createElement('input');
          input.type = 'text';
          input.value = pin.name;
          input.className = 'pin-inline-edit';
          td.textContent = '';
          td.appendChild(input);
          input.focus();
          input.select();

          const commit = () => {
            const newName = input.value.trim() || pin.name;
            onEdit(idx, { name: newName, direction: pin.direction });
          };
          input.addEventListener('blur', commit);
          input.addEventListener('keydown', (ke) => {
            if (ke.key === 'Enter') input.blur();
            if (ke.key === 'Escape') {
              input.value = pin.name;
              input.blur();
            }
          });
        });
      }

      tr.appendChild(td);
    }
    table.appendChild(tr);
  }

  container.appendChild(table);
}
