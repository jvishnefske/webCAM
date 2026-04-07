/** Create/update/remove node DOM elements, drag handling. */

import type { BlockSnapshot, NodePosition, GraphSnapshot } from './types.js';
import type { DataflowManager } from './graph.js';
import { createPorts, updateOutputLabels } from './port-view.js';
import { updateEdgesForBlock, type EdgeElements } from './edge-view.js';
import { mountStateMachineEditor } from './state-machine-editor.js';

const PORT_SPACING = 20;
const NODE_H_BASE = 40;

function nodeHeight(block: BlockSnapshot): number {
  const ports = Math.max(block.inputs.length, block.outputs.length);
  return NODE_H_BASE + Math.max(ports, 1) * PORT_SPACING;
}

export interface NodeElements {
  /** Map from block id → node DOM element */
  nodes: Map<number, HTMLDivElement>;
}

/** Reconcile node DOM elements against current snapshot. */
export function reconcileNodes(
  nodeLayer: HTMLDivElement,
  elements: NodeElements,
  blocks: BlockSnapshot[],
  positions: Map<number, NodePosition>,
  selectedId: number | null,
): void {
  const currentIds = new Set<number>();

  for (const block of blocks) {
    currentIds.add(block.id);
    let nodeEl = elements.nodes.get(block.id);

    if (!nodeEl) {
      // Create new node
      nodeEl = document.createElement('div');
      nodeEl.className = 'df-node';
      nodeEl.dataset.id = String(block.id);

      const header = document.createElement('div');
      header.className = 'df-node-header';
      header.textContent = block.name;
      nodeEl.appendChild(header);

      const typeLabel = document.createElement('span');
      typeLabel.className = 'df-node-type';
      typeLabel.textContent = block.block_type;
      nodeEl.appendChild(typeLabel);

      createPorts(nodeEl, block.inputs, block.outputs, block.output_values);

      if (block.block_type === 'state_machine') {
        const editorDiv = document.createElement('div');
        editorDiv.className = 'sm-editor-container';
        nodeEl.appendChild(editorDiv);
      }

      const h = nodeHeight(block);
      nodeEl.style.height = `${h}px`;

      nodeLayer.appendChild(nodeEl);
      elements.nodes.set(block.id, nodeEl);
    } else {
      // Update output labels
      updateOutputLabels(nodeEl, block.outputs, block.output_values);
    }

    // Position
    const pos = positions.get(block.id) ?? { x: 50, y: 50 };
    nodeEl.style.transform = `translate(${pos.x}px, ${pos.y}px)`;

    // Selection
    nodeEl.classList.toggle('selected', block.id === selectedId);
  }

  // Remove stale nodes
  for (const [id, nodeEl] of elements.nodes) {
    if (!currentIds.has(id)) {
      nodeEl.remove();
      elements.nodes.delete(id);
    }
  }
}

/** Set up node drag handling. Returns cleanup function. */
export function setupNodeDrag(
  workspace: HTMLDivElement,
  nodeLayer: HTMLDivElement,
  mgr: DataflowManager,
  edges: EdgeElements,
  getSnap: () => GraphSnapshot | null,
  getPanZoom: () => { panX: number; panY: number; scale: number },
  onSelect: (blockId: number | null) => void,
  onDragEnd: () => void,
): () => void {
  let dragBlockId: number | null = null;
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  function screenToWorld(clientX: number, clientY: number): { x: number; y: number } {
    const rect = workspace.getBoundingClientRect();
    const { panX, panY, scale } = getPanZoom();
    return {
      x: (clientX - rect.left - panX) / scale,
      y: (clientY - rect.top - panY) / scale,
    };
  }

  function onPointerDown(e: PointerEvent): void {
    // Don't interfere with port drags
    if ((e.target as HTMLElement).classList.contains('df-port')) return;

    const nodeEl = (e.target as HTMLElement).closest('.df-node') as HTMLElement | null;
    if (!nodeEl) {
      // Clicked empty space — deselect
      onSelect(null);
      return;
    }

    const blockId = parseInt(nodeEl.dataset.id!);
    onSelect(blockId);

    const pos = mgr.positions.get(blockId) ?? { x: 0, y: 0 };
    const world = screenToWorld(e.clientX, e.clientY);
    dragBlockId = blockId;
    dragOffsetX = world.x - pos.x;
    dragOffsetY = world.y - pos.y;

    nodeEl.style.cursor = 'grabbing';
    e.preventDefault();
  }

  function onPointerMove(e: PointerEvent): void {
    if (dragBlockId === null) return;
    const world = screenToWorld(e.clientX, e.clientY);
    const newPos = {
      x: world.x - dragOffsetX,
      y: world.y - dragOffsetY,
    };
    mgr.positions.set(dragBlockId, newPos);

    // Update node transform
    const nodeElements = nodeLayer.querySelectorAll('.df-node');
    for (const el of nodeElements) {
      if ((el as HTMLElement).dataset.id === String(dragBlockId)) {
        (el as HTMLElement).style.transform = `translate(${newPos.x}px, ${newPos.y}px)`;
        break;
      }
    }

    // Update connected edges
    const snap = getSnap();
    if (snap) {
      updateEdgesForBlock(edges, snap.channels, snap.blocks, mgr.positions, dragBlockId);
    }
  }

  function onPointerUp(): void {
    if (dragBlockId !== null) {
      const nodeElements = nodeLayer.querySelectorAll('.df-node');
      for (const el of nodeElements) {
        if ((el as HTMLElement).dataset.id === String(dragBlockId)) {
          (el as HTMLElement).style.cursor = 'grab';
          break;
        }
      }
      dragBlockId = null;
      onDragEnd();
    }
  }

  nodeLayer.addEventListener('pointerdown', onPointerDown);
  workspace.addEventListener('pointermove', onPointerMove);
  workspace.addEventListener('pointerup', onPointerUp);

  return () => {
    nodeLayer.removeEventListener('pointerdown', onPointerDown);
    workspace.removeEventListener('pointermove', onPointerMove);
    workspace.removeEventListener('pointerup', onPointerUp);
  };
}

export function updateStateMachineEditor(
  elements: NodeElements,
  selectedId: number | null,
  snap: { blocks: import('./types.js').BlockSnapshot[] } | null,
  mgr: DataflowManager,
  onConfigChanged: () => void,
): void {
  for (const [, nodeEl] of elements.nodes) {
    const container = nodeEl.querySelector('.sm-editor-container');
    if (container) container.textContent = '';
  }
  if (selectedId === null || !snap) return;
  const block = snap.blocks.find(b => b.id === selectedId);
  if (!block || block.block_type !== 'state_machine') return;
  const nodeEl = elements.nodes.get(selectedId);
  if (!nodeEl) return;
  const container = nodeEl.querySelector('.sm-editor-container') as HTMLElement | null;
  if (!container) return;
  mountStateMachineEditor(
    container,
    selectedId,
    block.config as unknown as import('./types.js').StateMachineConfig,
    mgr,
    onConfigChanged,
  );
}

/** Set up right-click delete on nodes. Returns cleanup function. */
export function setupNodeDelete(
  nodeLayer: HTMLDivElement,
  mgr: DataflowManager,
  _getSelected: () => number | null,
  onDelete: (deletedId: number) => void,
): () => void {
  function onContextMenu(e: MouseEvent): void {
    e.preventDefault();
    const nodeEl = (e.target as HTMLElement).closest('.df-node') as HTMLElement | null;
    if (!nodeEl) return;
    const blockId = parseInt(nodeEl.dataset.id!);
    mgr.removeBlock(blockId);
    onDelete(blockId);
  }

  nodeLayer.addEventListener('contextmenu', onContextMenu);
  return () => nodeLayer.removeEventListener('contextmenu', onContextMenu);
}
