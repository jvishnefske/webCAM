/* tslint:disable */
/* eslint-disable */

/**
 * Return JSON list of available machine profiles.
 */
export function available_profiles(): string;

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
    readonly available_profiles: () => [number, number];
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
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
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
