# RustCAM Design

## Overview

Browser-based CNC/CAM tool with a reactive dataflow engine. Compiles Rust to
WebAssembly for in-browser CAD/CAM toolpath generation and a visual block-based
control system editor with multi-target embedded codegen.

Two major subsystems:

1. **CAD/CAM**: STL/SVG → toolpath → G-code with machine tool profiles (CNC mill, laser cutter)
2. **Dataflow engine**: Visual block editor → multi-target embedded code generation with MCU inventory, deployment manifest, and hardware-in-the-loop testing

---

## Part I: CAD/CAM MVP

### Functional Requirements

#### FR-1: Type-Safe Unit System (ported from cnc-sender)
- [x] **FR-1.1**: Phantom-typed Distance\<Mm\>/Distance\<Inch\>, FeedRate\<U\>, SpindleSpeed
- [x] **FR-1.2**: Compile-time prevention of unit mixing
- [x] **FR-1.3**: Conversion methods between unit systems

#### FR-2: G-code Parser & Validator (ported from cnc-sender)
- [x] **FR-2.1**: Parse G-code lines into structured GCodeCommand enum
- [x] **FR-2.2**: Validate generated G-code against configurable machine limits
- [x] **FR-2.3**: Support all common G/M codes (G0-G3, G20/21, G90/91, M3-M5, M7-M9)

#### FR-3: Machine Profile System
- [x] **FR-3.1**: MachineType enum (CncMill, LaserCutter) with distinct capabilities
- [x] **FR-3.2**: Profile defines available strategies, axis capabilities, power source
- [x] **FR-3.3**: Default profiles with sensible parameters per machine type
- [x] **FR-3.4**: JSON-serializable for WASM boundary passing

#### FR-4: CNC Mill Profile
- [x] **FR-4.1**: Wraps all existing functionality (spindle, Z-axis, all strategies)
- [x] **FR-4.2**: Output: M3 spindle, Z-axis plunges, coolant control
- [x] **FR-4.3**: Backward compatible - default behavior unchanged

#### FR-5: Laser Cutter Profile
- [x] **FR-5.1**: Dynamic power mode (M4) with S-value power control
- [x] **FR-5.2**: No Z-axis moves (2D only), S0 for rapid traversals
- [x] **FR-5.3**: Supports contour (cut) and raster fill (engrave) strategies
- [x] **FR-5.4**: Multi-pass cutting for thicker materials

#### FR-6: Laser Cut Strategy
- [x] **FR-6.1**: Follow 2D contour paths with configurable laser power
- [x] **FR-6.2**: Multi-pass support (repeat path N times)
- [x] **FR-6.3**: Lead-in overcut for clean edge closure

#### FR-7: Laser Engrave Strategy
- [x] **FR-7.1**: Scanline raster fill of closed paths
- [x] **FR-7.2**: Bidirectional serpentine scanning
- [x] **FR-7.3**: Configurable line spacing (mm or DPI-derived)

#### FR-8: Profile-Aware G-code Emission
- [x] **FR-8.1**: Preamble/postamble per machine profile
- [x] **FR-8.2**: Rapid moves differ by profile (Z retract vs S0 power-off)
- [x] **FR-8.3**: Correct M-codes per machine (M3 vs M4, coolant, etc.)

#### FR-9: WASM API Extensions
- [x] **FR-9.1**: Config JSON accepts machine_type field (backward compatible)
- [x] **FR-9.2**: available_profiles() export returns profile metadata
- [x] **FR-9.3**: default_config(machine_type) export returns defaults

#### FR-10: Web UI Profile Integration
- [x] **FR-10.1**: Machine type selector dynamically shows/hides parameters
- [x] **FR-10.2**: Strategy dropdown filtered by profile capabilities
- [x] **FR-10.3**: Canvas preview adapts to profile (Z-color vs power-color)

### CAM Architecture

```
                    ┌─────────────────┐
                    │  MachineProfile  │
                    │  (CNC/Laser/..)  │
                    └────────┬────────┘
                             │ configures
    ┌────────────────────────┼────────────────────────┐
    │                        │                        │
    ▼                        ▼                        ▼
┌─────────┐          ┌──────────────┐          ┌──────────┐
│  Input   │          │  Strategies  │          │  Output  │
│ STL/SVG  │───────►  │ (filtered by │───────►  │  G-code  │
│ parsers  │          │  profile)    │          │ (profile │
└─────────┘          └──────────────┘          │  aware)  │
                                               └──────────┘
```

---

## Part II: Reactive Dataflow Engine

### Trait Hierarchy (module-traits)

```
Module ─── identity, ports, config, capability queries
  ├── Tick ─────── pure computation (browser sim + codegen)
  ├── SimModel ─── simulated hardware peripherals (WASM sim mode)
  ├── Codegen ──── custom code emission for embedded targets
  └── AnalysisModel ── math model analysis (planned)
```

### Block Types

| Category | Blocks |
|----------|--------|
| Math | constant, gain, add, multiply, clamp |
| Logic | state_machine (FSM with guarded transitions) |
| Serde | json_encode, json_decode |
| I/O | udp_source, udp_sink, pubsub_source, pubsub_sink |
| Embedded | adc_source, pwm_sink, gpio_in, gpio_out, uart_tx, uart_rx, encoder |
| Display/Motion | ssd1306_display, tmc2209_stepper, tmc2209_stallguard |
| Visualization | plot |

### Multi-Target Code Generation

Three codegen backends:

1. **Rust emit** (legacy) — string-interpolation Rust code generation (`emit.rs`, `targets/`)
2. **MLIR text** (tier 1) — textual `.mlir` via string concatenation → mlir-opt/mlir-translate → C
3. **Typed IR** (tier 2) — `IrOpKind` enum AST → `emit_rust.rs` safe Rust / `printer.rs` MLIR text

```
GraphSnapshot ──► partition.rs ──► per-target subgraphs
                                        │
                    ┌───────────────────┼───────────────────┐
                    │                   │                   │
                    ▼                   ▼                   ▼
              Rust emit            MLIR lower         MLIR lower
              (Embassy)            → .mlir             → .mlir
                 │                   │                   │
                 ▼                   ▼                   ▼
              firmware            mlir-opt            mlir-opt
              (.rs)               → .c/.h             → .c/.h
```

### Supported Targets

| Target | MCU | Framework | Thumb Target |
|--------|-----|-----------|-------------|
| Host | x86/ARM | std | native |
| RP2040 | Cortex-M0+ | embassy-rp | thumbv6m-none-eabi |
| STM32F4 | Cortex-M4F | embassy-stm32 | thumbv7em-none-eabihf |
| STM32G0B1 | Cortex-M0+ | embassy-stm32 | thumbv6m-none-eabi |
| ESP32-C3 | RISC-V | esp-hal | riscv32imc-unknown-none-elf |

### Multi-MCU Partitioning

`partition_graph()` splits a single GraphSnapshot into per-target subgraphs. Cross-partition channels are replaced with pub/sub bridge pairs using deterministic topic names.

```
┌─────────────┐  topic: "bridge_adc_0_out"  ┌─────────────┐
│  RP2040      │ ──── pubsub_sink ──────────► │  STM32F4     │
│  (ADC block) │                   pubsub_src │  (PID block) │
└─────────────┘                               └─────────────┘
```

### MCU Inventory (module-traits/inventory.rs)

Digital twin of each MCU, mirroring CubeMX:
- `McuDef`: CPU core, clock tree (HSI/HSE/PLL), memory map, peripheral instances
- `PinDef`: GPIO pins with alternate-function muxing
- `PeripheralInst`: DMA channels, interrupt numbers
- Pre-built definitions for all supported targets

### Deployment Manifest (module-traits/deployment.rs)

Links control logic IR to hardware topology:
- `SystemTopology`: MCU nodes + physical links (CAN, RS485, SPI, UART, I2C, Ethernet, WiFi)
- `TaskBinding`: Sub-graphs assigned to nodes with scheduling frequency/priority
- `ChannelBinding`: Logical signals → intra-node memory or inter-node pub/sub
- `PeripheralBinding`: Block ports → specific MCU peripherals and pins

### Hardware Configuration (module-traits/hardware.rs)

Three-layer model:
1. **PeripheralRequirement**: What the graph needs (extracted from blocks)
2. **TargetCapabilities**: What each MCU can provide
3. **HardwareConfig**: User's mapping from logical channels to physical pins

---

## Part III: Expression DAG (dag-core)

Compact `no_std` expression DAG for signal processing, distinct from the dataflow graph:

- `Op` enum: Const, Input, Output, Add, Mul, Sub, Div, Pow, Neg, Relu, Subscribe, Publish
- `Dag` struct: Vec-based with forward-reference validation
- CBOR serialization for embedded transport
- Block templates for common patterns (constant, gain, add, clamp, adc, pwm)
- Evaluator with topological ordering

---

## Part IV: Pub/Sub (pubsub)

`no_std` message broker for inter-MCU communication:

- `NodeAddr`: Hierarchical addressing (bus, device, endpoint)
- `TopicId`: Hashed topic identifier
- `Frame`: 17-byte binary envelope + CBOR payload
- Transport trait with UDP and embassy-net backends
- `CompositeTransport` for dual-bus bridge nodes

---

## Part V: Hardware-in-the-Loop (hil/)

### Board Support

| Crate | Target | Features |
|-------|--------|----------|
| board-support-pico2 | RP2350 | DAG runtime, USB CDC-NCM, WebSocket, HTTP API |
| board-support-pico | RP2040 | DAP/SWD firmware |
| board-support-stm32 | STM32 | Embassy firmware |
| board-support-pi-zero | Pi Zero | Host-side I2C bridge |

### HIL Infrastructure

- **hil-backplane**: UDP multicast message envelope + pub/sub transport
- **dap-dispatch**: CMSIS-DAP v2 CBOR protocol
- **i2c-hil-sim**: Simulated I2C buses (no hardware required)
- **i2c-hil-devices**: Simulated I2C device models (INA230, LTC4287, ADM1272)
- **hil-firmware-support**: Shared USB builder, WebSocket server, HTTP API handler

### Pico2 HTTP API

| Method | Path | Purpose |
|--------|------|---------|
| POST | /api/dag | Deploy CBOR-encoded DAG |
| POST | /api/tick | Evaluate DAG once |
| GET | /api/pubsub | Snapshot all topic values |
| GET | /api/channels | List registered I/O channels |
| GET | /api/status | DAG status (loaded/nodes/ticks) |
| POST | /api/debug | Toggle debug topic publishing |

---

## Part VI: MLIR Codegen Pipeline (mlir-codegen)

Three parallel IR representations generated from the same `GraphSnapshot`:

```
GraphSnapshot (JSON)
  ├─ Tier 1: lower.rs → .mlir text → mlir-opt → mlir-translate → .c/.h
  ├─ Tier 2: ir.rs    → IrModule (typed AST) → printer.rs → .mlir
  │                                           → emit_rust.rs → safe Rust
  └─ Tier 3: runtime.rs → BlockFn enum → DagRuntime (in-MCU execution)
```

### Typed IR Dialect System (ir.rs)

Operations use dialect-namespaced Rust enums (`IrOpKind`), not strings:

| Dialect | Enum | Ops | MLIR Standard |
|---------|------|-----|---------------|
| `arith` | `ArithOp` | `Constant`, `Addf`, `Mulf`, `Subf`, `Select` | Yes — standard MLIR arith |
| `func` | `FuncOp` | `Call { callee }` | Yes — standard MLIR func |
| `dataflow` | `DataflowOp` | `Clamp`, `AdcRead`, `PwmWrite`, `GpioRead`, `GpioWrite`, `UartRx`, `UartTx`, `EncoderRead` | No — custom hardware I/O |

**Pub/sub as function calls**: Subscribe/publish are modeled as `FuncOp::Call { callee: "subscribe"/"publish" }` with a topic attribute, aligning with MLIR's `func.call @symbol` pattern.

**Stepper/stallguard excluded from IR**: Motor control ops are custom message structs over channels, not language-level IR operations. They exist only in the runtime tier (`BlockFn::Stepper`, `BlockFn::StallGuard`).

See [mlir-codegen/MLIR.md](mlir-codegen/MLIR.md) for the full dialect reference.

### Runtime (mlir-codegen/runtime.rs)

- `BlockFn` enum: Captures config via partial application (19 variants including Stepper, StallGuard)
- `DagRuntime`: Deserialize graph JSON, topologically sort, build tickable object with flat `Vec<f64>` state
- `HardwareBridge` trait: Hardware abstraction (ADC, PWM, GPIO, UART, Encoder, Stepper, PubSub)
- `NullHardware`: No-op bridge for pure-logic testing

---

## Multi-Frontend Architecture

Three frontend paradigms share the same `GraphSnapshot` IR and codegen pipeline:

```
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  Block Editor    │  │  Python API     │  │  Jupyter         │
│  (DOM/SVG)       │  │  (Keras-style)  │  │  Notebook        │
│  Browser/WASM    │  │  PyO3 bindings  │  │  IPython display │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                     │
         ▼                    ▼                     ▼
    ┌─────────────────────────────────────────────────────┐
    │              GraphSnapshot JSON IR                   │
    └──────────────────────┬──────────────────────────────┘
                           │
                           ▼
    ┌─────────────────────────────────────────────────────┐
    │              Codegen Engine                           │
    │   generate_workspace(snapshot, dt, targets)           │
    └──────┬──────────┬──────────┬──────────┬─────────────┘
           │          │          │          │
           ▼          ▼          ▼          ▼
        Host      RP2040     STM32F4    ESP32-C3
      (std bin)  (embassy)  (embassy)   (esp-hal)
```

### Frontend Design Documents

- **[Block Editor](docs/block-frontend.md)** — DOM/SVG visual editor
- **[Python API](docs/api-frontend.md)** — Keras-style programmatic graph building via PyO3
- **[Jupyter Notebook](docs/notebook-frontend.md)** — IPython display integration
- **[GraphSnapshot Schema](docs/graph-snapshot-schema.md)** — formal IR specification

---

## Part VII: Configurable Blocks (configurable-blocks)

Higher-order blocks with schema-driven configuration, topic-based wiring, and DAG lowering.

### ConfigurableBlock Trait

```rust
pub trait ConfigurableBlock {
    fn block_type(&self) -> &str;
    fn display_name(&self) -> &str;
    fn category(&self) -> BlockCategory;
    fn config_schema(&self) -> Vec<ConfigField>;
    fn config_json(&self) -> serde_json::Value;
    fn apply_config(&mut self, config: &serde_json::Value);
    fn declared_channels(&self) -> Vec<DeclaredChannel>;
    fn lower(&self) -> Result<LowerResult, DagError>;
}
```

### Block Palette (16 blocks)

| Category | Blocks |
|----------|--------|
| Math | Constant, Gain, Add, Subtract, Multiply, Clamp, Negate, Map/Scale, Low-Pass Filter |
| Control | PID Controller |
| I/O | ADC Input, PWM Output |
| PubSub | Subscribe, Publish, PubSub Bridge |

### Deployment Codegen

`codegen::generate_all_crates(manifest, dag)` produces complete Embassy firmware crates:
- `Cargo.toml` with correct Embassy HAL per MCU family
- `.cargo/config.toml` with target triple + probe-rs runner
- `memory.x` linker script from MCU inventory memory regions
- `src/main.rs` with Embassy async main, CBOR-embedded DAG, ticker loop
- All generated code is `#![forbid(unsafe_code)]`

### Deployment Profile

`DeploymentProfile` maps logical channels to physical hardware:
- `ChannelMap`: Remap pubsub topic names for deployment context
- `node_assignments`: Block → MCU node mapping for multi-board deployments
- `boards`: List of `(node_id, mcu_family)` pairs
- Validation: missing peripheral bindings, unknown MCU families

---

## Part VIII: Combined Frontend (hil/combined-frontend)

Single Leptos 0.7 CSR app consolidating HIL dashboard + DAG editor + deployment panel. Served from MCU flash or native-server.

### Architecture

- **All safe Rust** — `#![forbid(unsafe_code)]`, no C FFI
- **Shared types** — `module-traits` and `dag-core` types used directly (no TS mirror types)
- **Signal-based state** — `WriteSignal<Vec<Request>>` request queue, `AppContext` via Leptos context
- **In-browser evaluation** — `dag.evaluate()` runs in WASM, no MCU needed

### Tabs

| Tab | Purpose |
|-----|---------|
| Buses | I2C bus topology and device enumeration |
| Telemetry | Temperature, power, fan readings (2s polling) |
| I2C Console | Manual read/write with transaction log |
| Firmware | OTA update with ack-based flow control |
| DAG Editor | Block palette, SVG canvas, config panel, evaluate + deploy |
| Deploy | Generate firmware crates from deployment config |

### Build Pipeline

```
trunk build --release → dist/ (WASM + JS + CSS)
    ↓
board-support-pico2/build.rs → gzip → include_bytes!
    ↓
firmware binary with embedded frontend (~180 KB gzipped)
```

---

## Part IX: Native Server (hil/native-server)

Host-side development server matching the Pico2 HTTP + WebSocket API:

- Axum HTTP server serving `www-cam/` and `www-dataflow/` frontends
- DAG API: POST /api/dag, POST /api/tick, GET /api/pubsub, GET /api/status
- WebSocket CBOR: I2C bus commands (tags 1-35) with simulated buses
- Mock HAL: `WasmSimPeripherals` with I2C, serial, socket simulation
- CLI: `cargo run -p native-server -- --port 3000 --www-dir www-cam`

---

## CI / Build System

### Single Source of Truth

CI uses Makefile targets directly — no duplicated commands:

```yaml
- run: make lint    # cargo clippy with WORKSPACE_EXCLUDES
- run: make test    # cargo llvm-cov with coverage threshold
- run: make wasm    # wasm-pack for rustcam + rustsim
- run: make ts      # npm typecheck + test + build for both frontends
```

### Crate Structure

```
crates/rustcam/     — CAM WASM (STL/SVG → G-code)
crates/rustsim/     — Dataflow WASM (block editor, simulation)
www-cam/            — CAM TypeScript frontend
www-dataflow/       — Dataflow TypeScript frontend
```

---

## Non-Goals (Out of Scope)
- Plasma cutter profile
- Machine communication / serial connection
- DXF/STEP/3MF import formats
- Image-to-raster engraving (grayscale bitmap input)
- Multi-tool operations in single job
- 5-axis toolpaths
- Material database / feeds & speeds calculator
