import { DagNode, DagState, NodeId } from './dag-types.js';

// WASM imports -- loaded dynamically
let wasm: any = null;

const NODE_W = 80;
const NODE_H = 36;
const PORT_R = 5;

// Colors per op type
const OP_COLORS: Record<string, string> = {
  const: '#2d6a4f', input: '#1d3557', output: '#6d2e46',
  add: '#457b9d', mul: '#457b9d', sub: '#457b9d', div: '#457b9d', pow: '#457b9d',
  neg: '#6c584c', relu: '#6c584c',
  subscribe: '#7b2d8b', publish: '#7b2d8b',
};

// Op display labels
const OP_LABELS: Record<string, string> = {
  const: 'C', input: 'IN', output: 'OUT',
  add: '+', mul: '\u00d7', sub: '\u2212', div: '\u00f7', pow: '^',
  neg: '\u2212x', relu: 'ReLU',
  subscribe: 'SUB', publish: 'PUB',
};

// Number of inputs per op
const OP_INPUTS: Record<string, number> = {
  const: 0, input: 0, subscribe: 0,
  neg: 1, relu: 1, output: 1, publish: 1,
  add: 2, mul: 2, sub: 2, div: 2, pow: 2,
};

let state: DagState = {
  nodes: [],
  selectedId: null,
  panX: 0, panY: 0, scale: 1,
};

let nextId = 0;
const svg = document.getElementById('canvas') as unknown as SVGSVGElement;

// --- Node management ---

function addNode(op: string): DagNode {
  // Remap toolbar ops to internal op names
  let actualOp = op;
  if (op === 'sub_ps') actualOp = 'subscribe';
  if (op === 'pub_ps') actualOp = 'publish';

  const node: DagNode = {
    id: nextId++,
    op: actualOp,
    x: 200 + Math.random() * 200,
    y: 100 + Math.random() * 200,
  };
  if (actualOp === 'const') node.value = 0;
  if (actualOp === 'input' || actualOp === 'output') node.name = '';
  if (actualOp === 'subscribe' || actualOp === 'publish') node.name = '';
  state.nodes.push(node);
  render();
  updateStatus();
  return node;
}

function removeNode(id: NodeId) {
  state.nodes = state.nodes.filter(n => n.id !== id);
  // Clear references to this node
  for (const n of state.nodes) {
    if (n.a === id) n.a = undefined;
    if (n.b === id) n.b = undefined;
    if (n.src === id) n.src = undefined;
  }
  if (state.selectedId === id) state.selectedId = null;
  render();
  updateStatus();
}

// --- SVG helpers ---

const SVG_NS = 'http://www.w3.org/2000/svg';

function svgEl(tag: string, attrs: Record<string, string>): SVGElement {
  const el = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) {
    el.setAttribute(k, v);
  }
  return el;
}

// --- Rendering ---

function render() {
  // Clear SVG
  while (svg.firstChild) svg.removeChild(svg.firstChild);

  const g = svgEl('g', {
    transform: `translate(${state.panX},${state.panY}) scale(${state.scale})`,
  });
  svg.appendChild(g);

  // Arrow marker definition
  const defs = svgEl('defs', {});
  const marker = svgEl('marker', {
    id: 'arrow', viewBox: '0 0 10 10',
    refX: '10', refY: '5',
    markerWidth: '6', markerHeight: '6',
    orient: 'auto',
  });
  const arrowPath = svgEl('path', { d: 'M 0 0 L 10 5 L 0 10 z', fill: '#534b62' });
  marker.appendChild(arrowPath);
  defs.appendChild(marker);
  g.appendChild(defs);

  // Draw edges first (underneath)
  for (const node of state.nodes) {
    const refs: NodeId[] = [];
    if (node.a !== undefined) refs.push(node.a);
    if (node.b !== undefined) refs.push(node.b);
    if (node.src !== undefined) refs.push(node.src);

    for (const refId of refs) {
      const src = state.nodes.find(n => n.id === refId);
      if (!src) continue;
      const line = svgEl('line', {
        x1: String(src.x + NODE_W),
        y1: String(src.y + NODE_H / 2),
        x2: String(node.x),
        y2: String(node.y + NODE_H / 2),
        stroke: '#534b62',
        'stroke-width': '2',
        'marker-end': 'url(#arrow)',
      });
      g.appendChild(line);
    }
  }

  // Draw nodes
  for (const node of state.nodes) {
    const ng = svgEl('g', {
      transform: `translate(${node.x},${node.y})`,
      'data-id': String(node.id),
    });
    (ng as HTMLElement).style.cursor = 'pointer';

    // Background rect
    const isSelected = node.id === state.selectedId;
    const rect = svgEl('rect', {
      width: String(NODE_W),
      height: String(NODE_H),
      rx: '4',
      fill: OP_COLORS[node.op] || '#333',
      stroke: isSelected ? '#e0e0e0' : '#534b62',
      'stroke-width': isSelected ? '2' : '1',
    });
    ng.appendChild(rect);

    // Op label
    const label = svgEl('text', {
      x: String(NODE_W / 2),
      y: '14',
      'text-anchor': 'middle',
      fill: '#e0e0e0',
      'font-size': '11',
      'font-weight': 'bold',
    });
    label.textContent = OP_LABELS[node.op] || node.op;
    ng.appendChild(label);

    // Value/name detail label
    const detail = svgEl('text', {
      x: String(NODE_W / 2),
      y: '28',
      'text-anchor': 'middle',
      fill: '#aaa',
      'font-size': '10',
    });
    if (node.op === 'const') detail.textContent = String(node.value ?? 0);
    else if (node.name !== undefined) detail.textContent = node.name || '\u2026';
    else if (node.result !== undefined) detail.textContent = node.result.toFixed(4);
    else detail.textContent = `#${node.id}`;
    ng.appendChild(detail);

    // Input ports (left side)
    const numIn = OP_INPUTS[node.op] || 0;
    for (let i = 0; i < numIn; i++) {
      const cy = (NODE_H / (numIn + 1)) * (i + 1);
      const port = svgEl('circle', {
        cx: '0', cy: String(cy), r: String(PORT_R),
        fill: '#0f3460', stroke: '#e0e0e0', 'stroke-width': '1',
        'data-port': String(i), 'data-side': 'in',
      });
      ng.appendChild(port);
    }

    // Output port (right side) -- all ops except output and publish have one
    if (node.op !== 'output' && node.op !== 'publish') {
      const port = svgEl('circle', {
        cx: String(NODE_W), cy: String(NODE_H / 2), r: String(PORT_R),
        fill: '#2d6a4f', stroke: '#e0e0e0', 'stroke-width': '1',
        'data-side': 'out',
      });
      ng.appendChild(port);
    }

    g.appendChild(ng);
  }
}

// --- Interaction ---

function setupInteraction() {
  let dragNode: DagNode | null = null;
  let dragOffX = 0, dragOffY = 0;
  let connectMode: { nodeId: NodeId; port: number } | null = null;

  svg.addEventListener('mousedown', (e) => {
    const target = e.target as SVGElement;
    const nodeG = target.closest('[data-id]') as SVGElement | null;

    // Click on an input port to complete a connection
    if (target.getAttribute('data-side') === 'in' && nodeG) {
      const id = parseInt(nodeG.getAttribute('data-id')!);
      const port = parseInt(target.getAttribute('data-port')!);
      if (connectMode) {
        const destNode = state.nodes.find(n => n.id === id);
        if (destNode) {
          if (port === 0) {
            if (destNode.src !== undefined) destNode.src = connectMode.nodeId;
            else destNode.a = connectMode.nodeId;
          } else {
            destNode.b = connectMode.nodeId;
          }
        }
        connectMode = null;
        render();
        return;
      }
    }

    // Click on an output port to start a connection
    if (target.getAttribute('data-side') === 'out' && nodeG) {
      const id = parseInt(nodeG.getAttribute('data-id')!);
      connectMode = { nodeId: id, port: 0 };
      return;
    }

    // Click on a node body to select/drag
    if (nodeG) {
      const id = parseInt(nodeG.getAttribute('data-id')!);
      state.selectedId = id;
      const node = state.nodes.find(n => n.id === id);
      if (node) {
        dragNode = node;
        dragOffX = e.clientX / state.scale - node.x;
        dragOffY = e.clientY / state.scale - node.y;
      }
      render();
      showInspector(node!);
      return;
    }

    // Click on background -- deselect
    state.selectedId = null;
    connectMode = null;
    hideInspector();
    render();
  });

  svg.addEventListener('mousemove', (e) => {
    if (dragNode) {
      dragNode.x = e.clientX / state.scale - dragOffX;
      dragNode.y = e.clientY / state.scale - dragOffY;
      render();
    }
  });

  svg.addEventListener('mouseup', () => {
    dragNode = null;
  });

  // Delete key removes selected node
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Delete' || e.key === 'Backspace') {
      if (state.selectedId !== null) {
        const active = document.activeElement;
        if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) return;
        removeNode(state.selectedId);
        hideInspector();
      }
    }
  });
}

// --- Inspector ---

function showInspector(node: DagNode) {
  const panel = document.getElementById('node-inspector')!;
  const title = document.getElementById('insp-title')!;
  const body = document.getElementById('insp-body')!;

  panel.style.display = 'block';
  title.textContent = `${OP_LABELS[node.op] || node.op} #${node.id}`;

  // Clear body safely
  while (body.firstChild) body.removeChild(body.firstChild);

  if (node.op === 'const') {
    const field = document.createElement('div');
    field.className = 'field';
    field.textContent = 'Value: ';
    const inp = document.createElement('input');
    inp.type = 'number';
    inp.value = String(node.value ?? 0);
    inp.step = 'any';
    inp.addEventListener('change', () => {
      node.value = parseFloat(inp.value);
      render();
    });
    field.appendChild(inp);
    body.appendChild(field);
  }

  if (node.name !== undefined) {
    const field = document.createElement('div');
    field.className = 'field';
    field.textContent = 'Name: ';
    const inp = document.createElement('input');
    inp.type = 'text';
    inp.value = node.name || '';
    inp.addEventListener('change', () => {
      node.name = inp.value;
      render();
    });
    field.appendChild(inp);
    body.appendChild(field);
  }

  if (node.result !== undefined) {
    const field = document.createElement('div');
    field.className = 'field';
    field.textContent = `Result: ${node.result.toFixed(6)}`;
    body.appendChild(field);
  }

  // Show connections
  const conns: string[] = [];
  if (node.a !== undefined) conns.push(`a \u2190 #${node.a}`);
  if (node.b !== undefined) conns.push(`b \u2190 #${node.b}`);
  if (node.src !== undefined) conns.push(`src \u2190 #${node.src}`);
  if (conns.length) {
    const field = document.createElement('div');
    field.className = 'field';
    field.style.marginTop = '4px';
    field.textContent = conns.join(', ');
    body.appendChild(field);
  }
}

function hideInspector() {
  document.getElementById('node-inspector')!.style.display = 'none';
}

// --- DAG building helper (shared by evaluate and push) ---

function buildDagHandle(): { handle: any; nodeMap: Map<number, number> } {
  const handle = new wasm.DagHandle();
  const nodeMap = new Map<number, number>();
  const resolved = new Set<number>();
  const pending = [...state.nodes];
  let maxIter = pending.length * pending.length + 1;

  while (pending.length > 0 && maxIter-- > 0) {
    for (let i = 0; i < pending.length; i++) {
      const n = pending[i];
      const deps: (number | undefined)[] = [n.a, n.b, n.src];
      const unresolvedDeps = deps.filter(d => d !== undefined && !resolved.has(d!));

      if (unresolvedDeps.length === 0) {
        let dagId: number;
        switch (n.op) {
          case 'const':
            dagId = handle.constant(n.value ?? 0);
            break;
          case 'input':
            dagId = handle.input(n.name || 'unnamed');
            break;
          case 'output':
            dagId = handle.output(n.name || 'unnamed', nodeMap.get(n.src!)!);
            break;
          case 'add':
            dagId = handle.add(nodeMap.get(n.a!)!, nodeMap.get(n.b!)!);
            break;
          case 'mul':
            dagId = handle.mul(nodeMap.get(n.a!)!, nodeMap.get(n.b!)!);
            break;
          case 'sub':
            dagId = handle.sub(nodeMap.get(n.a!)!, nodeMap.get(n.b!)!);
            break;
          case 'div':
            dagId = handle.div(nodeMap.get(n.a!)!, nodeMap.get(n.b!)!);
            break;
          case 'pow':
            dagId = handle.pow(nodeMap.get(n.a!)!, nodeMap.get(n.b!)!);
            break;
          case 'neg':
            dagId = handle.neg(nodeMap.get(n.a!)!);
            break;
          case 'relu':
            dagId = handle.relu(nodeMap.get(n.a!)!);
            break;
          case 'subscribe':
            dagId = handle.subscribe(n.name || 'unnamed');
            break;
          case 'publish':
            dagId = handle.publish(n.name || 'unnamed', nodeMap.get(n.src!)!);
            break;
          default:
            continue;
        }

        nodeMap.set(n.id, dagId);
        resolved.add(n.id);
        pending.splice(i, 1);
        i--;
      }
    }
  }

  return { handle, nodeMap };
}

// --- Evaluate ---

async function evaluate() {
  if (!wasm) {
    document.getElementById('st-result')!.textContent = 'WASM not loaded';
    return;
  }

  try {
    const { handle, nodeMap } = buildDagHandle();

    // Evaluate
    const values: number[] = handle.evaluate();

    // Update visual nodes with results
    for (const n of state.nodes) {
      const dagId = nodeMap.get(n.id);
      if (dagId !== undefined) {
        n.result = values[dagId];
      }
    }

    // Show last node's result
    if (state.nodes.length > 0) {
      const lastNode = state.nodes[state.nodes.length - 1];
      document.getElementById('st-result')!.textContent =
        `Result: ${lastNode.result?.toFixed(4) ?? '?'}`;
    }

    // Show CBOR size
    const cbor: Uint8Array = handle.to_cbor();
    document.getElementById('st-cbor')!.textContent = `CBOR: ${cbor.length}B`;

    handle.free();
    render();

    // Update inspector if a node is selected
    if (state.selectedId !== null) {
      const sel = state.nodes.find(n => n.id === state.selectedId);
      if (sel) showInspector(sel);
    }
  } catch (e) {
    document.getElementById('st-result')!.textContent = `Error: ${e}`;
  }
}

// --- Push to MCU ---

async function pushToMCU() {
  if (!wasm) {
    document.getElementById('st-result')!.textContent = 'WASM not loaded';
    return;
  }

  try {
    const { handle } = buildDagHandle();
    const cbor: Uint8Array = handle.to_cbor();
    handle.free();

    const resp = await fetch('/api/dag', {
      method: 'POST',
      headers: { 'Content-Type': 'application/cbor' },
      body: cbor,
    });

    document.getElementById('st-result')!.textContent =
      resp.ok ? `Pushed ${cbor.length}B to MCU` : `Push failed: ${resp.status}`;
  } catch (e) {
    document.getElementById('st-result')!.textContent = `Push error: ${e}`;
  }
}

// --- Status ---

function updateStatus() {
  document.getElementById('st-nodes')!.textContent = `Nodes: ${state.nodes.length}`;
}

// --- Init ---

async function init() {
  try {
    const mod = await import('../pkg/rustcam.js');
    await mod.default();
    wasm = mod;
    document.getElementById('st-result')!.textContent = 'WASM loaded';
  } catch (_e) {
    document.getElementById('st-result')!.textContent = 'WASM not available (standalone mode)';
  }

  setupInteraction();

  // Toolbar op buttons
  for (const btn of document.querySelectorAll('#toolbar button[data-op]')) {
    btn.addEventListener('click', () => {
      addNode(btn.getAttribute('data-op')!);
    });
  }

  document.getElementById('btn-eval')!.addEventListener('click', evaluate);
  document.getElementById('btn-push')!.addEventListener('click', pushToMCU);
  document.getElementById('btn-clear')!.addEventListener('click', () => {
    state.nodes = [];
    state.selectedId = null;
    nextId = 0;
    hideInspector();
    render();
    updateStatus();
  });

  render();
  updateStatus();
}

init();
