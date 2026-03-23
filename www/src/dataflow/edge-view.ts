/** SVG path elements for dataflow connections. */

import { portColor } from '../theme.js';
import type { ChannelSnapshot, BlockSnapshot } from './types.js';
import type { NodePosition } from './types.js';

const NODE_W = 140;
const PORT_OFFSET_Y = 30;
const PORT_SPACING = 20;

/** Compute SVG cubic Bezier d attribute for a connection. */
export function edgePath(x1: number, y1: number, x2: number, y2: number): string {
  const dx = Math.abs(x2 - x1);
  const cpX = Math.max(dx * 0.5, Math.min(Math.abs(y2 - y1), 50));
  return `M ${x1},${y1} C ${x1 + cpX},${y1} ${x2 - cpX},${y2} ${x2},${y2}`;
}

/** Compute port center Y in world coords. */
function portY(nodeY: number, portIndex: number): number {
  return nodeY + PORT_OFFSET_Y + portIndex * PORT_SPACING + PORT_SPACING / 2;
}

export interface EdgeElements {
  /** Map from channel id → SVG path element */
  paths: Map<number, SVGPathElement>;
}

/** Create or update SVG path elements to match the current channel list. */
export function reconcileEdges(
  svg: SVGSVGElement,
  edges: EdgeElements,
  channels: ChannelSnapshot[],
  blocks: BlockSnapshot[],
  positions: Map<number, NodePosition>,
  selectedEdge: number | null = null,
): void {
  const blockMap = new Map<number, BlockSnapshot>();
  for (const b of blocks) blockMap.set(b.id, b);

  const currentIds = new Set<number>();
  for (const ch of channels) {
    const chId = ch.id[0];
    currentIds.add(chId);

    const fromBlock = blockMap.get(ch.from_block[0]);
    const toBlock = blockMap.get(ch.to_block[0]);
    if (!fromBlock || !toBlock) continue;

    const fromPos = positions.get(fromBlock.id) ?? { x: 0, y: 0 };
    const toPos = positions.get(toBlock.id) ?? { x: 0, y: 0 };

    const x1 = fromPos.x + NODE_W;
    const y1 = portY(fromPos.y, ch.from_port);
    const x2 = toPos.x;
    const y2 = portY(toPos.y, ch.to_port);

    let path = edges.paths.get(chId);
    if (!path) {
      path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.classList.add('df-edge');
      path.setAttribute('fill', 'none');
      path.setAttribute('stroke-width', '2');
      path.dataset.ch = String(chId);
      svg.appendChild(path);
      edges.paths.set(chId, path);
    }

    // Color based on output port kind
    const outPort = fromBlock.outputs[ch.from_port];
    const color = outPort ? portColor(outPort.kind) : portColor('Any');
    path.setAttribute('stroke', color);
    path.setAttribute('d', edgePath(x1, y1, x2, y2));

    // Selection state
    path.classList.toggle('selected', chId === selectedEdge);
  }

  // Remove stale edges
  for (const [id, path] of edges.paths) {
    if (!currentIds.has(id)) {
      path.remove();
      edges.paths.delete(id);
    }
  }
}

/** Update edge endpoints for edges connected to a specific block (during drag). */
export function updateEdgesForBlock(
  edges: EdgeElements,
  channels: ChannelSnapshot[],
  blocks: BlockSnapshot[],
  positions: Map<number, NodePosition>,
  blockId: number,
): void {
  const blockMap = new Map<number, BlockSnapshot>();
  for (const b of blocks) blockMap.set(b.id, b);

  for (const ch of channels) {
    if (ch.from_block[0] !== blockId && ch.to_block[0] !== blockId) continue;
    const chId = ch.id[0];
    const path = edges.paths.get(chId);
    if (!path) continue;

    const fromBlock = blockMap.get(ch.from_block[0]);
    const toBlock = blockMap.get(ch.to_block[0]);
    if (!fromBlock || !toBlock) continue;

    const fromPos = positions.get(fromBlock.id) ?? { x: 0, y: 0 };
    const toPos = positions.get(toBlock.id) ?? { x: 0, y: 0 };

    const x1 = fromPos.x + NODE_W;
    const y1 = portY(fromPos.y, ch.from_port);
    const x2 = toPos.x;
    const y2 = portY(toPos.y, ch.to_port);

    path.setAttribute('d', edgePath(x1, y1, x2, y2));
  }
}

/** Create a temporary dashed wire for drag-to-connect preview. */
export function createDragWire(svg: SVGSVGElement): SVGPathElement {
  const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.classList.add('df-edge', 'dragging');
  path.setAttribute('fill', 'none');
  path.setAttribute('stroke-width', '2');
  path.setAttribute('stroke-dasharray', '4 4');
  svg.appendChild(path);
  return path;
}
