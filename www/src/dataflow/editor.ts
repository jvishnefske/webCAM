/** DOM/SVG dataflow editor — workspace orchestrator. */

import { DataflowManager } from './graph.js';
import type { GraphSnapshot, BlockTypeInfo } from './types.js';
import { reconcileNodes, setupNodeDrag, setupNodeDelete, type NodeElements } from './node-view.js';
import { reconcileEdges, type EdgeElements } from './edge-view.js';
import { setupWireDrag } from './port-view.js';
import { showPalette } from './palette.js';

export class DataflowEditor {
  private container: HTMLDivElement;
  private workspace: HTMLDivElement;
  private grid: HTMLDivElement;
  private svg: SVGSVGElement;
  private nodeLayer: HTMLDivElement;
  private mgr: DataflowManager;
  private snap: GraphSnapshot | null = null;
  private selected: number | null = null;
  private selectedEdge: number | null = null;
  private panX = 0;
  private panY = 0;
  private scale = 1;
  private blockTypes: BlockTypeInfo[];
  private nodeElements: NodeElements = { nodes: new Map() };
  private edgeElements: EdgeElements = { paths: new Map() };
  private cleanupFns: Array<() => void> = [];

  /** Fires when block selection changes. */
  onSelect: ((blockId: number | null, snap: GraphSnapshot | null) => void) | null = null;
  /** Fires when edge selection changes. */
  onEdgeSelect: ((channelId: number | null, snap: GraphSnapshot | null) => void) | null = null;

  constructor(container: HTMLDivElement, mgr: DataflowManager) {
    this.container = container;
    this.mgr = mgr;
    this.blockTypes = DataflowManager.blockTypes();

    // Build DOM structure
    this.workspace = container;
    this.workspace.textContent = '';
    this.workspace.classList.add('df-workspace');

    this.grid = document.createElement('div');
    this.grid.className = 'df-grid';
    this.workspace.appendChild(this.grid);

    this.svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.svg.classList.add('df-edge-layer');
    this.svg.setAttribute('width', '100%');
    this.svg.setAttribute('height', '100%');
    this.workspace.appendChild(this.svg);

    this.nodeLayer = document.createElement('div');
    this.nodeLayer.className = 'df-node-layer';
    this.workspace.appendChild(this.nodeLayer);

    // Wire up pan/zoom
    this.setupPanZoom();

    // Wire up node drag
    const cleanupDrag = setupNodeDrag(
      this.workspace, this.nodeLayer, mgr, this.edgeElements,
      () => this.snap,
      () => ({ panX: this.panX, panY: this.panY, scale: this.scale }),
      (blockId) => {
        this.selected = blockId;
        this.selectedEdge = null;
        this.reconcile();
        this.onSelect?.(blockId, this.snap);
      },
      () => { /* drag end — edges already updated during drag */ },
    );
    this.cleanupFns.push(cleanupDrag);

    // Wire up node delete
    const cleanupDelete = setupNodeDelete(
      this.nodeLayer, mgr,
      () => this.selected,
      (deletedId) => {
        if (this.selected === deletedId) {
          this.selected = null;
          this.onSelect?.(null, this.snap);
        }
        this.snap = mgr.snapshot();
        this.reconcile();
      },
    );
    this.cleanupFns.push(cleanupDelete);

    // Wire up wire drag (port-to-port connections)
    const cleanupWire = setupWireDrag(
      this.workspace, this.nodeLayer, this.svg, mgr,
      () => this.snap,
      () => ({ panX: this.panX, panY: this.panY, scale: this.scale }),
      () => {
        this.snap = mgr.snapshot();
        this.reconcile();
      },
    );
    this.cleanupFns.push(cleanupWire);

    // Wire up double-click palette
    this.setupDblClick();

    // Wire up edge click (event delegation)
    this.setupEdgeClick();

    // Wire up keyboard delete
    this.setupKeyDelete();

    // Manager tick callback
    mgr.onTick = (snap) => {
      this.snap = snap;
      this.reconcile();
    };

    // Initial render
    this.snap = mgr.snapshot();
    this.reconcile();
  }

  resize(): void {
    // The workspace is sized by CSS (flex/grid), no manual sizing needed.
    // Just re-render in case layout changed.
    this.applyTransform();
  }

  updateSnapshot(): void {
    this.snap = this.mgr.snapshot();
    this.reconcile();
  }

  clearSelection(): void {
    this.selected = null;
    this.selectedEdge = null;
    this.reconcile();
  }

  destroy(): void {
    for (const fn of this.cleanupFns) fn();
    this.cleanupFns = [];
    this.mgr.onTick = null;
    this.workspace.removeEventListener('wheel', this.onWheel);
    this.workspace.removeEventListener('pointerdown', this.onPanStart);
    this.workspace.removeEventListener('dblclick', this.onDblClick);
    this.svg.removeEventListener('click', this.onEdgeClick);
    this.workspace.removeEventListener('keydown', this.onKeyDelete);
    this.workspace.textContent = '';
    this.nodeElements.nodes.clear();
    this.edgeElements.paths.clear();
  }

  private reconcile(): void {
    if (!this.snap) return;
    reconcileNodes(this.nodeLayer, this.nodeElements, this.snap.blocks, this.mgr.positions, this.selected);
    reconcileEdges(this.svg, this.edgeElements, this.snap.channels, this.snap.blocks, this.mgr.positions, this.selectedEdge);

    // Update time display
    const timeInfo = document.getElementById('df-time-info');
    if (timeInfo) timeInfo.textContent = `t=${this.snap.time.toFixed(3)}`;
  }

  private applyTransform(): void {
    const transform = `translate(${this.panX}px, ${this.panY}px) scale(${this.scale})`;
    this.nodeLayer.style.transform = transform;
    this.nodeLayer.style.transformOrigin = '0 0';
    this.svg.style.transform = transform;
    this.svg.style.transformOrigin = '0 0';

    // Update grid to match
    const gridSize = 20 * this.scale;
    this.grid.style.backgroundSize = `${gridSize}px ${gridSize}px`;
    this.grid.style.backgroundPosition = `${this.panX}px ${this.panY}px`;
  }

  // ── Edge click ──────────────────────────────────────────────────

  private onEdgeClick = (e: MouseEvent): void => {
    const target = e.target as Element;
    const chAttr = target instanceof SVGElement ? target.dataset.ch : undefined;
    if (!chAttr) return;
    const channelId = parseInt(chAttr, 10);
    if (isNaN(channelId)) return;
    this.selectedEdge = channelId;
    this.selected = null;
    this.reconcile();
    this.onEdgeSelect?.(channelId, this.snap);
  };

  private setupEdgeClick(): void {
    this.svg.addEventListener('click', this.onEdgeClick);
  }

  // ── Keyboard delete ────────────────────────────────────────────

  private onKeyDelete = (e: KeyboardEvent): void => {
    if (e.key !== 'Delete' && e.key !== 'Backspace') return;
    // Don't intercept when focused on an input
    const tag = (e.target as HTMLElement).tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

    if (this.selectedEdge !== null) {
      this.mgr.disconnect(this.selectedEdge);
      this.selectedEdge = null;
      this.snap = this.mgr.snapshot();
      this.reconcile();
      this.onEdgeSelect?.(null, null);
    } else if (this.selected !== null) {
      const id = this.selected;
      this.mgr.removeBlock(id);
      this.selected = null;
      this.snap = this.mgr.snapshot();
      this.reconcile();
      this.onSelect?.(null, null);
    }
  };

  private setupKeyDelete(): void {
    this.workspace.setAttribute('tabindex', '0');
    this.workspace.style.outline = 'none';
    this.workspace.addEventListener('keydown', this.onKeyDelete);
  }

  // ── Pan/Zoom ─────────────────────────────────────────────────────

  private isPanning = false;
  private panStartX = 0;
  private panStartY = 0;
  private panBaseX = 0;
  private panBaseY = 0;

  private setupPanZoom(): void {
    this.workspace.addEventListener('wheel', this.onWheel, { passive: false });
    this.workspace.addEventListener('pointerdown', this.onPanStart);
  }

  private onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    const rect = this.workspace.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;

    const oldScale = this.scale;
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    this.scale = Math.max(0.2, Math.min(3.0, this.scale * delta));

    // Preserve focal point under cursor
    this.panX = mx - (mx - this.panX) * (this.scale / oldScale);
    this.panY = my - (my - this.panY) * (this.scale / oldScale);

    this.applyTransform();
  };

  private onPanStart = (e: PointerEvent): void => {
    // Middle mouse button or shift+left
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
      // Don't pan if clicking on a node/port
      const target = e.target as HTMLElement;
      if (target.closest('.df-node') || target.classList.contains('df-port')) return;

      this.isPanning = true;
      this.panStartX = e.clientX;
      this.panStartY = e.clientY;
      this.panBaseX = this.panX;
      this.panBaseY = this.panY;
      this.workspace.style.cursor = 'grabbing';
      e.preventDefault();

      const onMove = (ev: PointerEvent): void => {
        if (!this.isPanning) return;
        this.panX = this.panBaseX + (ev.clientX - this.panStartX);
        this.panY = this.panBaseY + (ev.clientY - this.panStartY);
        this.applyTransform();
      };

      const onUp = (): void => {
        this.isPanning = false;
        this.workspace.style.cursor = '';
        window.removeEventListener('pointermove', onMove);
        window.removeEventListener('pointerup', onUp);
      };

      window.addEventListener('pointermove', onMove);
      window.addEventListener('pointerup', onUp);
    }
  };

  // ── Double-click palette ─────────────────────────────────────────

  private onDblClick = (e: MouseEvent): void => {
    const target = e.target as HTMLElement;
    if (target.closest('.df-node') || target.closest('.df-palette')) return;

    const rect = this.workspace.getBoundingClientRect();
    const worldX = (e.clientX - rect.left - this.panX) / this.scale;
    const worldY = (e.clientY - rect.top - this.panY) / this.scale;

    showPalette(
      this.workspace, this.blockTypes, this.mgr,
      e.clientX, e.clientY,
      worldX, worldY,
      () => {
        this.snap = this.mgr.snapshot();
        this.reconcile();
      },
    );
  };

  private setupDblClick(): void {
    this.workspace.addEventListener('dblclick', this.onDblClick);
  }
}
