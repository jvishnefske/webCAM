/** Dataflow graph types mirroring the Rust structures. */

export interface PortDef {
  name: string;
  kind: string; // "Float" | "Bytes" | "Text" | "Series" | "Any"
}

export interface ValueFloat { type: 'Float'; data: number }
export interface ValueBytes { type: 'Bytes'; data: number[] }
export interface ValueText { type: 'Text'; data: string }
export interface ValueSeries { type: 'Series'; data: number[] }
export type Value = ValueFloat | ValueBytes | ValueText | ValueSeries;

export interface ChannelSnapshot {
  id: number | { 0: number };
  from_block: number | { 0: number };
  from_port: number;
  to_block: number | { 0: number };
  to_port: number;
}

export interface BlockSnapshot {
  id: number;
  block_type: string;
  name: string;
  inputs: PortDef[];
  outputs: PortDef[];
  config: Record<string, unknown>;
  output_values: (Value | null)[];
}

export interface GraphSnapshot {
  blocks: BlockSnapshot[];
  channels: ChannelSnapshot[];
  tick_count: number;
  time: number;
}

export interface BlockTypeInfo {
  block_type: string;
  name: string;
  category: string;
  tags: string[];
}

/** UI-only position data for the node editor. */
export interface NodePosition {
  x: number;
  y: number;
}
