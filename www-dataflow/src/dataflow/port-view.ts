/** Port circles and drag-to-connect wire interaction. */

import { theme, portColor } from '../theme.js';
import { edgePath, createDragWire } from './edge-view.js';
import type { DataflowManager } from './graph.js';
import type { GraphSnapshot } from './types.js';

const NODE_W = 140;
const PORT_R = 6;
const PORT_SPACING = 20;
const PORT_OFFSET_Y = 30;

export interface WireDragState {
  fromBlock: number;
  fromPort: number;
  fromX: number;
  fromY: number;
  isOutput: boolean;
  dragPath: SVGPathElement;
}

/** Create port DOM elements inside a node div. Returns cleanup function. */
export function createPorts(
  nodeEl: HTMLDivElement,
  inputs: Array<{ name: string; kind: string }>,
  outputs: Array<{ name: string; kind: string }>,
  outputValues: Array<{ type: string; data: unknown } | null>,
): void {
  // Input ports (left side)
  for (let i = 0; i < inputs.length; i++) {
    const port = inputs[i];
    const py = PORT_OFFSET_Y + i * PORT_SPACING + PORT_SPACING / 2;

    const portEl = document.createElement('div');
    portEl.className = 'df-port';
    portEl.dataset.side = 'input';
    portEl.dataset.index = String(i);
    portEl.style.backgroundColor = portColor(port.kind);
    portEl.style.left = `${-PORT_R}px`;
    portEl.style.top = `${py - PORT_R}px`;
    nodeEl.appendChild(portEl);

    const label = document.createElement('span');
    label.className = 'df-port-label';
    label.style.left = `${PORT_R + 4}px`;
    label.style.top = `${py - 5}px`;
    label.textContent = port.name;
    nodeEl.appendChild(label);
  }

  // Output ports (right side)
  for (let i = 0; i < outputs.length; i++) {
    const port = outputs[i];
    const py = PORT_OFFSET_Y + i * PORT_SPACING + PORT_SPACING / 2;

    const portEl = document.createElement('div');
    portEl.className = 'df-port';
    portEl.dataset.side = 'output';
    portEl.dataset.index = String(i);
    portEl.style.backgroundColor = portColor(port.kind);
    portEl.style.left = `${NODE_W - PORT_R}px`;
    portEl.style.top = `${py - PORT_R}px`;
    nodeEl.appendChild(portEl);

    const val = outputValues[i];
    let labelText = port.name;
    if (val) {
      if (val.type === 'Float') labelText = (val.data as number).toFixed(2);
      else if (val.type === 'Text') labelText = (val.data as string).slice(0, 12);
      else if (val.type === 'Series') labelText = `[${(val.data as number[]).length}]`;
    }
    const label = document.createElement('span');
    label.className = 'df-port-label df-output-value';
    label.style.right = `${PORT_R + 4}px`;
    label.style.top = `${py - 5}px`;
    label.textContent = labelText;
    nodeEl.appendChild(label);
  }
}

/** Update output value labels on an existing node element. */
export function updateOutputLabels(
  nodeEl: HTMLDivElement,
  outputs: Array<{ name: string; kind: string }>,
  outputValues: Array<{ type: string; data: unknown } | null>,
): void {
  const labels = nodeEl.querySelectorAll('.df-output-value');
  for (let i = 0; i < labels.length && i < outputs.length; i++) {
    const val = outputValues[i];
    let labelText = outputs[i].name;
    if (val) {
      if (val.type === 'Float') labelText = (val.data as number).toFixed(2);
      else if (val.type === 'Text') labelText = (val.data as string).slice(0, 12);
      else if (val.type === 'Series') labelText = `[${(val.data as number[]).length}]`;
    }
    (labels[i] as HTMLElement).textContent = labelText;
  }
}

/** Set up wire drag handlers on a workspace. Returns cleanup function. */
export function setupWireDrag(
  workspace: HTMLDivElement,
  nodeLayer: HTMLDivElement,
  svg: SVGSVGElement,
  mgr: DataflowManager,
  _getSnap: () => GraphSnapshot | null,
  getPanZoom: () => { panX: number; panY: number; scale: number },
  onConnect: () => void,
): () => void {
  let wireDrag: WireDragState | null = null;

  function screenToWorld(clientX: number, clientY: number): { x: number; y: number } {
    const rect = workspace.getBoundingClientRect();
    const { panX, panY, scale } = getPanZoom();
    return {
      x: (clientX - rect.left - panX) / scale,
      y: (clientY - rect.top - panY) / scale,
    };
  }

  function onPointerDown(e: PointerEvent): void {
    const target = e.target as HTMLElement;
    if (!target.classList.contains('df-port')) return;

    const nodeEl = target.closest('.df-node') as HTMLElement | null;
    if (!nodeEl) return;

    const blockId = parseInt(nodeEl.dataset.id!);
    const side = target.dataset.side!;
    const portIndex = parseInt(target.dataset.index!);
    const isOutput = side === 'output';

    const pos = mgr.positions.get(blockId) ?? { x: 0, y: 0 };
    const fromX = isOutput ? pos.x + NODE_W : pos.x;
    const fromY = pos.y + PORT_OFFSET_Y + portIndex * PORT_SPACING + PORT_SPACING / 2;

    const dragPath = createDragWire(svg);
    dragPath.setAttribute('stroke', theme.colors.wireActive);

    wireDrag = { fromBlock: blockId, fromPort: portIndex, fromX, fromY, isOutput, dragPath };

    e.preventDefault();
    e.stopPropagation();
  }

  function onPointerMove(e: PointerEvent): void {
    if (!wireDrag) return;
    const world = screenToWorld(e.clientX, e.clientY);
    // edgePath assumes output→input (left-to-right curve).
    // When dragging from an input port, the mouse is the "output" end.
    if (wireDrag.isOutput) {
      wireDrag.dragPath.setAttribute('d', edgePath(wireDrag.fromX, wireDrag.fromY, world.x, world.y));
    } else {
      wireDrag.dragPath.setAttribute('d', edgePath(world.x, world.y, wireDrag.fromX, wireDrag.fromY));
    }
  }

  function onPointerUp(e: PointerEvent): void {
    if (!wireDrag) return;

    // Check if we dropped on a port
    const target = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
    const trace: Record<string, unknown> = {
      event: 'wire-drop',
      fromBlock: wireDrag.fromBlock,
      fromPort: wireDrag.fromPort,
      fromIsOutput: wireDrag.isOutput,
      targetElement: target?.tagName,
      targetClasses: target?.className,
      targetDataSide: target?.dataset?.side,
      targetDataIndex: target?.dataset?.index,
    };

    if (!target) {
      console.log('[wire-trace] no element at point', trace);
      wireDrag.dragPath.remove();
      wireDrag = null;
      return;
    }
    if (target.classList.contains('df-port')) {
      const nodeEl = target.closest('.df-node') as HTMLElement | null;
      if (nodeEl) {
        const toBlockId = parseInt(nodeEl.dataset.id!);
        const toSide = target.dataset.side!;
        const toPortIndex = parseInt(target.dataset.index!);
        const toIsOutput = toSide === 'output';

        trace.toBlock = toBlockId;
        trace.toPort = toPortIndex;
        trace.toIsOutput = toIsOutput;
        trace.sidesMatch = toIsOutput === wireDrag.isOutput;

        if (toIsOutput !== wireDrag.isOutput) {
          const outBlock = wireDrag.isOutput ? wireDrag.fromBlock : toBlockId;
          const outPort = wireDrag.isOutput ? wireDrag.fromPort : toPortIndex;
          const inBlock = wireDrag.isOutput ? toBlockId : wireDrag.fromBlock;
          const inPort = wireDrag.isOutput ? toPortIndex : wireDrag.fromPort;
          trace.connectCall = { outBlock, outPort, inBlock, inPort };
          try {
            mgr.connect(outBlock, outPort, inBlock, inPort);
            trace.result = 'success';
            console.log('[wire-trace]', trace);
            onConnect();
          } catch (err) {
            trace.result = 'error';
            trace.error = String(err);
            console.error('[wire-trace]', trace);
            // Flash target port red briefly, then restore original color
            const origColor = target.style.backgroundColor;
            target.style.backgroundColor = 'var(--color-danger)';
            setTimeout(() => { target.style.backgroundColor = origColor; }, 500);
          }
        } else {
          trace.result = 'same-side-skip';
          console.log('[wire-trace]', trace);
        }
      }
    } else {
      trace.result = 'not-a-port';
      console.log('[wire-trace]', trace);
    }

    wireDrag.dragPath.remove();
    wireDrag = null;
  }

  nodeLayer.addEventListener('pointerdown', onPointerDown);
  workspace.addEventListener('pointermove', onPointerMove);
  workspace.addEventListener('pointerup', onPointerUp);

  return () => {
    nodeLayer.removeEventListener('pointerdown', onPointerDown);
    workspace.removeEventListener('pointermove', onPointerMove);
    workspace.removeEventListener('pointerup', onPointerUp);
    if (wireDrag) {
      wireDrag.dragPath.remove();
      wireDrag = null;
    }
  };
}
