# RustCAM

Browser-based CNC/CAM tool with a reactive dataflow engine. Compiles Rust to WebAssembly for in-browser CAD/CAM toolpath generation and a visual block-based control system editor with multi-target embedded codegen.

## Architecture

See [design.md](design.md) for full design including:
- CAD/CAM MVP with machine tool profiles (CNC mill, laser cutter)
- Multi-frontend model-based development (block editor, Python API, Jupyter notebook)
- MLIR-based code generation and embedded runtime
- MCU inventory, deployment manifest, and multi-node topology

## Tech Stack

- **Core**: Rust (library + WASM via `wasm-bindgen`)
- **Frontend**: Leptos 0.7 (Rust/WASM via Trunk), legacy TypeScript (deprecated)
- **Python bindings**: PyO3 + maturin (planned, `py-binding/` crate)
- **Dataflow engine**: `src/dataflow/` — graph, blocks, channels, codegen
- **Expression DAG**: `dag-core/` — lightweight no_std DAG with CBOR serialization
- **MLIR codegen**: `mlir-codegen/` — MLIR pipeline + curried BlockFn runtime
- **Pub/Sub**: `pubsub/` — no_std message broker (CAN, LIN, IP transports)
- **Module traits**: `module-traits/` — shared trait definitions, MCU inventory, deployment manifest
- **HIL testing**: `hil/` — hardware-in-the-loop with USB/WebSocket/HTTP APIs
- **Codegen targets**: Host, RP2040, STM32F4, STM32G0B1, ESP32-C3

## Build Commands

```bash
# Leptos frontend (primary)
cd hil/combined-frontend && trunk build --release
cd hil/combined-frontend && trunk serve --port 3000

# Rust/WASM (legacy TypeScript frontends, deprecated)
wasm-pack build --target web

# Frontend dev server (deprecated, use trunk serve instead)
cd www && npm run dev

# Rust tests (default-members only)
cargo test

# Full workspace including HIL crates
cargo test --workspace

# Specific crate
cargo test -p dag-core
cargo test -p mlir-codegen
cargo test -p module-traits

# Clippy (CI command)
cargo clippy --all-targets -- -D warnings

# Frontend tests
cd www && npm test

# Build Pico2 firmware
EMBASSY_USB_MAX_INTERFACE_COUNT=16 cargo build --release --target thumbv8m.main-none-eabihf -p board-support-pico2

# Flash Pico2
probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/board-support-pico2
```

## Workspace Crates

```
rustcam                  # Main WASM library (src/)
module-traits/           # Shared traits: Module, Tick, SimModel, Codegen, AnalysisModel
                         #   inventory.rs — MCU definitions (clock, pins, DMA, IRQ)
                         #   deployment.rs — deployment manifest (topology, tasks, channels)
                         #   hardware.rs — peripheral requirements, capabilities, pin assignments
dag-core/                # Expression DAG: Op enum, topological eval, CBOR encode/decode
dag-runtime/             # Lightweight runtime: channel map, evaluation loop
mlir-codegen/            # MLIR pipeline: lower.rs → dialect → C via mlir-opt/mlir-translate
                         #   runtime.rs — curried BlockFn enum, DagRuntime, typeless f64 container
pubsub/                  # no_std pub/sub broker: Topic, NodeAddr, Transport trait
dataflow-rt/             # Embedded runtime: Peripherals trait, Block trait, RingBuffer
parser/                  # PEG-based expression parser
hil/
  hil-backplane/         # UDP multicast message envelope + pub/sub for HIL
  hil-firmware-support/  # Shared firmware: USB builder, WebSocket server, HTTP API handler
  board-support-pico2/   # Pico 2 firmware (Embassy, USB CDC-NCM, DAG executor)
  board-support-pico/    # Pico 1 firmware
  board-support-stm32/   # STM32 firmware
  board-support-pi-zero/ # Pi Zero host-side I2C bridge
  dap-dispatch/          # CMSIS-DAP v2 CBOR protocol
  i2c-hil-sim/           # Simulated I2C buses
  i2c-hil-devices/       # Simulated I2C device models
```

## Key Directories

```
hil/combined-frontend/   # Primary frontend (Leptos 0.7 / Rust WASM via Trunk)

src/
  dataflow/              # Reactive dataflow engine
    block.rs             # Block trait, Value, PortDef, PortKind
    blocks/              # 17+ block types (constant, gain, add, plot, adc, pwm, ...)
    graph.rs             # DataflowGraph, GraphSnapshot
    channel.rs           # Channel, ChannelId
    codegen/             # Multi-target Rust/MLIR code generation
      emit.rs            # String-interpolation Rust codegen (legacy)
      targets/           # Per-MCU Embassy code generators
      binding.rs         # Pin/peripheral binding conversion
      partition.rs       # Multi-MCU graph partitioning with pubsub bridges
    scheduler.rs         # Tick scheduling
  lib.rs                 # WASM API surface (wasm-bindgen exports)

www/                     # (deprecated) Legacy TypeScript frontend — use hil/combined-frontend/
  src/
    dataflow/            # Block editor UI
      editor.ts          # Workspace orchestrator
      graph.ts           # DataflowManager (WASM wrapper)
      hil-client.ts      # HilClient: WebSocket CBOR + HTTP DAG/pubsub API
      types.ts           # TypeScript mirror of Rust types
      node-view.ts       # DOM node rendering
      edge-view.ts       # SVG edge rendering
      port-view.ts       # Wire drag interaction
      palette.ts         # Block type picker
      plot.ts            # Signal plotting

docs/                    # Design documents for multi-frontend architecture
```

## Pico2 HTTP API (DAG / PubSub)

When running on Pico2 (169.254.1.61:8080):

| Method | Path           | Purpose                        |
|--------|----------------|--------------------------------|
| POST   | /api/dag       | Deploy CBOR-encoded DAG        |
| POST   | /api/tick      | Evaluate DAG once              |
| GET    | /api/pubsub    | Snapshot all topic values       |
| GET    | /api/channels  | List registered I/O channels   |
| GET    | /api/status    | DAG status (loaded/nodes/ticks) |
| POST   | /api/debug     | Toggle debug topic publishing  |

## Branches

- `main` — stable
- `feature-embedded-targets` — active development: MLIR codegen, MCU inventory, deployment manifest, HIL
