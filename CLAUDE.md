# RustCAM

Browser-based CNC/CAM tool with a reactive dataflow engine. Compiles Rust to WebAssembly for in-browser CAD/CAM toolpath generation and a visual block-based control system editor with multi-target embedded codegen.

## Architecture

See [design.md](design.md) for full design including:
- CAD/CAM MVP with machine tool profiles (CNC mill, laser cutter)
- Multi-frontend model-based development (block editor, Python API, Jupyter notebook)

## Tech Stack

- **Core**: Rust (library + WASM via `wasm-bindgen`)
- **Frontend**: TypeScript, Tailwind CSS, Vite
- **Python bindings**: PyO3 + maturin (planned, `py-binding/` crate)
- **Dataflow engine**: `src/dataflow/` — graph, blocks, channels, codegen
- **Codegen targets**: host (std), RP2040, STM32F4, ESP32-C3

## Build Commands

```bash
# Rust/WASM
wasm-pack build --target web

# Frontend dev server
cd www && npm run dev

# Rust tests
cargo test

# Rust tests (specific module)
cargo test dataflow
```

## Key Directories

```
src/
  dataflow/          # Reactive dataflow engine
    block.rs         # Block trait, Value, PortDef, PortKind
    blocks/          # 17 block types (constant, gain, add, plot, adc, pwm, ...)
    graph.rs         # DataflowGraph, GraphSnapshot
    channel.rs       # Channel, ChannelId
    codegen/         # Multi-target Rust workspace code generation
    scheduler.rs     # Tick scheduling
  lib.rs             # WASM API surface (wasm-bindgen exports)

www/
  src/
    dataflow/        # Block editor UI
      editor.ts      # Workspace orchestrator
      graph.ts       # DataflowManager (WASM wrapper)
      types.ts       # TypeScript mirror of Rust types
      node-view.ts   # DOM node rendering
      edge-view.ts   # SVG edge rendering
      port-view.ts   # Wire drag interaction
      palette.ts     # Block type picker
      plot.ts        # Signal plotting

docs/                # Design documents for multi-frontend architecture
```

## Branches

- `main` — stable
- `feature-dataflow` — active development branch for the dataflow engine and block editor
