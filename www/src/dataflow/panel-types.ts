/** Panel model types mirroring the Rust structures. */

/** Direction of data flow between a widget and a pubsub topic. */
export type ChannelDirection = 'Input' | 'Output';

/** Binds a widget port to a pubsub topic. */
export interface ChannelBinding {
  topic: string;
  direction: ChannelDirection;
  port_kind: string; // "Float" | "Text" | "Bytes" | "Series"
}

export interface Position { x: number; y: number; }
export interface Size { width: number; height: number; }

export type WidgetKind =
  | { type: 'Toggle' }
  | { type: 'Slider'; min: number; max: number; step: number }
  | { type: 'Gauge'; min: number; max: number }
  | { type: 'Label' }
  | { type: 'Button' }
  | { type: 'Indicator' };

export interface Widget {
  id: number;
  kind: WidgetKind;
  label: string;
  position: Position;
  size: Size;
  channels: ChannelBinding[];
}

export interface PanelModel {
  name: string;
  widgets: Widget[];
}
