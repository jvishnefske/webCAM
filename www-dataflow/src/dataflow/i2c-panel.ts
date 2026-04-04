/**
 * I2C bus/device CRUD panel.
 * Shows bus cards with devices, supports add/remove/edit operations.
 */

import type { HilClient, BusEntry } from './hil-client.js';

/**
 * Render the I2C management panel into `container`.
 * Call this again whenever the bus list updates.
 */
export function renderI2cPanel(container: HTMLElement, client: HilClient): void {
  container.textContent = '';

  // Top controls
  const controls = document.createElement('div');
  controls.className = 'i2c-controls';

  const busCountLabel = document.createElement('label');
  busCountLabel.className = 'text-xs text-text-dim';
  busCountLabel.textContent = 'Bus count: ';
  const busCountInput = document.createElement('input');
  busCountInput.type = 'number';
  busCountInput.min = '1';
  busCountInput.max = '16';
  busCountInput.value = '1';
  busCountInput.className = 'i2c-input i2c-input-sm';
  const setBusBtn = document.createElement('button');
  setBusBtn.className = 'btn btn-secondary btn-sm';
  setBusBtn.textContent = 'Set';
  setBusBtn.addEventListener('click', () => {
    client.setBusCount(parseInt(busCountInput.value) || 1);
  });

  const clearBtn = document.createElement('button');
  clearBtn.className = 'btn btn-sm';
  clearBtn.style.backgroundColor = 'var(--color-danger)';
  clearBtn.style.color = 'white';
  clearBtn.textContent = 'Clear All';
  clearBtn.addEventListener('click', () => {
    client.clearAll();
  });

  controls.appendChild(busCountLabel);
  controls.appendChild(busCountInput);
  controls.appendChild(setBusBtn);
  controls.appendChild(clearBtn);
  container.appendChild(controls);

  // Bus cards container
  const cardGrid = document.createElement('div');
  cardGrid.className = 'i2c-bus-grid';
  container.appendChild(cardGrid);

  // Add device form
  const form = document.createElement('div');
  form.className = 'i2c-form';
  const formTitle = document.createElement('div');
  formTitle.className = 'text-xs font-semibold mb-1';
  formTitle.textContent = 'Add Device';
  form.appendChild(formTitle);

  const busInput = createField(form, 'Bus', 'number', '0');
  const addrInput = createField(form, 'Addr (hex)', 'text', '0x50');
  const nameInput = createField(form, 'Name', 'text', 'eeprom');
  const regInput = createField(form, 'Registers', 'number', '256');

  const addBtn = document.createElement('button');
  addBtn.className = 'btn btn-primary btn-sm';
  addBtn.textContent = 'Add Device';
  addBtn.addEventListener('click', () => {
    const bus = parseInt(busInput.value) || 0;
    const addr = parseInt(addrInput.value, 16) || parseInt(addrInput.value) || 0;
    const name = nameInput.value.trim() || 'device';
    const regs = parseInt(regInput.value) || 256;
    client.addDevice(bus, addr, name, regs);
  });
  form.appendChild(addBtn);
  container.appendChild(form);

  // Store ref for updates
  (container as HTMLElement & { _cardGrid?: HTMLDivElement })._cardGrid = cardGrid;
}

/**
 * Update the bus card grid with fresh data.
 */
export function updateI2cBuses(
  container: HTMLElement,
  buses: BusEntry[],
  client: HilClient,
): void {
  const cardGrid = (container as HTMLElement & { _cardGrid?: HTMLDivElement })._cardGrid;
  if (!cardGrid) return;
  cardGrid.textContent = '';

  if (buses.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'text-xs text-text-dim p-2';
    empty.textContent = 'No buses configured.';
    cardGrid.appendChild(empty);
    return;
  }

  for (const bus of buses) {
    const card = document.createElement('div');
    card.className = 'i2c-bus-card';

    const header = document.createElement('div');
    header.className = 'i2c-bus-header';
    header.textContent = `Bus ${bus.busIdx}`;
    const count = document.createElement('span');
    count.className = 'text-text-dim';
    count.textContent = ` (${bus.devices.length} devices)`;
    header.appendChild(count);
    card.appendChild(header);

    for (const dev of bus.devices) {
      const row = document.createElement('div');
      row.className = 'i2c-device-row';

      const addrSpan = document.createElement('span');
      addrSpan.className = 'i2c-device-addr';
      addrSpan.textContent = `0x${dev.addr.toString(16).padStart(2, '0')}`;
      row.appendChild(addrSpan);

      const nameSpan = document.createElement('span');
      nameSpan.className = 'i2c-device-name';
      nameSpan.textContent = dev.name;
      row.appendChild(nameSpan);

      const removeBtn = document.createElement('button');
      removeBtn.className = 'btn btn-sm i2c-remove-btn';
      removeBtn.textContent = '×';
      removeBtn.title = 'Remove device';
      removeBtn.addEventListener('click', () => {
        client.removeDevice(bus.busIdx, dev.addr);
      });
      row.appendChild(removeBtn);

      card.appendChild(row);
    }

    cardGrid.appendChild(card);
  }
}

function createField(parent: HTMLElement, label: string, type: string, value: string): HTMLInputElement {
  const row = document.createElement('div');
  row.className = 'i2c-form-row';
  const lbl = document.createElement('label');
  lbl.className = 'text-[11px] text-text-dim';
  lbl.textContent = label;
  row.appendChild(lbl);
  const input = document.createElement('input');
  input.type = type;
  input.value = value;
  input.className = 'i2c-input';
  row.appendChild(input);
  parent.appendChild(row);
  return input;
}
