export type NodeId = number;

export interface DagNode {
  id: NodeId;
  op: string;  // "const", "input", "output", "add", "mul", "sub", "div", "pow", "neg", "relu", "subscribe", "publish"
  // Op-specific fields
  value?: number;       // for const
  name?: string;        // for input, output, subscribe, publish
  a?: NodeId;           // first operand
  b?: NodeId;           // second operand (binary ops)
  src?: NodeId;         // source node (output, publish)
  // Visual position
  x: number;
  y: number;
  // Evaluation result
  result?: number;
}

export interface DagState {
  nodes: DagNode[];
  // Track which node is selected
  selectedId: NodeId | null;
  // Viewport
  panX: number;
  panY: number;
  scale: number;
}

export interface SavedState {
  nodes: DagNode[];
  nextId: number;
  panX: number;
  panY: number;
  scale: number;
}
