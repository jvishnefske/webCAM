# Rust webCAM

Browser-based computer-aided manufacturing tool. Load an **STL** (3-D mesh) or
**SVG** (2-D vector) file and generate **G-code** — all in WebAssembly, no
server required.

Inspired by [PyCAM](https://pycam.sourceforge.net/); reference implementation
in Rust targeting `wasm32-unknown-unknown`.

## Swiss Cheese Architecture

The processing pipeline is four independent layers with explicit extension
points ("holes") where new functionality plugs in without touching existing
code:

```
Input ──> Geometry ──> Strategy ──> Output
(STL,SVG)  (Mesh,Paths)  (Contour,    (G-code)
                          Pocket,
 +OBJ,3MF   +NURBS       Slice)       +HPGL
 +STEP,DXF  +T-spline   +trochoidal   +Marlin
                         +adaptive     +GRBL
```

Each layer is a Rust module behind a trait boundary.

## Quick start

```bash
# Run unit tests
make test

# Build WASM (requires wasm-pack)
make wasm

# Serve locally
make serve       # http://localhost:8080

# Package for release
make release     # dist/rustcam.zip
```

### Requirements

| Tool | Version |
|------|---------|
| Rust | stable  |
| wasm-pack | 0.12+ |
| Python 3 | (for `make serve` only) |

## Usage

1. Open the app in a browser (or unzip the release and open `index.html`).
2. Drop an `.stl` or `.svg` file onto the drop zone.
3. Select a machining strategy (Contour / Pocket / Slice).
4. Adjust tool diameter, feed rate, step-down, and other parameters.
5. Click **Generate G-code**.
6. Copy or download the `.nc` file.

## Modules

| Module | File | Purpose |
|--------|------|---------|
| `geometry` | `src/geometry.rs` | Core types: Vec3, Vec2, Triangle, Mesh, Polyline, Toolpath |
| `stl` | `src/stl.rs` | Binary + ASCII STL parser |
| `svg` | `src/svg.rs` | SVG path / rect / circle / polygon parser |
| `slicer` | `src/slicer.rs` | 3-D mesh to 2-D contour slicing |
| `toolpath` | `src/toolpath.rs` | Contour and pocket toolpath strategies |
| `gcode` | `src/gcode.rs` | G-code emitter |
| `lib` | `src/lib.rs` | WASM entry points, pipeline orchestration |

## CI / Release / Pages

GitHub Actions runs tests on every push. On pushes to `main` (or tags),
the WASM bundle is built and deployed to **GitHub Pages** automatically.

Live site: **https://jvishnefske.github.io/cam**

To enable Pages for `/cam`:
1. Create an empty repo `jvishnefske/cam` on GitHub.
2. Go to **Settings > Pages** and set source to **GitHub Actions**.
3. Push to `main` here — the deploy job publishes to Pages.

Tag a version (`v0.1.0`) to also create a GitHub Release with the
`rustcam.zip` download.

## License

MIT
