/** Panel view: renders widgets to DOM from PanelManager snapshots. */

import type { PanelManager } from './panel-manager.js';
import type { Widget, WidgetKind } from './panel-types.js';

type InteractionCallback = (widgetId: number, value: number | string) => void;

// ── Individual widget renderers ────────────────────────────────────

function renderToggle(
  widget: Widget,
  _kind: Extract<WidgetKind, { type: 'Toggle' }>,
  onInteraction: InteractionCallback,
): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'flex items-center gap-2';

  const label = document.createElement('span');
  label.className = 'text-[13px] text-text';
  label.textContent = widget.label;
  wrapper.appendChild(label);

  const toggle = document.createElement('label');
  toggle.className = 'relative inline-flex items-center cursor-pointer';

  const input = document.createElement('input');
  input.type = 'checkbox';
  input.className = 'sr-only peer';
  input.addEventListener('change', () => {
    onInteraction(widget.id, input.checked ? 1.0 : 0.0);
    // Update visual state of the track
    track.classList.toggle('bg-accent', input.checked);
    track.classList.toggle('bg-border', !input.checked);
    dot.classList.toggle('translate-x-5', input.checked);
    dot.classList.toggle('translate-x-0', !input.checked);
  });
  toggle.appendChild(input);

  const track = document.createElement('div');
  track.className = 'w-10 h-5 bg-border rounded-full transition-colors duration-200';
  track.dataset.role = 'track';
  toggle.appendChild(track);

  const dot = document.createElement('div');
  dot.className =
    'absolute left-0.5 top-0.5 w-4 h-4 bg-text rounded-full transition-transform duration-200 translate-x-0';
  dot.dataset.role = 'dot';
  toggle.appendChild(dot);

  wrapper.appendChild(toggle);
  return wrapper;
}

function renderSlider(
  widget: Widget,
  kind: Extract<WidgetKind, { type: 'Slider' }>,
  onInteraction: InteractionCallback,
): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'flex flex-col gap-1';

  const header = document.createElement('div');
  header.className = 'flex justify-between items-center';

  const label = document.createElement('span');
  label.className = 'text-[13px] text-text';
  label.textContent = widget.label;
  header.appendChild(label);

  const valueLabel = document.createElement('span');
  valueLabel.className = 'text-[13px] text-text-dim font-mono';
  valueLabel.dataset.role = 'value';
  valueLabel.textContent = String(kind.min);
  header.appendChild(valueLabel);

  wrapper.appendChild(header);

  const input = document.createElement('input');
  input.type = 'range';
  input.min = String(kind.min);
  input.max = String(kind.max);
  input.step = String(kind.step);
  input.value = String(kind.min);
  input.className = 'w-full accent-accent';
  input.addEventListener('input', () => {
    const v = parseFloat(input.value);
    valueLabel.textContent = String(v);
    onInteraction(widget.id, v);
  });
  wrapper.appendChild(input);

  return wrapper;
}

function renderGauge(
  widget: Widget,
  kind: Extract<WidgetKind, { type: 'Gauge' }>,
): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'flex flex-col gap-1';

  const header = document.createElement('div');
  header.className = 'flex justify-between items-center';

  const label = document.createElement('span');
  label.className = 'text-[13px] text-text';
  label.textContent = widget.label;
  header.appendChild(label);

  const valueLabel = document.createElement('span');
  valueLabel.className = 'text-[13px] text-text-dim font-mono';
  valueLabel.dataset.role = 'value';
  valueLabel.textContent = String(kind.min);
  header.appendChild(valueLabel);

  wrapper.appendChild(header);

  const barBg = document.createElement('div');
  barBg.className = 'w-full h-2 bg-bg rounded-full overflow-hidden';

  const barFill = document.createElement('div');
  barFill.className = 'h-full bg-accent rounded-full transition-all duration-150';
  barFill.dataset.role = 'bar';
  barFill.dataset.min = String(kind.min);
  barFill.dataset.max = String(kind.max);
  barFill.style.width = '0%';
  barBg.appendChild(barFill);

  wrapper.appendChild(barBg);

  return wrapper;
}

function renderLabel(widget: Widget): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'flex flex-col gap-0.5';

  const label = document.createElement('span');
  label.className = 'text-[11px] uppercase tracking-wider text-text-dim';
  label.textContent = widget.label;
  wrapper.appendChild(label);

  const value = document.createElement('span');
  value.className = 'text-[15px] text-text font-mono';
  value.dataset.role = 'value';
  value.textContent = '\u2014'; // em-dash placeholder
  wrapper.appendChild(value);

  return wrapper;
}

function renderButton(
  widget: Widget,
  _kind: Extract<WidgetKind, { type: 'Button' }>,
  onInteraction: InteractionCallback,
): HTMLDivElement {
  const wrapper = document.createElement('div');

  const btn = document.createElement('button');
  btn.className =
    'bg-accent text-white px-4 py-1.5 rounded text-[13px] font-semibold ' +
    'cursor-pointer transition-colors duration-150 hover:bg-accent-dim active:opacity-80';
  btn.textContent = widget.label;
  btn.addEventListener('pointerdown', () => {
    onInteraction(widget.id, 1.0);
  });
  btn.addEventListener('pointerup', () => {
    onInteraction(widget.id, 0.0);
  });
  btn.addEventListener('pointerleave', () => {
    onInteraction(widget.id, 0.0);
  });
  wrapper.appendChild(btn);

  return wrapper;
}

function renderIndicator(widget: Widget): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'flex items-center gap-2';

  const dot = document.createElement('div');
  dot.className =
    'w-3 h-3 rounded-full bg-border transition-colors duration-150';
  dot.dataset.role = 'indicator';
  wrapper.appendChild(dot);

  const label = document.createElement('span');
  label.className = 'text-[13px] text-text';
  label.textContent = widget.label;
  wrapper.appendChild(label);

  return wrapper;
}

// ── Main render entry point ────────────────────────────────────────

/**
 * Render the full panel into a container div.
 * Clears existing content and rebuilds from a PanelManager snapshot.
 */
export function renderPanel(
  container: HTMLDivElement,
  mgr: PanelManager,
  onWidgetInteraction: InteractionCallback,
): void {
  const model = mgr.snapshot();
  container.textContent = '';

  const title = document.createElement('h2');
  title.className = 'text-[13px] uppercase tracking-wider text-text-dim mb-3';
  title.textContent = model.name;
  container.appendChild(title);

  for (const widget of model.widgets) {
    const card = document.createElement('div');
    card.className =
      'bg-surface border border-border rounded-lg p-3 mb-2';
    card.dataset.widgetId = String(widget.id);
    card.style.minWidth = `${widget.size.width}px`;
    card.style.minHeight = `${widget.size.height}px`;

    let content: HTMLDivElement;
    switch (widget.kind.type) {
      case 'Toggle':
        content = renderToggle(widget, widget.kind, onWidgetInteraction);
        break;
      case 'Slider':
        content = renderSlider(widget, widget.kind, onWidgetInteraction);
        break;
      case 'Gauge':
        content = renderGauge(widget, widget.kind);
        break;
      case 'Label':
        content = renderLabel(widget);
        break;
      case 'Button':
        content = renderButton(widget, widget.kind, onWidgetInteraction);
        break;
      case 'Indicator':
        content = renderIndicator(widget);
        break;
    }

    card.appendChild(content);
    container.appendChild(card);
  }
}

// ── Live value update (no full re-render) ──────────────────────────

/**
 * Update displayed values for widgets without rebuilding the DOM.
 * Keys in the map are widget ids; values are the new display value.
 */
export function updatePanelValues(
  container: HTMLDivElement,
  values: Map<number, number | string>,
): void {
  for (const [widgetId, value] of values) {
    const card = container.querySelector<HTMLDivElement>(
      `[data-widget-id="${widgetId}"]`,
    );
    if (!card) continue;

    // Update value label (slider, gauge, label)
    const valueEl = card.querySelector<HTMLElement>('[data-role="value"]');
    if (valueEl) {
      valueEl.textContent = typeof value === 'number'
        ? String(Math.round(value * 1000) / 1000)
        : String(value);
    }

    // Update gauge bar fill
    const barEl = card.querySelector<HTMLDivElement>('[data-role="bar"]');
    if (barEl && typeof value === 'number') {
      const min = parseFloat(barEl.dataset.min ?? '0');
      const max = parseFloat(barEl.dataset.max ?? '100');
      const pct = Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100));
      barEl.style.width = `${pct}%`;
    }

    // Update indicator dot
    const indicatorEl = card.querySelector<HTMLDivElement>('[data-role="indicator"]');
    if (indicatorEl && typeof value === 'number') {
      const lit = value > 0.5;
      indicatorEl.classList.toggle('bg-accent', lit);
      indicatorEl.classList.toggle('bg-border', !lit);
    }

    // Update toggle state
    const checkbox = card.querySelector<HTMLInputElement>('input[type="checkbox"]');
    if (checkbox && typeof value === 'number') {
      const checked = value > 0.5;
      if (checkbox.checked !== checked) {
        checkbox.checked = checked;
        // Sync visual track/dot
        const track = card.querySelector<HTMLDivElement>('[data-role="track"]');
        const dot = card.querySelector<HTMLDivElement>('[data-role="dot"]');
        if (track) {
          track.classList.toggle('bg-accent', checked);
          track.classList.toggle('bg-border', !checked);
        }
        if (dot) {
          dot.classList.toggle('translate-x-5', checked);
          dot.classList.toggle('translate-x-0', !checked);
        }
      }
    }

    // Update slider position
    const slider = card.querySelector<HTMLInputElement>('input[type="range"]');
    if (slider && typeof value === 'number') {
      slider.value = String(value);
    }
  }
}
