/** Canvas-based node editor for the dataflow graph. */

import { DataflowManager } from './graph.js';
import type { GraphSnapshot, BlockSnapshot, ChannelSnapshot, BlockTypeInfo } from './types.js';

const NODE_W = 140;
const NODE_H_BASE = 40;
const PORT_R = 6;
const PORT_SPACING = 20;
const PORT_OFFSET_Y = 30;

const COLORS = {
  bg: '#0f1117',
  node: '#1a1d27',
  nodeBorder: '#2a2d3a',
  nodeSelected: '#4f8cff',
  text: '#e0e0e8',
  textDim: '#8888a0',
  portFloat: '#4f8cff',
  portBytes: '#ff9800',
  portText: '#55ff88',
  portSeries: '#ff55aa',
  portAny: '#aaa',
  wire: '#4f8cff66',
  wireActive: '#4f8cff',
};

function portColor(kind: string): string {
  switch (kind) {
    case 'Float': return COLORS.portFloat;
    case 'Bytes': return COLORS.portBytes;
    case 'Text': return COLORS.portText;
    case 'Series': return COLORS.portSeries;
    default: return COLORS.portAny;
  }
}

function nodeHeight(block: BlockSnapshot): number {
  const ports = Math.max(block.inputs.length, block.outputs.length);
  return NODE_H_BASE + Math.max(ports, 1) * PORT_SPACING;
}

interface DragState {
  type: 'move-node';
  blockId: number;
  offsetX: number;
  offsetY: number;
}

interface WireDrag {
  fromBlock: number;
  fromPort: number;
  fromX: number;
  fromY: number;
  isOutput: boolean;
  mouseX: number;
  mouseY: number;
}

export class DataflowEditor {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private mgr: DataflowManager;
  private snap: GraphSnapshot | null = null;
  private selected: number | null = null;
  private drag: DragState | null = null;
  private wireDrag: WireDrag | null = null;
  private panX = 0;
  private panY = 0;
  private blockTypes: BlockTypeInfo[] = [];

  /** Fires when block selection changes. */
  onSelect: ((blockId: number | null, snap: GraphSnapshot | null) => void) | null = null;

  constructor(canvas: HTMLCanvasElement, mgr: DataflowManager) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d')!;
    this.mgr = mgr;
    this.blockTypes = DataflowManager.blockTypes();

    canvas.addEventListener('mousedown', this.onMouseDown);
    canvas.addEventListener('mousemove', this.onMouseMove);
    canvas.addEventListener('mouseup', this.onMouseUp);
    canvas.addEventListener('dblclick', this.onDblClick);
    canvas.addEventListener('contextmenu', this.onContextMenu);

    mgr.onTick = (snap) => {
      this.snap = snap;
      this.draw();
    };
  }

  resize(): void {
    const rect = this.canvas.getBoundingClientRect();
    if (rect.width < 1 || rect.height < 1) return;
    const dpr = window.devicePixelRatio || 1;
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.draw();
  }

  updateSnapshot(): void {
    this.snap = this.mgr.snapshot();
    this.draw();
  }

  draw(): void {
    const ctx = this.ctx;
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    if (rect.width < 1) return;

    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, rect.width, rect.height);
    ctx.fillStyle = COLORS.bg;
    ctx.fillRect(0, 0, rect.width, rect.height);

    if (!this.snap) { ctx.restore(); return; }

    ctx.translate(this.panX, this.panY);

    // Draw wires
    for (const ch of this.snap.channels) {
      this.drawWire(ch);
    }

    // Draw wire being dragged
    if (this.wireDrag) {
      ctx.strokeStyle = COLORS.wireActive;
      ctx.lineWidth = 2;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(this.wireDrag.fromX, this.wireDrag.fromY);
      ctx.lineTo(this.wireDrag.mouseX - this.panX, this.wireDrag.mouseY - this.panY);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // Draw nodes
    for (const block of this.snap.blocks) {
      this.drawNode(block);
    }

    ctx.restore();
  }

  private drawNode(block: BlockSnapshot): void {
    const ctx = this.ctx;
    const pos = this.mgr.positions.get(block.id) ?? { x: 50, y: 50 };
    const h = nodeHeight(block);
    const isSelected = this.selected === block.id;

    // Background
    ctx.fillStyle = COLORS.node;
    ctx.strokeStyle = isSelected ? COLORS.nodeSelected : COLORS.nodeBorder;
    ctx.lineWidth = isSelected ? 2 : 1;
    ctx.beginPath();
    ctx.roundRect(pos.x, pos.y, NODE_W, h, 6);
    ctx.fill();
    ctx.stroke();

    // Title
    ctx.fillStyle = COLORS.text;
    ctx.font = '12px -apple-system, sans-serif';
    ctx.fillText(block.name, pos.x + 10, pos.y + 18);

    // Type subtitle
    ctx.fillStyle = COLORS.textDim;
    ctx.font = '10px monospace';
    ctx.fillText(block.block_type, pos.x + 10, pos.y + 30);

    // Input ports (left side)
    for (let i = 0; i < block.inputs.length; i++) {
      const py = pos.y + PORT_OFFSET_Y + i * PORT_SPACING + PORT_SPACING / 2;
      ctx.fillStyle = portColor(block.inputs[i].kind);
      ctx.beginPath();
      ctx.arc(pos.x, py, PORT_R, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = COLORS.textDim;
      ctx.font = '10px monospace';
      ctx.fillText(block.inputs[i].name, pos.x + PORT_R + 4, py + 3);
    }

    // Output ports (right side)
    for (let i = 0; i < block.outputs.length; i++) {
      const py = pos.y + PORT_OFFSET_Y + i * PORT_SPACING + PORT_SPACING / 2;
      ctx.fillStyle = portColor(block.outputs[i].kind);
      ctx.beginPath();
      ctx.arc(pos.x + NODE_W, py, PORT_R, 0, Math.PI * 2);
      ctx.fill();

      // Output value label
      const val = block.output_values[i];
      let label = block.outputs[i].name;
      if (val) {
        if (val.type === 'Float') label = val.data.toFixed(2);
        else if (val.type === 'Text') label = val.data.slice(0, 12);
        else if (val.type === 'Series') label = `[${val.data.length}]`;
      }
      ctx.fillStyle = COLORS.textDim;
      ctx.font = '10px monospace';
      const tw = ctx.measureText(label).width;
      ctx.fillText(label, pos.x + NODE_W - PORT_R - 4 - tw, py + 3);
    }
  }

  private drawWire(ch: ChannelSnapshot): void {
    const ctx = this.ctx;
    const fromBlock = this.snap!.blocks.find(b => b.id === ch.from_block[0]);
    const toBlock = this.snap!.blocks.find(b => b.id === ch.to_block[0]);
    if (!fromBlock || !toBlock) return;

    const fromPos = this.mgr.positions.get(fromBlock.id) ?? { x: 0, y: 0 };
    const toPos = this.mgr.positions.get(toBlock.id) ?? { x: 0, y: 0 };

    const x1 = fromPos.x + NODE_W;
    const y1 = fromPos.y + PORT_OFFSET_Y + ch.from_port * PORT_SPACING + PORT_SPACING / 2;
    const x2 = toPos.x;
    const y2 = toPos.y + PORT_OFFSET_Y + ch.to_port * PORT_SPACING + PORT_SPACING / 2;

    const cpX = Math.abs(x2 - x1) * 0.5;
    ctx.strokeStyle = COLORS.wireActive;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.bezierCurveTo(x1 + cpX, y1, x2 - cpX, y2, x2, y2);
    ctx.stroke();
  }

  private getPortAt(mx: number, my: number): { blockId: number; portIndex: number; isOutput: boolean; px: number; py: number } | null {
    if (!this.snap) return null;
    for (const block of this.snap.blocks) {
      const pos = this.mgr.positions.get(block.id) ?? { x: 0, y: 0 };
      // Check output ports
      for (let i = 0; i < block.outputs.length; i++) {
        const px = pos.x + NODE_W;
        const py = pos.y + PORT_OFFSET_Y + i * PORT_SPACING + PORT_SPACING / 2;
        if (Math.hypot(mx - px, my - py) < PORT_R + 4) {
          return { blockId: block.id, portIndex: i, isOutput: true, px, py };
        }
      }
      // Check input ports
      for (let i = 0; i < block.inputs.length; i++) {
        const px = pos.x;
        const py = pos.y + PORT_OFFSET_Y + i * PORT_SPACING + PORT_SPACING / 2;
        if (Math.hypot(mx - px, my - py) < PORT_R + 4) {
          return { blockId: block.id, portIndex: i, isOutput: false, px, py };
        }
      }
    }
    return null;
  }

  private getNodeAt(mx: number, my: number): number | null {
    if (!this.snap) return null;
    // Iterate in reverse so topmost node wins
    for (let i = this.snap.blocks.length - 1; i >= 0; i--) {
      const block = this.snap.blocks[i];
      const pos = this.mgr.positions.get(block.id) ?? { x: 0, y: 0 };
      const h = nodeHeight(block);
      if (mx >= pos.x && mx <= pos.x + NODE_W && my >= pos.y && my <= pos.y + h) {
        return block.id;
      }
    }
    return null;
  }

  private canvasCoords(e: MouseEvent): [number, number] {
    const rect = this.canvas.getBoundingClientRect();
    return [e.clientX - rect.left - this.panX, e.clientY - rect.top - this.panY];
  }

  private onMouseDown = (e: MouseEvent): void => {
    const [mx, my] = this.canvasCoords(e);

    // Check for port click (start wiring)
    const port = this.getPortAt(mx, my);
    if (port) {
      this.wireDrag = {
        fromBlock: port.blockId,
        fromPort: port.portIndex,
        fromX: port.px,
        fromY: port.py,
        isOutput: port.isOutput,
        mouseX: e.clientX - this.canvas.getBoundingClientRect().left,
        mouseY: e.clientY - this.canvas.getBoundingClientRect().top,
      };
      return;
    }

    // Check for node click (start dragging)
    const nodeId = this.getNodeAt(mx, my);
    if (nodeId !== null) {
      const pos = this.mgr.positions.get(nodeId) ?? { x: 0, y: 0 };
      this.drag = {
        type: 'move-node',
        blockId: nodeId,
        offsetX: mx - pos.x,
        offsetY: my - pos.y,
      };
      this.selected = nodeId;
      this.onSelect?.(nodeId, this.snap);
      this.draw();
      return;
    }

    this.selected = null;
    this.onSelect?.(null, this.snap);
    this.draw();
  };

  private onMouseMove = (e: MouseEvent): void => {
    if (this.drag) {
      const [mx, my] = this.canvasCoords(e);
      this.mgr.positions.set(this.drag.blockId, {
        x: mx - this.drag.offsetX,
        y: my - this.drag.offsetY,
      });
      this.draw();
    }
    if (this.wireDrag) {
      const rect = this.canvas.getBoundingClientRect();
      this.wireDrag.mouseX = e.clientX - rect.left;
      this.wireDrag.mouseY = e.clientY - rect.top;
      this.draw();
    }
  };

  private onMouseUp = (e: MouseEvent): void => {
    if (this.wireDrag) {
      const [mx, my] = this.canvasCoords(e);
      const port = this.getPortAt(mx, my);
      if (port && port.isOutput !== this.wireDrag.isOutput) {
        try {
          if (this.wireDrag.isOutput) {
            this.mgr.connect(this.wireDrag.fromBlock, this.wireDrag.fromPort, port.blockId, port.portIndex);
          } else {
            this.mgr.connect(port.blockId, port.portIndex, this.wireDrag.fromBlock, this.wireDrag.fromPort);
          }
          this.snap = this.mgr.snapshot();
        } catch (err) {
          console.warn('connect failed:', err);
        }
      }
      this.wireDrag = null;
      this.draw();
    }
    this.drag = null;
  };

  private onDblClick = (e: MouseEvent): void => {
    // Double-click on empty space: show block palette
    const [mx, my] = this.canvasCoords(e);
    const nodeId = this.getNodeAt(mx, my);
    if (nodeId === null) {
      this.showPalette(mx, my);
    }
  };

  private onContextMenu = (e: MouseEvent): void => {
    e.preventDefault();
    const [mx, my] = this.canvasCoords(e);
    const nodeId = this.getNodeAt(mx, my);
    if (nodeId !== null) {
      this.mgr.removeBlock(nodeId);
      if (this.selected === nodeId) {
        this.selected = null;
        this.onSelect?.(null, this.snap);
      }
      this.snap = this.mgr.snapshot();
      this.draw();
    }
  };

  private showPalette(x: number, y: number): void {
    // Remove any existing palette
    document.getElementById('df-palette')?.remove();

    const div = document.createElement('div');
    div.id = 'df-palette';
    div.style.cssText = `
      position: fixed; z-index: 100; background: #1a1d27; border: 1px solid #2a2d3a;
      border-radius: 6px; padding: 4px 0; font-size: 13px; color: #e0e0e8;
      max-height: 300px; overflow-y: auto; min-width: 160px;
    `;
    const rect = this.canvas.getBoundingClientRect();
    div.style.left = `${rect.left + x + this.panX}px`;
    div.style.top = `${rect.top + y + this.panY}px`;

    let lastCat = '';
    for (const bt of this.blockTypes) {
      if (bt.category !== lastCat) {
        lastCat = bt.category;
        const header = document.createElement('div');
        header.style.cssText = 'padding: 4px 12px; font-size: 11px; color: #8888a0; text-transform: uppercase;';
        header.textContent = bt.category;
        div.appendChild(header);
      }
      const item = document.createElement('div');
      item.style.cssText = 'padding: 4px 12px; cursor: pointer;';
      item.textContent = bt.name;
      item.addEventListener('mouseenter', () => { item.style.background = '#2a2d3a'; });
      item.addEventListener('mouseleave', () => { item.style.background = 'transparent'; });
      item.addEventListener('click', () => {
        const defaultConfig = bt.block_type === 'constant' ? { value: 1.0 }
          : bt.block_type === 'gain' ? { op: 'Gain', param1: 1.0, param2: 0.0 }
          : bt.block_type === 'clamp' ? { op: 'Clamp', param1: 0.0, param2: 100.0 }
          : bt.block_type === 'plot' ? { max_samples: 500 }
          : bt.block_type === 'udp_source' || bt.block_type === 'udp_sink' ? { address: '127.0.0.1:9000' }
          : {};
        this.mgr.addBlock(bt.block_type, defaultConfig, x, y);
        this.snap = this.mgr.snapshot();
        this.draw();
        div.remove();
      });
      div.appendChild(item);
    }

    document.body.appendChild(div);
    const dismiss = (ev: MouseEvent) => {
      if (!div.contains(ev.target as Node)) {
        div.remove();
        document.removeEventListener('mousedown', dismiss);
      }
    };
    setTimeout(() => document.addEventListener('mousedown', dismiss), 0);
  }
}
