# Split rustcam into rustcam (CAM+sketch) and rustsim (dataflow)

## Motivation

Separate audiences: CAM+sketch targets CNC/laser hobbyists, dataflow targets embedded control engineers. Each gets its own crate, WASM bundle, and webapp.

## Workspace Layout

```
webCAM/
├── Cargo.toml              # workspace-only manifest (no [package])
├── crates/
│   ├── rustcam/            # CAM + sketch WASM crate
│   │   ├── Cargo.toml      # depends on: parser
│   │   └── src/
│   │       ├── lib.rs      # wasm-bindgen: process_*, sketch_*, preview_*, config
│   │       ├── toolpath.rs
│   │       ├── sketch_actor.rs
│   │       ├── gcode.rs
│   │       ├── gcode_parser.rs
│   │       ├── svg.rs
│   │       ├── stl.rs
│   │       ├── slicer.rs
│   │       ├── geometry.rs
│   │       ├── machine.rs
│   │       ├── tool.rs
│   │       └── units.rs
│   └── rustsim/            # Dataflow WASM crate
│       ├── Cargo.toml      # depends on: module-traits, dag-core, parser(?), configurable-blocks
│       └── src/
│           ├── lib.rs      # wasm-bindgen: dataflow_*
│           └── dataflow/   # entire dataflow module tree
├── module-traits/
├── dag-core/
├── dag-runtime/
├── mlir-codegen/
├── pubsub/
├── parser/
├── configurable-blocks/
├── dataflow-rt/
├── hil/
├── www-cam/
├── www-dataflow/
└── www-shared/
```

## Rust Crate Split

### rustcam (CAM + sketch)

**Moved from `src/`:**
- toolpath.rs, sketch_actor.rs, gcode.rs, gcode_parser.rs, svg.rs, stl.rs, slicer.rs, geometry.rs, machine.rs, tool.rs, units.rs

**lib.rs exports (wasm-bindgen):**
- `process_stl()`, `process_svg()`, `preview_stl()`, `preview_svg()`
- `sketch_reset()`, `sketch_add_point()`, `sketch_solve()`, `sketch_snapshot()`
- `available_profiles()`, `default_config()`

**Dependencies:** parser

### rustsim (dataflow)

**Moved from `src/`:**
- `dataflow/` entire module tree (block.rs, channel.rs, graph.rs, scheduler.rs, dsl_bridge.rs, sim_peripherals.rs, blocks/, codegen/)

**lib.rs exports (wasm-bindgen):**
- `dataflow_new()`, `dataflow_add_block()`, `dataflow_connect()`
- `dataflow_advance()`, `dataflow_run()`, `dataflow_set_speed()`
- `dataflow_codegen()`, `dataflow_codegen_multi()`
- `dataflow_set_simulation_mode()`, `dataflow_set_sim_adc()`, `dataflow_get_sim_pwm()`

**Dependencies:** module-traits, dag-core, configurable-blocks, i2c-hil-sim (dev)

## Frontend Split

### www-shared/
- tailwind.config.js (shared Tailwind config)
- src/dom.ts, theme.ts, types.ts (shared utilities)

### www-cam/
- package.json (depends on www-shared)
- index.html (CAM+sketch landing page, mode switcher for CAM/sketch only)
- src/main.ts, cam.ts, sketch.ts, constraints.ts, sim.ts, worker.ts, input.css

### www-dataflow/
- package.json (depends on www-shared)
- index.html (dataflow landing page)
- src/main.ts, dataflow/ (all existing dataflow modules), input.css

## Build Targets

```makefile
cam:
    wasm-pack build crates/rustcam --target web --out-dir ../../www-cam/pkg
    cd www-cam && npm run build

dataflow:
    wasm-pack build crates/rustsim --target web --out-dir ../../www-dataflow/pkg
    cd www-dataflow && npm run build

dev-cam:     cd www-cam && npm run dev
dev-dataflow: cd www-dataflow && npm run dev
```

## What Doesn't Change

- All HIL crates stay in hil/
- dag-core, dag-runtime, module-traits, mlir-codegen, pubsub, parser, configurable-blocks, dataflow-rt stay where they are
- design.md updated to reflect two-product structure

## Deleted After Migration

- src/ (old unified crate root)
- www/ (old unified webapp)
