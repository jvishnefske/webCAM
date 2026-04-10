/** Dataflow graph types — schema-driven from WASM. */

export interface PortDef {
  name: string;
  kind: string; // "Float" | "Bytes" | "Text" | "Series" | "Any"
}

// -- Schema types (pushed from WASM) --

export type ParamKind = 'Float' | 'Int' | 'String' | 'Bool';

export interface ParamDef {
  name: string;
  kind: ParamKind;
  default: string;
}

export interface FuncPortDef {
  name: string;
  kind: string;
}

export interface FunctionDef {
  id: string;
  display_name: string;
  category: string;
  op: string;
  inputs: FuncPortDef[];
  outputs: FuncPortDef[];
  params: ParamDef[];
  mlir_op: string | null;
}

// -- MCU / BSP types (pushed from WASM) --

export interface PinDef {
  name: string;
  port: string;
  number: number;
  alt_functions: AltFunction[];
  adc_channel: number | null;
  five_v_tolerant: boolean;
}

export interface AltFunction {
  af: number;
  peripheral: string;
  signal: string;
}

export interface PeripheralInst {
  name: string;
  kind: string;
  signals: { signal: string; pin: string; af: number | null }[];
}

export interface McuDef {
  family: string;
  part_number: string;
  display_name: string;
  pins: PinDef[];
  peripherals: PeripheralInst[];
}

export interface ValueFloat { type: 'Float'; data: number }
export interface ValueBytes { type: 'Bytes'; data: number[] }
export interface ValueText { type: 'Text'; data: string }
export interface ValueSeries { type: 'Series'; data: number[] }
export type Value = ValueFloat | ValueBytes | ValueText | ValueSeries | ValueMessage;

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
}

/** UI-only position data for the node editor. */
export interface NodePosition {
  x: number;
  y: number;
}

// -- Message types --

export type FieldType = 'F32' | 'F64' | 'U8' | 'U16' | 'U32' | 'I32' | 'Bool';

export interface MessageField {
  name: string;
  field_type: FieldType;
}

export interface MessageSchema {
  name: string;
  fields: MessageField[];
}

export interface MessageData {
  schema_name: string;
  fields: [string, number][];
}

export interface ValueMessage { type: 'Message'; data: MessageData }

// -- State machine config --

export interface TopicBinding {
  topic: string;
  schema: MessageSchema;
}

export interface FieldCondition {
  field: string;
  op: 'Eq' | 'Ne' | 'Gt' | 'Lt' | 'Ge' | 'Le';
  value: number;
}

export type TransitionGuard =
  | { type: 'Topic'; topic: string; condition?: FieldCondition }
  | { type: 'Unconditional' }
  | { type: 'GuardPort'; port: number };

export interface TransitionAction {
  topic: string;
  message: [string, number][];
}

export interface TransitionConfig {
  from: string;
  to: string;
  guard: TransitionGuard;
  actions: TransitionAction[];
}

export interface StateMachineConfig {
  states: string[];
  initial: string;
  transitions: TransitionConfig[];
  input_topics: TopicBinding[];
  output_topics: TopicBinding[];
}
