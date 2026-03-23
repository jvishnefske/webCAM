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

## Quick start

```bash
make test            # run unit tests
make wasm            # build WASM (requires wasm-pack 0.12+)
make serve           # http://localhost:8080
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
