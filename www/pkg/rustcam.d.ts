/* tslint:disable */
/* eslint-disable */

export class DagHandle {
    free(): void;
    [Symbol.dispose](): void;
    add(a: number, b: number): number;
    constant(value: number): number;
    div(a: number, b: number): number;
    /**
     * Evaluate the DAG with null channels (pure math).
     * Returns the values array as a `Float64Array`.
     */
    evaluate(): Float64Array;
    /**
     * Get value at a specific node after evaluation.
     */
    evaluate_node(node_id: number): number;
    /**
     * Decode from CBOR bytes.
     */
    static from_cbor(bytes: Uint8Array): DagHandle;
    input(name: string): number;
    is_empty(): boolean;
    len(): number;
    mul(a: number, b: number): number;
    neg(a: number): number;
    constructor();
    output(name: string, src: number): number;
    pow(base: number, exp: number): number;
    publish(topic: string, src: number): number;
    relu(a: number): number;
    sub(a: number, b: number): number;
    subscribe(topic: string): number;
    /**
     * Encode to CBOR bytes.
     */
    to_cbor(): Uint8Array;
    /**
     * Get a JSON representation of the DAG structure for the UI.
     */
    to_json(): string;
}

/**
 * Return JSON list of available machine profiles.
 */
export function available_profiles(): string;

/**
 * Add a block to a graph. Returns block id.
 */
export function dataflow_add_block(graph_id: number, block_type: string, config_json: string): number;

/**
 * Add a simulated I2C device on the given bus at the given 7-bit address.
 */
export function dataflow_add_i2c_device(graph_id: number, bus: number, addr: number, name: string): void;

/**
 * Advance the graph by wall-clock elapsed seconds (realtime mode).
 * Returns snapshot JSON.
 */
export function dataflow_advance(graph_id: number, elapsed: number): string;

/**
 * List available block types as JSON.
 */
export function dataflow_block_types(): string;

/**
 * Generate a standalone Rust crate from a dataflow graph.
 * Returns JSON: `{ "files": [["path", "content"], ...] }` or error.
 */
export function dataflow_codegen(graph_id: number, dt: number): string;

/**
 * Generate a multi-target workspace from a dataflow graph.
 *
 * `targets_json` is a JSON array of `{ "target": "host"|"rp2040"|"stm32f4"|"esp32c3", "binding": {...} }`.
 * Returns JSON: `[["path", "content"], ...]` or error.
 */
export function dataflow_codegen_multi(graph_id: number, dt: number, targets_json: string): string;

/**
 * Configure a simulated serial port. Parity: 0=None, 1=Odd, 2=Even.
 */
export function dataflow_configure_serial(graph_id: number, port: number, baud: number, data_bits: number, parity: number, stop_bits: number): void;

/**
 * Connect an output port to an input port. Returns channel id.
 */
export function dataflow_connect(graph_id: number, from_block: number, from_port: number, to_block: number, to_port: number): number;

/**
 * Destroy a dataflow graph.
 */
export function dataflow_destroy(graph_id: number): void;

/**
 * Disconnect a channel.
 */
export function dataflow_disconnect(graph_id: number, channel_id: number): void;

/**
 * Read the last PWM duty written by a simulated PWM block.
 */
export function dataflow_get_sim_pwm(graph_id: number, channel: number): number;

/**
 * Read the 256-byte register map of a simulated I2C device (as JSON array).
 */
export function dataflow_i2c_device_registers(graph_id: number, bus: number, addr: number): any;

/**
 * Create a new dataflow graph. Returns its id.
 */
export function dataflow_new(dt: number): number;

/**
 * Remove a block from a graph.
 */
export function dataflow_remove_block(graph_id: number, block_id: number): void;

/**
 * Remove a simulated I2C device.
 */
export function dataflow_remove_i2c_device(graph_id: number, bus: number, addr: number): void;

/**
 * Run a fixed number of ticks (non-realtime batch mode).
 * Returns snapshot JSON.
 */
export function dataflow_run(graph_id: number, steps: number, dt: number): string;

/**
 * List all configured serial ports as JSON.
 */
export function dataflow_serial_ports(graph_id: number): any;

/**
 * Set a simulated ADC channel voltage.
 */
export function dataflow_set_sim_adc(graph_id: number, channel: number, voltage: number): void;

/**
 * Enable or disable simulation mode for a graph.
 * When enabled, peripheral blocks use SimModel dispatch with simulated peripherals.
 */
export function dataflow_set_simulation_mode(graph_id: number, enabled: boolean): void;

/**
 * Set the simulation speed multiplier.
 */
export function dataflow_set_speed(graph_id: number, speed: number): void;

/**
 * Get a snapshot of the graph without ticking.
 */
export function dataflow_snapshot(graph_id: number): string;

/**
 * Drain data from a simulated TCP send buffer (as JSON array).
 */
export function dataflow_tcp_drain(graph_id: number, socket_id: number): any;

/**
 * Inject data into a simulated TCP receive buffer.
 */
export function dataflow_tcp_inject(graph_id: number, socket_id: number, data: Uint8Array): void;

/**
 * Update a block's config by replacing it in-place (preserving channels where ports still match).
 */
export function dataflow_update_block(graph_id: number, block_id: number, block_type: string, config_json: string): void;

/**
 * Return a default config JSON for the given machine type.
 */
export function default_config(machine_type: string): string;

/**
 * Return toolpath data as JSON (for the 2-D preview canvas).
 * Returns toolpath moves with Z coordinates for 3D visualization.
 */
export function preview_stl(data: Uint8Array, config_json: string): string;

/**
 * Return toolpath data from SVG as JSON (for the 2-D preview canvas).
 */
export function preview_svg(svg_text: string): string;

/**
 * Process an STL file (binary bytes) and return G-code.
 */
export function process_stl(data: Uint8Array, config_json: string): string;

/**
 * Process an STL file with progress reporting.
 * The callback receives (completed_layers, total_layers) after each layer.
 */
export function process_stl_progress(data: Uint8Array, config_json: string, on_progress: Function): string;

/**
 * Process an SVG string and return G-code.
 */
export function process_svg(svg_text: string, config_json: string): string;

/**
 * Process an SVG string with progress reporting.
 * The callback receives (completed_layers, total_layers) after each layer.
 */
export function process_svg_progress(svg_text: string, config_json: string, on_progress: Function): string;

/**
 * Create a new empty panel. Returns its id.
 */
export function panel_new(name: string): number;

/**
 * Destroy a panel, removing it from storage.
 */
export function panel_destroy(panel_id: number): void;

/**
 * Deserialize a PanelModel from JSON, store it, and return its id.
 */
export function panel_load(json: string): number;

/**
 * Serialize a panel to JSON.
 */
export function panel_save(panel_id: number): string;

/**
 * Add a widget to a panel from JSON config. Returns the assigned widget id.
 */
export function panel_add_widget(panel_id: number, config_json: string): number;

/**
 * Remove a widget from a panel. Returns whether it was found.
 */
export function panel_remove_widget(panel_id: number, widget_id: number): boolean;

/**
 * Update a widget's config in-place (id is preserved).
 */
export function panel_update_widget(panel_id: number, widget_id: number, config_json: string): void;

/**
 * JSON snapshot of the full panel.
 */
export function panel_snapshot(panel_id: number): string;

/**
 * Return flat move list as JSON for the tool simulation.
 * Each move: `{ x, y, z, rapid }`.
 */
export function sim_moves_stl(data: Uint8Array, config_json: string): string;

export function sim_moves_svg(svg_text: string, config_json: string): string;

/**
 * Add a constraint. `kind` is one of: "coincident", "distance",
 * "horizontal", "vertical", "fixed", "angle", "radius",
 * "perpendicular", "parallel", "midpoint", "equal_length", "symmetric".
 *
 * `ids` is a JSON array of point ids, `value` is the numeric parameter
 * (distance, angle, radius, x, y — depends on constraint type).
 * For "fixed", pass `value` as x and `value2` as y.
 *
 * Returns JSON `{"id": <u32>}`.
 */
export function sketch_add_constraint(kind: string, ids_json: string, value: number, value2: number): string;

/**
 * Add a fixed point. Returns JSON `{"id": <u32>}`.
 */
export function sketch_add_fixed_point(x: number, y: number): string;

/**
 * Add a free point. Returns JSON `{"id": <u32>}`.
 */
export function sketch_add_point(x: number, y: number): string;

/**
 * Move a point to new coordinates.
 */
export function sketch_move_point(id: number, x: number, y: number): void;

/**
 * Process queued messages and return snapshot JSON.
 */
export function sketch_pump(): string;

/**
 * Remove a constraint by id.
 */
export function sketch_remove_constraint(id: number): void;

/**
 * Remove a point and all its constraints.
 */
export function sketch_remove_point(id: number): void;

/**
 * Reset the sketch actor to a blank state.
 */
export function sketch_reset(): void;

/**
 * Set a point's fixed flag.
 */
export function sketch_set_fixed(id: number, fixed: boolean): void;

/**
 * Get current snapshot without solving (read-only query).
 */
export function sketch_snapshot(): string;

/**
 * Run the constraint solver and return a full snapshot as JSON.
 * The snapshot includes points, constraints, DOF, solve status,
 * and per-point coloring status.
 */
export function sketch_solve(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_daghandle_free: (a: number, b: number) => void;
    readonly available_profiles: () => [number, number];
    readonly daghandle_add: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_constant: (a: number, b: number) => [number, number, number];
    readonly daghandle_div: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_evaluate: (a: number) => [number, number];
    readonly daghandle_evaluate_node: (a: number, b: number) => number;
    readonly daghandle_from_cbor: (a: number, b: number) => [number, number, number];
    readonly daghandle_input: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_is_empty: (a: number) => number;
    readonly daghandle_len: (a: number) => number;
    readonly daghandle_mul: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_neg: (a: number, b: number) => [number, number, number];
    readonly daghandle_new: () => number;
    readonly daghandle_output: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly daghandle_pow: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_publish: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly daghandle_relu: (a: number, b: number) => [number, number, number];
    readonly daghandle_sub: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_subscribe: (a: number, b: number, c: number) => [number, number, number];
    readonly daghandle_to_cbor: (a: number) => [number, number];
    readonly daghandle_to_json: (a: number) => [number, number, number, number];
    readonly dataflow_add_block: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly dataflow_add_i2c_device: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly dataflow_advance: (a: number, b: number) => [number, number, number, number];
    readonly dataflow_block_types: () => [number, number];
    readonly dataflow_codegen: (a: number, b: number) => [number, number, number, number];
    readonly dataflow_codegen_multi: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly dataflow_configure_serial: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly dataflow_connect: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly dataflow_destroy: (a: number) => void;
    readonly dataflow_disconnect: (a: number, b: number) => [number, number];
    readonly dataflow_get_sim_pwm: (a: number, b: number) => [number, number, number];
    readonly dataflow_i2c_device_registers: (a: number, b: number, c: number) => [number, number, number];
    readonly dataflow_new: (a: number) => number;
    readonly dataflow_remove_block: (a: number, b: number) => [number, number];
    readonly dataflow_remove_i2c_device: (a: number, b: number, c: number) => [number, number];
    readonly dataflow_run: (a: number, b: number, c: number) => [number, number, number, number];
    readonly dataflow_serial_ports: (a: number) => [number, number, number];
    readonly dataflow_set_sim_adc: (a: number, b: number, c: number) => [number, number];
    readonly dataflow_set_simulation_mode: (a: number, b: number) => [number, number];
    readonly dataflow_set_speed: (a: number, b: number) => [number, number];
    readonly dataflow_snapshot: (a: number) => [number, number, number, number];
    readonly dataflow_tcp_drain: (a: number, b: number) => [number, number, number];
    readonly dataflow_tcp_inject: (a: number, b: number, c: number, d: number) => [number, number];
    readonly dataflow_update_block: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly default_config: (a: number, b: number) => [number, number];
    readonly preview_stl: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly preview_svg: (a: number, b: number) => [number, number, number, number];
    readonly process_stl: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly process_stl_progress: (a: number, b: number, c: number, d: number, e: any) => [number, number, number, number];
    readonly process_svg: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly process_svg_progress: (a: number, b: number, c: number, d: number, e: any) => [number, number, number, number];
    readonly sim_moves_stl: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly sim_moves_svg: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly sketch_add_constraint: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly sketch_add_fixed_point: (a: number, b: number) => [number, number];
    readonly sketch_add_point: (a: number, b: number) => [number, number];
    readonly sketch_move_point: (a: number, b: number, c: number) => void;
    readonly sketch_pump: () => [number, number, number, number];
    readonly sketch_remove_constraint: (a: number) => void;
    readonly sketch_remove_point: (a: number) => void;
    readonly sketch_reset: () => void;
    readonly sketch_set_fixed: (a: number, b: number) => void;
    readonly sketch_snapshot: () => [number, number, number, number];
    readonly sketch_solve: () => [number, number, number, number];
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
