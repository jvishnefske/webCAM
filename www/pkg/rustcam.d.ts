/** Type declarations for wasm-pack generated bindings (rustcam). */

export default function init(): Promise<void>;
export function process_stl(data: Uint8Array, config_json: string): string;
export function process_svg(svg_text: string, config_json: string): string;
export function process_stl_progress(
  data: Uint8Array,
  config_json: string,
  on_progress: (completed: number, total: number) => void,
): string;
export function process_svg_progress(
  svg_text: string,
  config_json: string,
  on_progress: (completed: number, total: number) => void,
): string;
export function preview_stl(data: Uint8Array, config_json: string): string;
export function preview_svg(svg_text: string): string;
export function sim_moves_stl(data: Uint8Array, config_json: string): string;
export function sim_moves_svg(svg_text: string, config_json: string): string;
export function available_profiles(): string;
export function default_config(machine_type: string): string;

// Sketch actor API
export function sketch_reset(): void;
export function sketch_add_point(x: number, y: number): string;
export function sketch_add_fixed_point(x: number, y: number): string;
export function sketch_move_point(id: number, x: number, y: number): void;
export function sketch_remove_point(id: number): void;
export function sketch_set_fixed(id: number, fixed: boolean): void;
export function sketch_add_constraint(
  kind: string,
  ids_json: string,
  value: number,
  value2: number,
): string;
export function sketch_remove_constraint(id: number): void;
export function sketch_solve(): string;
export function sketch_pump(): string;
export function sketch_snapshot(): string;

// Dataflow simulator API
export function dataflow_new(dt: number): number;
export function dataflow_destroy(graph_id: number): void;
export function dataflow_add_block(graph_id: number, block_type: string, config_json: string): number;
export function dataflow_remove_block(graph_id: number, block_id: number): void;
export function dataflow_connect(graph_id: number, from_block: number, from_port: number, to_block: number, to_port: number): number;
export function dataflow_disconnect(graph_id: number, channel_id: number): void;
export function dataflow_advance(graph_id: number, elapsed: number): string;
export function dataflow_run(graph_id: number, steps: number, dt: number): string;
export function dataflow_set_speed(graph_id: number, speed: number): void;
export function dataflow_snapshot(graph_id: number): string;
export function dataflow_block_types(): string;
