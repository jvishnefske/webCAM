# RustCAM Design: CAD/CAM MVP with Machine Tool Profiles

## Overview

Browser-based CAD/CAM tool running entirely in WASM. Supports multiple machine types
(CNC mill, laser cutter) through a profile system that adapts parameters, strategies,
and G-code output per machine. Ports useful Rust code from the cnc-sender project.

## Functional Requirements

### FR-1: Type-Safe Unit System (ported from cnc-sender)
- [x] **FR-1.1**: Phantom-typed Distance<Mm>/Distance<Inch>, FeedRate<U>, SpindleSpeed
- [x] **FR-1.2**: Compile-time prevention of unit mixing
- [x] **FR-1.3**: Conversion methods between unit systems

### FR-2: G-code Parser & Validator (ported from cnc-sender)
- [x] **FR-2.1**: Parse G-code lines into structured GCodeCommand enum
- [x] **FR-2.2**: Validate generated G-code against configurable machine limits
- [x] **FR-2.3**: Support all common G/M codes (G0-G3, G20/21, G90/91, M3-M5, M7-M9)

### FR-3: Machine Profile System
- [x] **FR-3.1**: MachineType enum (CncMill, LaserCutter) with distinct capabilities
- [x] **FR-3.2**: Profile defines available strategies, axis capabilities, power source
- [x] **FR-3.3**: Default profiles with sensible parameters per machine type
- [x] **FR-3.4**: JSON-serializable for WASM boundary passing

### FR-4: CNC Mill Profile
- [x] **FR-4.1**: Wraps all existing functionality (spindle, Z-axis, all strategies)
- [x] **FR-4.2**: Output: M3 spindle, Z-axis plunges, coolant control
- [x] **FR-4.3**: Backward compatible - default behavior unchanged

### FR-5: Laser Cutter Profile
- [x] **FR-5.1**: Dynamic power mode (M4) with S-value power control
- [x] **FR-5.2**: No Z-axis moves (2D only), S0 for rapid traversals
- [x] **FR-5.3**: Supports contour (cut) and raster fill (engrave) strategies
- [x] **FR-5.4**: Multi-pass cutting for thicker materials

### FR-6: Laser Cut Strategy
- [x] **FR-6.1**: Follow 2D contour paths with configurable laser power
- [x] **FR-6.2**: Multi-pass support (repeat path N times)
- [x] **FR-6.3**: Lead-in overcut for clean edge closure

### FR-7: Laser Engrave Strategy
- [x] **FR-7.1**: Scanline raster fill of closed paths
- [x] **FR-7.2**: Bidirectional serpentine scanning
- [x] **FR-7.3**: Configurable line spacing (mm or DPI-derived)

### FR-8: Profile-Aware G-code Emission
- [x] **FR-8.1**: Preamble/postamble per machine profile
- [x] **FR-8.2**: Rapid moves differ by profile (Z retract vs S0 power-off)
- [x] **FR-8.3**: Correct M-codes per machine (M3 vs M4, coolant, etc.)

### FR-9: WASM API Extensions
- [x] **FR-9.1**: Config JSON accepts machine_type field (backward compatible)
- [x] **FR-9.2**: available_profiles() export returns profile metadata
- [x] **FR-9.3**: default_config(machine_type) export returns defaults

### FR-10: Web UI Profile Integration
- [x] **FR-10.1**: Machine type selector dynamically shows/hides parameters
- [x] **FR-10.2**: Strategy dropdown filtered by profile capabilities
- [x] **FR-10.3**: Canvas preview adapts to profile (Z-color vs power-color)

## Architecture

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

### Source Files from cnc-sender to Port
- `cnc-types/src/units.rs` → `src/units.rs` (phantom type units)
- `cnc-gcode/src/parser.rs` → `src/gcode_parser.rs` (G-code parsing)
- `cnc-gcode/src/validator.rs` → `src/gcode_validator.rs` (validation)

## Implementation Checklist

### Phase 1: Foundation (parallel)
- [x] task-001: Port type-safe unit system from cnc-sender → FR-1
- [x] task-002: Port G-code parser and validator → FR-2
- [x] task-003: Define MachineProfile and MachineType → FR-3

### Phase 2: Profiles (parallel after Phase 1)
- [x] task-004: CNC mill profile wrapping existing behavior → FR-4
- [x] task-005: Laser cutter profile → FR-5

### Phase 3: Strategies & Emitter (parallel after Phase 2)
- [x] task-006: Profile-aware G-code emitter → FR-8
- [x] task-007: Laser cut strategy → FR-6
- [x] task-008: Laser engrave strategy → FR-7

### Phase 4: WASM API
- [x] task-009: WASM API profile selection → FR-9

### Phase 5: UI & Testing (parallel after Phase 4)
- [x] task-010: Web UI profile selector → FR-10
- [x] task-011: Profile-aware canvas preview → FR-10.3
- [x] task-012: End-to-end integration tests

### Phase 6: Validation
- [x] task-013: Post-generation G-code validation → FR-2.2

## Traceability Matrix

| Requirement | Tasks |
|-------------|-------|
| FR-1 (Units) | task-001 |
| FR-2 (Parser) | task-002, task-013 |
| FR-3 (Profiles) | task-003 |
| FR-4 (CNC Mill) | task-004 |
| FR-5 (Laser) | task-005 |
| FR-6 (Laser Cut) | task-007 |
| FR-7 (Laser Engrave) | task-008 |
| FR-8 (G-code Emit) | task-006 |
| FR-9 (WASM API) | task-009 |
| FR-10 (UI) | task-010, task-011 |

## Non-Goals (Out of Scope for MVP)
- Plasma cutter profile (future: port cnc-plasma layer system)
- Machine communication / serial connection (stays in cnc-sender)
- DXF/STEP/3MF import formats
- Image-to-raster engraving (grayscale bitmap input)
- Multi-tool operations in single job
- 5-axis toolpaths
- Material database / feeds & speeds calculator

---

## Multi-Frontend Model-Based Development

RustCAM's reactive dataflow engine supports three frontend paradigms — all sharing the same `GraphSnapshot` IR and codegen pipeline. No server required.

### Architecture

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
    │         (universal contract — see schema)            │
    └──────────────────────┬──────────────────────────────┘
                           │
                           ▼
    ┌─────────────────────────────────────────────────────┐
    │              Rust Codegen Engine                      │
    │     generate_workspace(snapshot, dt, targets)         │
    └──────┬──────────┬──────────┬──────────┬─────────────┘
           │          │          │          │
           ▼          ▼          ▼          ▼
        Host      RP2040     STM32F4    ESP32-C3
      (std bin)  (embassy)  (embassy)   (esp-hal)
```

### Approach: Hybrid (WASM + PyO3)

- **Browser**: WASM via `wasm-bindgen` powers the block editor (existing)
- **Python**: PyO3 bindings wrap the same Rust `DataflowGraph` for the API and notebook frontends
- **Contract**: `GraphSnapshot` JSON is the interchange format — any frontend can produce/consume it
- **No server**: both WASM and PyO3 run the engine locally, no network round-trips

### Market Positioning

| Capability | RustCAM | Simulink | Keras | Node-RED |
|-----------|---------|----------|-------|----------|
| Visual block editor | Yes | Yes | No | Yes |
| Programmatic API | Yes (Python) | Yes (MATLAB) | Yes | Limited |
| Notebook integration | Yes (Jupyter) | No | Yes | No |
| Embedded codegen | Yes (Rust, multi-target) | Yes (C) | No | No |
| Runs in browser | Yes (WASM) | No | No | Yes (server) |
| Open source | Yes | No | Yes | Yes |
| No server required | Yes | N/A | Yes | No |

### URL Structure

Each frontend is served at its own path on `jvishnefske.github.io/cam/`. One user sees one frontend at a time — no shared mode switcher.

| Path | Frontend | Content |
|------|----------|---------|
| `/cam/` | CAM Tool | STL/SVG → G-code (existing) |
| `/cam/blocks/` | Block Editor | Visual dataflow editor (WASM) |
| `/cam/api/` | Python API | Landing page, install instructions, examples |
| `/cam/notebook/` | Jupyter Notebook | Landing page, example notebooks, Binder links |

Each path has its own `index.html`. A minimal nav bar links between them, but each page is self-contained — no tabbed mode switcher.

The existing single-page mode switcher (CAM / 2D Sketch / Dataflow) in `www/index.html` will be split into separate entry points during the multi-frontend migration.

### Frontend Design Documents

- **[Block Editor](docs/block-frontend.md)** — DOM/SVG visual editor (~70% built on `feature-dataflow`)
- **[Python API](docs/api-frontend.md)** — Keras-style programmatic graph building via PyO3
- **[Jupyter Notebook](docs/notebook-frontend.md)** — IPython display integration with SVG and matplotlib
- **[GraphSnapshot Schema](docs/graph-snapshot-schema.md)** — formal IR specification (the contract all frontends share)
