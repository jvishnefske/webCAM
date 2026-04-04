/** Shared types for the CAM frontend. */

export interface CamConfig {
  tool_diameter: number;
  step_over: number;
  strategy: string;
  tool_type: string;
  machine_type: string;
  step_down?: number;
  feed_rate?: number;
  plunge_rate?: number;
  spindle_speed?: number;
  safe_z?: number;
  cut_depth?: number;
  corner_radius?: number;
  effective_diameter?: number;
  scan_direction?: string;
  climb_cut?: boolean;
  perimeter_passes?: number;
  laser_power?: number;
  passes?: number;
  air_assist?: boolean;
}

export interface SimMove {
  x: number;
  y: number;
  z: number;
  rapid: boolean;
}

export interface SimBounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
  w: number;
  h: number;
}

// Sketch shape types
export interface LineShape {
  type: 'line';
  p1: { x: number; y: number };
  p2: { x: number; y: number };
}

export interface RectShape {
  type: 'rect';
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CircleShape {
  type: 'circle';
  cx: number;
  cy: number;
  r: number;
}

export interface PolylineShape {
  type: 'polyline';
  points: Array<{ x: number; y: number }>;
}

export type SketchShape = LineShape | RectShape | CircleShape | PolylineShape;

export interface DraftShape {
  type?: string;
  p1?: { x: number; y: number };
  p2?: { x: number; y: number };
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  cx?: number;
  cy?: number;
  r?: number;
  _cursor?: { x: number; y: number };
}

// Constraint snapshot from WASM
export type DofStatus = 'FullyConstrained' | 'UnderConstrained' | 'OverConstrained';

export interface SketchPoint {
  x: number;
  y: number;
  fixed: boolean;
}

export interface SketchSnapshot {
  points: Array<[number, SketchPoint]>;
  constraints: Array<[number, Record<string, unknown>]>;
  solve: { status: string; iterations: number; max_error: number };
  dof: number;
  dof_status: DofStatus;
  point_status: Record<number, DofStatus>;
}

/** Worker messages */
export interface WorkerReadyMsg { type: 'ready' }
export interface WorkerProgressMsg { type: 'progress'; completed: number; total: number }
export interface WorkerDoneMsg { type: 'done'; gcode: string }
export interface WorkerErrorMsg { type: 'error'; error: string }
export type WorkerOutMsg = WorkerReadyMsg | WorkerProgressMsg | WorkerDoneMsg | WorkerErrorMsg;

export interface WorkerInMsg {
  fileData: Uint8Array | string;
  fileType: 'stl' | 'svg';
  configJson: string;
}
