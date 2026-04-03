/** Panel manager: wraps WASM panel API, manages widget CRUD. */

import {
  panel_new, panel_destroy, panel_load, panel_save,
  panel_add_widget, panel_remove_widget, panel_update_widget, panel_snapshot,
} from '../../pkg/rustcam.js';
import type { PanelModel, Widget } from './panel-types.js';

/**
 * WASM wrapper for the control panel model.
 *
 * Methods that interact with the WASM backend may throw if the panel ID
 * is invalid or JSON is malformed.  Callers should handle exceptions.
 */
export class PanelManager {
  panelId: number;

  constructor(name: string) {
    this.panelId = panel_new(name);
  }

  destroy(): void {
    panel_destroy(this.panelId);
  }

  /** Deserialize a PanelModel from JSON and wrap it. */
  static load(json: string): PanelManager {
    const mgr = Object.create(PanelManager.prototype) as PanelManager;
    mgr.panelId = panel_load(json);
    return mgr;
  }

  /** Serialize the panel to JSON. */
  save(): string {
    return panel_save(this.panelId);
  }

  /** Add a widget (id field is ignored, assigned by WASM). Returns assigned id. */
  addWidget(widget: Omit<Widget, 'id'>): number {
    const w = { id: 0, ...widget };
    return panel_add_widget(this.panelId, JSON.stringify(w));
  }

  /** Remove a widget by id. Returns true if it existed. */
  removeWidget(widgetId: number): boolean {
    return panel_remove_widget(this.panelId, widgetId);
  }

  /** Replace a widget's config in-place (id is preserved by WASM). */
  updateWidget(widgetId: number, widget: Omit<Widget, 'id'>): void {
    const w = { id: widgetId, ...widget };
    panel_update_widget(this.panelId, widgetId, JSON.stringify(w));
  }

  /** Get a live snapshot of the panel model. */
  snapshot(): PanelModel {
    return JSON.parse(panel_snapshot(this.panelId));
  }
}
