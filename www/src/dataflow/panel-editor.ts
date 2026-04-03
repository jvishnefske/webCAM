/** Panel editor: orchestrates panel mode UI. */

import { PanelManager } from './panel-manager.js';
import { renderPanel, updatePanelValues } from './panel-view.js';
import { HilClient } from './hil-client.js';
import type { WidgetKind, ChannelBinding } from './panel-types.js';

function nanOr(value: number, fallback: number): number {
  return isNaN(value) ? fallback : value;
}

let panelMgr: PanelManager | null = null;
let selectedWidgetId: number | null = null;
let hilClient: HilClient | null = null;
let syncIntervalId: ReturnType<typeof setInterval> | null = null;

// Default widget configs for the palette
const WIDGET_DEFAULTS: Record<string, { kind: WidgetKind; defaultChannels: ChannelBinding[] }> = {
  Toggle: {
    kind: { type: 'Toggle' },
    defaultChannels: [{ topic: 'topic/name', direction: 'Output', port_kind: 'Float' }],
  },
  Slider: {
    kind: { type: 'Slider', min: 0, max: 100, step: 1 },
    defaultChannels: [{ topic: 'topic/name', direction: 'Output', port_kind: 'Float' }],
  },
  Gauge: {
    kind: { type: 'Gauge', min: 0, max: 100 },
    defaultChannels: [{ topic: 'topic/name', direction: 'Input', port_kind: 'Float' }],
  },
  Label: {
    kind: { type: 'Label' },
    defaultChannels: [{ topic: 'topic/name', direction: 'Input', port_kind: 'Float' }],
  },
  Button: {
    kind: { type: 'Button' },
    defaultChannels: [{ topic: 'topic/name', direction: 'Output', port_kind: 'Float' }],
  },
  Indicator: {
    kind: { type: 'Indicator' },
    defaultChannels: [{ topic: 'topic/name', direction: 'Input', port_kind: 'Float' }],
  },
};

// Output widgets send values (accent/blue), Input widgets display values (green)
const OUTPUT_WIDGETS = new Set(['Toggle', 'Slider', 'Button']);

function getWorkspace(): HTMLDivElement {
  return document.getElementById('panel-workspace') as HTMLDivElement;
}

function rerender(): void {
  if (!panelMgr) return;
  const workspace = getWorkspace();
  renderPanel(workspace, panelMgr, onWidgetInteraction);

  // Attach click handlers to widget cards for selection
  const cards = workspace.querySelectorAll<HTMLDivElement>('[data-widget-id]');
  for (const card of cards) {
    card.style.cursor = 'pointer';
    card.addEventListener('click', (e) => {
      e.stopPropagation();
      const id = parseInt(card.dataset.widgetId!, 10);
      selectWidget(id);
    });
  }

  // Highlight selected widget
  if (selectedWidgetId !== null) {
    const sel = workspace.querySelector<HTMLDivElement>(
      `[data-widget-id="${selectedWidgetId}"]`,
    );
    if (sel) {
      sel.style.borderColor = 'var(--color-accent)';
      sel.style.borderWidth = '2px';
    }
  }
}

function onWidgetInteraction(widgetId: number, value: number | string): void {
  if (!panelMgr) return;
  const model = panelMgr.snapshot();
  const widget = model.widgets.find(w => w.id === widgetId);
  if (!widget) return;

  // Write value to all output topics bound to this widget
  const numValue = typeof value === 'string' ? parseFloat(value) || 0 : value;
  for (const ch of widget.channels) {
    if (ch.direction === 'Output') {
      panelMgr.setTopic(ch.topic, numValue);
    }
  }
}

function selectWidget(widgetId: number): void {
  selectedWidgetId = widgetId;
  rerender();
  showInspector(widgetId);
}

function clearInspector(): void {
  selectedWidgetId = null;
  const inspector = document.getElementById('panel-inspector')!;
  inspector.textContent = '';
  const hint = document.createElement('span');
  hint.className = 'text-text-dim text-[11px]';
  hint.textContent = 'Select a widget to configure';
  inspector.appendChild(hint);
}

function showInspector(widgetId: number): void {
  if (!panelMgr) return;
  const model = panelMgr.snapshot();
  const widget = model.widgets.find(w => w.id === widgetId);
  if (!widget) {
    clearInspector();
    return;
  }

  const inspector = document.getElementById('panel-inspector')!;
  inspector.textContent = '';

  // Widget label
  const labelRow = document.createElement('div');
  labelRow.className = 'mb-2';
  const labelLabel = document.createElement('label');
  labelLabel.className = 'block text-text-dim text-[11px] mb-0.5';
  labelLabel.textContent = 'Label';
  labelRow.appendChild(labelLabel);
  const labelInput = document.createElement('input');
  labelInput.type = 'text';
  labelInput.value = widget.label;
  labelInput.className = 'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
  labelRow.appendChild(labelInput);
  inspector.appendChild(labelRow);

  // Kind-specific parameters
  const kindInputs: Record<string, HTMLInputElement> = {};
  if (widget.kind.type === 'Slider') {
    for (const key of ['min', 'max', 'step'] as const) {
      const row = document.createElement('div');
      row.className = 'mb-2';
      const lab = document.createElement('label');
      lab.className = 'block text-text-dim text-[11px] mb-0.5';
      lab.textContent = key;
      row.appendChild(lab);
      const inp = document.createElement('input');
      inp.type = 'number';
      inp.step = 'any';
      inp.value = String(widget.kind[key]);
      inp.className = 'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
      row.appendChild(inp);
      inspector.appendChild(row);
      kindInputs[key] = inp;
    }
  } else if (widget.kind.type === 'Gauge') {
    for (const key of ['min', 'max'] as const) {
      const row = document.createElement('div');
      row.className = 'mb-2';
      const lab = document.createElement('label');
      lab.className = 'block text-text-dim text-[11px] mb-0.5';
      lab.textContent = key;
      row.appendChild(lab);
      const inp = document.createElement('input');
      inp.type = 'number';
      inp.step = 'any';
      inp.value = String(widget.kind[key]);
      inp.className = 'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs focus:outline-none focus:border-accent';
      row.appendChild(inp);
      inspector.appendChild(row);
      kindInputs[key] = inp;
    }
  }

  // Channel bindings
  const channelInputs: HTMLInputElement[] = [];
  if (widget.channels.length > 0) {
    const chHeader = document.createElement('div');
    chHeader.className = 'text-text-dim text-[11px] mt-2 mb-1 font-semibold';
    chHeader.textContent = 'Channels';
    inspector.appendChild(chHeader);

    for (let i = 0; i < widget.channels.length; i++) {
      const ch = widget.channels[i];
      const row = document.createElement('div');
      row.className = 'mb-2';

      const dirSpan = document.createElement('span');
      dirSpan.className = 'text-[10px] text-text-dim';
      dirSpan.textContent = `${ch.direction} (${ch.port_kind})`;
      row.appendChild(dirSpan);

      const topicInput = document.createElement('input');
      topicInput.type = 'text';
      topicInput.value = ch.topic;
      topicInput.className = 'w-full bg-bg border border-border text-text px-2 py-1 rounded text-xs mt-0.5 focus:outline-none focus:border-accent';
      row.appendChild(topicInput);
      inspector.appendChild(row);
      channelInputs.push(topicInput);
    }
  }

  // Apply button
  const applyBtn = document.createElement('button');
  applyBtn.className = 'btn btn-primary btn-sm mt-2';
  applyBtn.textContent = 'Apply';
  applyBtn.addEventListener('click', () => {
    if (!panelMgr) return;

    // Build updated kind
    let updatedKind: WidgetKind = widget.kind;
    if (widget.kind.type === 'Slider') {
      updatedKind = {
        type: 'Slider',
        min: nanOr(parseFloat(kindInputs['min'].value), 0),
        max: nanOr(parseFloat(kindInputs['max'].value), 100),
        step: nanOr(parseFloat(kindInputs['step'].value), 1),
      };
    } else if (widget.kind.type === 'Gauge') {
      updatedKind = {
        type: 'Gauge',
        min: nanOr(parseFloat(kindInputs['min'].value), 0),
        max: nanOr(parseFloat(kindInputs['max'].value), 100),
      };
    }

    // Build updated channels
    const updatedChannels: ChannelBinding[] = widget.channels.map((ch, i) => ({
      ...ch,
      topic: channelInputs[i]?.value ?? ch.topic,
    }));

    panelMgr.updateWidget(widgetId, {
      kind: updatedKind,
      label: labelInput.value,
      position: widget.position,
      size: widget.size,
      channels: updatedChannels,
    });

    rerender();
    showInspector(widgetId);
  });
  inspector.appendChild(applyBtn);

  // Delete button
  const deleteBtn = document.createElement('button');
  deleteBtn.className = 'btn btn-sm mt-2';
  deleteBtn.style.backgroundColor = 'var(--color-danger)';
  deleteBtn.style.color = 'white';
  deleteBtn.textContent = 'Delete Widget';
  deleteBtn.addEventListener('click', () => {
    if (!panelMgr) return;
    panelMgr.removeWidget(widgetId);
    clearInspector();
    rerender();
  });
  inspector.appendChild(deleteBtn);
}

function startPubsubSync(): void {
  if (syncIntervalId) return;
  syncIntervalId = setInterval(async () => {
    if (!panelMgr || !hilClient?.connected) return;
    try {
      // Pull input values from HIL
      const pubsubValues = await hilClient.getPubsub();
      panelMgr.mergeValues(pubsubValues);

      // Push output values to HIL (via HTTP, since there's no WS publish)
      // For now, outputs are stored in WASM runtime — a future POST /api/pubsub
      // endpoint on the MCU would complete the loop.
      // const outputs = panelMgr.collectOutputs();

      // Update widget displays with current values
      const allValues = panelMgr.getValues();
      const model = panelMgr.snapshot();
      const valueMap = new Map<number, number | string>();
      for (const widget of model.widgets) {
        for (const ch of widget.channels) {
          if (ch.direction === 'Input' && ch.topic in allValues) {
            valueMap.set(widget.id, allValues[ch.topic]);
          }
        }
      }
      if (valueMap.size > 0) {
        const workspace = getWorkspace();
        updatePanelValues(workspace, valueMap);
      }
    } catch {
      // Ignore transient fetch errors
    }
  }, 200); // 5Hz poll
}

function stopPubsubSync(): void {
  if (syncIntervalId) {
    clearInterval(syncIntervalId);
    syncIntervalId = null;
  }
}

function buildPalette(): void {
  const palette = document.getElementById('panel-widget-palette')!;
  palette.textContent = '';

  for (const [name, def] of Object.entries(WIDGET_DEFAULTS)) {
    const btn = document.createElement('button');
    btn.className =
      'block w-full text-left text-xs px-2 py-1.5 cursor-pointer bg-transparent border-none text-text transition-colors';
    const isOutput = OUTPUT_WIDGETS.has(name);
    btn.style.borderLeft = `3px solid ${isOutput ? 'var(--color-accent)' : 'var(--color-success)'}`;
    btn.addEventListener('mouseenter', () => {
      btn.style.background = 'var(--color-border)';
    });
    btn.addEventListener('mouseleave', () => {
      btn.style.background = 'transparent';
    });
    btn.textContent = name;
    btn.addEventListener('click', () => {
      if (!panelMgr) return;
      panelMgr.addWidget({
        kind: def.kind,
        label: name,
        position: { x: 0, y: 0 },
        size: { width: 160, height: 60 },
        channels: def.defaultChannels.map(c => ({ ...c })),
      });
      rerender();
    });
    palette.appendChild(btn);
  }
}

function loadPanel(name: string): void {
  const json = localStorage.getItem('panel:' + name);
  if (!json) return;
  const nameInput = document.getElementById('panel-name') as HTMLInputElement;
  if (panelMgr) panelMgr.destroy();
  panelMgr = PanelManager.load(json);
  nameInput.value = name;
  clearInspector();
  rerender();
  refreshPanelList();
}

function refreshPanelList(): void {
  const container = document.getElementById('panel-list');
  if (!container) return;
  container.textContent = '';

  const nameInput = document.getElementById('panel-name') as HTMLInputElement;
  const currentName = nameInput?.value.trim() ?? '';

  const panelKeys: string[] = [];
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key && key.startsWith('panel:')) {
      panelKeys.push(key.slice('panel:'.length));
    }
  }
  panelKeys.sort();

  if (panelKeys.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'text-[11px] text-text-dim px-2 py-2';
    empty.textContent = 'No saved panels';
    container.appendChild(empty);
    return;
  }

  for (const panelName of panelKeys) {
    const item = document.createElement('div');
    item.className = 'flex items-center justify-between px-2 py-1.5 text-xs cursor-pointer transition-colors';
    item.style.borderLeft = '2px solid transparent';

    if (panelName === currentName) {
      item.style.borderLeftColor = 'var(--color-accent)';
      item.style.background = 'var(--color-surface)';
    }

    item.addEventListener('mouseenter', () => {
      if (panelName !== currentName) {
        item.style.background = 'var(--color-border)';
      }
    });
    item.addEventListener('mouseleave', () => {
      if (panelName !== currentName) {
        item.style.background = 'transparent';
      }
    });

    const nameSpan = document.createElement('span');
    nameSpan.textContent = panelName;
    nameSpan.className = 'truncate flex-1';
    nameSpan.addEventListener('click', () => loadPanel(panelName));
    item.appendChild(nameSpan);

    const delBtn = document.createElement('button');
    delBtn.textContent = '\u00d7';
    delBtn.className = 'text-text-dim hover:text-danger text-sm leading-none ml-2';
    delBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      localStorage.removeItem('panel:' + panelName);
      refreshPanelList();
    });
    item.appendChild(delBtn);

    container.appendChild(item);
  }
}

export function initPanel(): void {
  const nameInput = document.getElementById('panel-name') as HTMLInputElement;
  const newBtn = document.getElementById('panel-new') as HTMLButtonElement;
  const saveBtn = document.getElementById('panel-save') as HTMLButtonElement;

  panelMgr = new PanelManager('My Panel');
  buildPalette();
  rerender();
  refreshPanelList();

  // Click on workspace background to deselect
  const workspace = getWorkspace();
  workspace.addEventListener('click', (e) => {
    if (e.target === workspace) {
      clearInspector();
      rerender();
    }
  });

  // New panel
  newBtn.addEventListener('click', () => {
    if (panelMgr) panelMgr.destroy();
    panelMgr = new PanelManager('My Panel');
    nameInput.value = 'My Panel';
    clearInspector();
    rerender();
    refreshPanelList();
  });

  // Save panel
  saveBtn.addEventListener('click', () => {
    if (!panelMgr) return;
    const name = nameInput.value.trim() || 'My Panel';
    localStorage.setItem('panel:' + name, panelMgr.save());
    refreshPanelList();

    // Brief "Saved!" feedback
    const origText = saveBtn.textContent;
    saveBtn.textContent = 'Saved!';
    setTimeout(() => { saveBtn.textContent = origText; }, 1500);
  });

  // HIL Connection
  const hilUrlInput = document.getElementById('panel-hil-url') as HTMLInputElement;
  const hilConnectBtn = document.getElementById('panel-hil-connect') as HTMLButtonElement;
  const hilStatusEl = document.getElementById('panel-hil-status')!;

  hilConnectBtn.addEventListener('click', () => {
    if (hilClient?.connected) {
      hilClient.disconnect();
      stopPubsubSync();
      return;
    }
    const url = hilUrlInput.value.trim();
    if (!url) return;

    hilClient = new HilClient();
    hilClient.onConnect = () => {
      hilStatusEl.textContent = 'Connected';
      hilStatusEl.className = 'text-[11px] text-success';
      hilConnectBtn.textContent = 'Disconnect';
      startPubsubSync();
    };
    hilClient.onDisconnect = () => {
      hilStatusEl.textContent = 'Disconnected';
      hilStatusEl.className = 'text-[11px] text-text-dim';
      hilConnectBtn.textContent = 'Connect';
      stopPubsubSync();
    };
    hilClient.onError = (msg) => {
      hilStatusEl.textContent = 'Error: ' + msg;
      hilStatusEl.className = 'text-[11px] text-danger';
    };

    hilClient.connect(url);
    hilStatusEl.textContent = 'Connecting...';
    hilStatusEl.className = 'text-[11px] text-warning';
  });

  // HIL Connection
  const hilUrlInput = document.getElementById('panel-hil-url') as HTMLInputElement;
  const hilConnectBtn = document.getElementById('panel-hil-connect') as HTMLButtonElement;
  const hilStatusEl = document.getElementById('panel-hil-status')!;

  hilConnectBtn.addEventListener('click', () => {
    if (hilClient?.connected) {
      hilClient.disconnect();
      stopPubsubSync();
      return;
    }
    const url = hilUrlInput.value.trim();
    if (!url) return;

    hilClient = new HilClient();
    hilClient.onConnect = () => {
      hilStatusEl.textContent = 'Connected';
      hilStatusEl.className = 'text-[11px] text-success';
      hilConnectBtn.textContent = 'Disconnect';
      startPubsubSync();
    };
    hilClient.onDisconnect = () => {
      hilStatusEl.textContent = 'Disconnected';
      hilStatusEl.className = 'text-[11px] text-text-dim';
      hilConnectBtn.textContent = 'Connect';
      stopPubsubSync();
    };
    hilClient.onError = (msg) => {
      hilStatusEl.textContent = 'Error: ' + msg;
      hilStatusEl.className = 'text-[11px] text-danger';
    };

    hilClient.connect(url);
    hilStatusEl.textContent = 'Connecting...';
    hilStatusEl.className = 'text-[11px] text-warning';
  });
}

export function activatePanel(): void {
  // Re-render when panel mode becomes visible
  requestAnimationFrame(() => rerender());
}
