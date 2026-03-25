# webCAM

**CNC toolpath generation that runs entirely in your browser.**

Desktop CAM software is heavy, expensive, and platform-locked. webCAM compiles
Rust to WebAssembly so you get STL/SVG → G-code conversion with zero installs,
zero server calls, and zero cost.

**[Try it live →](https://jvishnefske.github.io/cam)**

## What it does

```
 Drop a file         Pick a strategy        Get G-code
┌───────────┐      ┌───────────────┐      ┌───────────┐
│  .stl     │ ──→  │  Contour      │ ──→  │  .nc file │
│  .svg     │      │  Pocket       │      │  copy or  │
│  sketch   │      │  Slice        │      │  download │
└───────────┘      │  Zigzag       │      └───────────┘
                   │  Laser cut    │
                   └───────────────┘
```

- **3D meshes** (STL) — slice into layers, generate surface and contour paths
- **2D vectors** (SVG) — profile cuts, pocket clearing, laser engraving
- **Built-in sketcher** — draw constrained 2D geometry and send it straight to CAM
- **Toolpath simulation** — watch the toolhead trace the path before you cut
- **Dataflow editor** — wire up signal-processing blocks for custom workflows
- **HIL testing** — hardware-in-the-loop I2C simulation and firmware for embedded targets

## Quick start

```bash
make test            # run unit tests
make wasm            # build WASM (requires wasm-pack 0.12+)
make serve           # http://localhost:8080
```

## Microcontroller targets

The `hil/` directory contains crates for hardware-in-the-loop testing and embedded firmware. Each board-support crate targets a specific microcontroller:

| Target | Crate | MCU |
|--------|-------|-----|
| **Pico** | `board-support-pico` | RP2040 |
| **Pico 2** | `board-support-pico2` | RP2350 |
| **STM32** | `board-support-stm32` | STM32 (via embassy-stm32) |
| **Pi Zero** | `board-support-pi-zero` | BCM2835 (Linux) |

Supporting crates:

- `i2c-hil-sim` — software I2C bus simulator with pluggable device models
- `i2c-hil-devices` — simulated I2C peripherals (temp sensors, GPIO expanders, PMBus, EEPROMs)
- `hil-backplane` — message framing, DHCP, pub/sub, and request/response over USB or network
- `board-config-common` — shared bus topology and device configuration across all boards
- `hil-firmware-support` — common firmware utilities (USB setup, task spawning)
- `hil-frontend` — Leptos web UI for live HIL monitoring
- `usb-composite-dispatchers`, `usb-can-dispatcher`, `usb-gpio-dispatcher` — USB class dispatchers (composite, CAN, GPIO)
- `dap-dispatch` — CMSIS-DAP debug probe dispatcher
- `pico-bootloader` — UF2 bootloader for RP2040

```bash
make hil-test        # run host-side HIL tests
make hil-firmware    # build Pico firmware
make hil-stm32      # build STM32 firmware
make hil-pi-zero    # build Pi Zero support
make hil-verify     # clippy + test + firmware
make all             # full CI + HIL verification
```

## How the pipeline works

Four layers, each behind a trait boundary. Extend any layer without touching the others:

| Layer | Does | Extend with |
|-------|------|-------------|
| **Input** | Parse STL, SVG, sketch | OBJ, STEP, DXF |
| **Geometry** | Mesh, polylines, toolpaths | NURBS, T-splines |
| **Strategy** | Contour, pocket, slice, zigzag, laser | Trochoidal, adaptive |
| **Output** | G-code emitter | HPGL, Marlin, GRBL |

## License

MIT
