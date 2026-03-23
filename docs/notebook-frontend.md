# Frontend 3: Jupyter Notebook Integration

**URL:** `/cam/notebook/` (landing page with example notebooks and Binder links)

## Overview

Builds on top of the Python API (Frontend 2) to provide rich inline visualization of dataflow graphs in Jupyter notebooks. Uses IPython display hooks for automatic rendering — no extra widgets required.

## Target Audience

- Researchers documenting control system experiments
- Educators building interactive course materials
- Documentation-heavy workflows where graphs and signals live alongside narrative text

## Architecture

```
┌──────────────────────────────────────────┐
│  Jupyter Notebook                         │
│                                           │
│  ┌───────────────┐   ┌────────────────┐  │
│  │ rustcam.Graph  │──▶│ IPython display│  │
│  │ (Python API)   │   │ _repr_svg_()   │  │
│  └───────┬───────┘   └────────────────┘  │
│          │                                │
│  ┌───────┴───────┐   ┌────────────────┐  │
│  │ show_graph()   │   │ show_signals() │  │
│  │ → SVG block    │   │ → matplotlib   │  │
│  │   diagram      │   │   plot         │  │
│  └───────────────┘   └────────────────┘  │
│                                           │
│  ┌───────────────────────────────────┐   │
│  │ show_codegen() → syntax-highlighted│   │
│  │ Rust code output                   │   │
│  └───────────────────────────────────┘   │
└──────────────────────────────────────────┘
```

The notebook frontend is a pure Python layer — no Rust/WASM in the notebook itself. It depends on `rustcam` (the Python API) for all graph operations and adds visualization.

## Display Integration

### Automatic SVG Rendering

The `Graph` class implements `_repr_svg_()` so Jupyter auto-renders it:

```python
from rustcam import Graph, Constant, Gain, PWM

g = Graph(dt=0.01)
c = g.add(Constant(value=5.0))
amp = g.add(Gain(factor=2.0))
out = g.add(PWM(channel=0))
g.connect(c.out, amp.input)
g.connect(amp.out, out.duty)

g  # → displays SVG block diagram inline
```

The SVG renderer reads the `GraphSnapshot` and draws:
- Rounded rectangles for blocks, color-coded by category
- Port circles on left (inputs) and right (outputs) edges
- Cubic bezier curves for channels
- Block name and type labels
- Output values as small annotations when available

### Explicit Display Functions

```python
from rustcam.notebook import show_graph, show_signals, show_codegen

# Show graph with custom options
show_graph(g, width=800, show_values=True)

# Run simulation and plot signals over time
g.run(steps=500)
show_signals(g, blocks=[amp, out], ports=["out", "duty"])
# → matplotlib figure with time-series traces

# Show generated code with syntax highlighting
show_codegen(g, target="rp2040")
# → syntax-highlighted Rust code in a notebook output cell
```

### `show_graph(graph, **kwargs)`

Renders the graph as an SVG block diagram.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `graph` | `Graph` | required | The graph to render |
| `width` | `int` | `600` | SVG viewport width in pixels |
| `show_values` | `bool` | `False` | Annotate ports with last output values |
| `highlight` | `list[int]` | `[]` | Block ids to highlight |

### `show_signals(graph, blocks, ports, **kwargs)`

Plots signal traces using matplotlib.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `graph` | `Graph` | required | The graph (must have been run) |
| `blocks` | `list` | required | Block handles to plot |
| `ports` | `list[str]` | `["out"]` | Port names to plot per block |
| `last_n` | `int` | `None` | Only show last N ticks |

Requires `matplotlib` as an optional dependency.

### `show_codegen(graph, target, **kwargs)`

Displays generated Rust code with syntax highlighting.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `graph` | `Graph` | required | The graph to compile |
| `target` | `str` | `"host"` | Target platform |
| `file` | `str` | `None` | Show only this file (e.g. `"src/main.rs"`) |

Uses IPython's `display(Code(...))` for syntax highlighting, falling back to `<pre>` blocks.

## SVG Block Diagram Renderer

The SVG renderer is a pure-Python function that converts a `GraphSnapshot` dict into an SVG string. Layout algorithm:

1. Topological sort of blocks (sources left, sinks right)
2. Assign column by topological depth
3. Within each column, space blocks vertically
4. Route edges as cubic bezier curves between port positions

Category colors match the block editor:

| Category | Color |
|----------|-------|
| Sources | `#4CAF50` (green) |
| Math | `#2196F3` (blue) |
| Sinks | `#FF9800` (orange) |
| Serde | `#9C27B0` (purple) |
| I/O | `#00BCD4` (cyan) |
| Embedded | `#F44336` (red) |
| Logic | `#795548` (brown) |

## Future: JupyterLab Widget

A potential future extension embeds the full block editor (Frontend 1) as a JupyterLab widget:

```python
from rustcam.widget import DataflowWidget

w = DataflowWidget(g)
display(w)  # interactive block editor inside JupyterLab
# Edits in the widget sync back to the Python Graph object
```

This would use `anywidget` to embed the same TypeScript/WASM block editor inside a notebook cell. Out of scope for the initial release — the SVG rendering provides sufficient notebook visualization.

## Package Structure

The notebook integration lives alongside the Python API:

```
py/
  rustcam/
    __init__.py       # Graph, block classes
    graph.py
    blocks.py
    codegen.py
    notebook.py       # show_graph, show_signals, show_codegen
    _svg.py           # SVG block diagram renderer
```

Optional dependencies:
```toml
[project.optional-dependencies]
notebook = ["matplotlib>=3.5", "ipython>=7.0"]
widget = ["anywidget>=0.9"]
```

## Shared IR Contract

All notebook visualizations read from `GraphSnapshot` as defined in [graph-snapshot-schema.md](graph-snapshot-schema.md). The `_repr_svg_()` method calls `graph.snapshot()` and renders the result — no separate data model.
