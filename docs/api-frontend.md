# Frontend 2: Python API (Keras-Style Programmatic Interface)

**URL:** `/cam/api/` (landing page with install instructions and examples)

## Overview

A Python package (`rustcam`) that lets users build dataflow graphs programmatically, simulate them, and compile to embedded targets — all without a browser. The API follows the Keras pattern: define blocks, connect them, "compile" to a target.

## Target Audience

- ML engineers familiar with Keras/PyTorch model-building patterns
- Data scientists building sensor processing pipelines
- Automation engineers scripting control system design
- CI/CD pipelines that need headless graph validation and codegen

## Architecture

The Python API wraps the same Rust `DataflowGraph` via PyO3, sharing identical graph logic and codegen with the block editor.

```
┌──────────────────────────────────┐
│  Python                          │
│                                  │
│  from rustcam import Graph, ADC, │
│    Gain, PWM                     │
│                                  │
│  ┌─────────────┐                 │
│  │ rustcam pkg  │                │
│  │ (Python API) │                │
│  └──────┬──────┘                 │
│         │ PyO3 FFI               │
│  ┌──────┴──────────┐            │
│  │ py-binding crate │            │
│  │ (Rust + PyO3)    │            │
│  └──────┬──────────┘            │
│         │                        │
│  ┌──────┴──────────┐            │
│  │ rustcam core     │            │
│  │ DataflowGraph    │            │
│  │ + Codegen        │            │
│  └─────────────────┘            │
└──────────────────────────────────┘
```

### Crate Structure

```
py-binding/
  Cargo.toml          # PyO3 crate depending on rustcam core
  src/
    lib.rs            # #[pymodule] wrapping DataflowGraph, codegen

py/
  rustcam/
    __init__.py       # Re-exports Graph, block classes
    graph.py          # Graph class (wraps PyO3 binding)
    blocks.py         # Block classes with named ports
    codegen.py        # compile() and export helpers
  pyproject.toml      # maturin build config
```

## API Design

### Block Classes

Each block type maps to a Python class with named port accessors:

```python
from rustcam import Graph, Constant, ADC, Gain, Clamp, PWM

g = Graph(dt=0.01)

# Sources
sensor = g.add(ADC(channel=0))
setpoint = g.add(Constant(value=3.3))

# Processing
error = g.add(Gain(factor=-1.0))
g.connect(setpoint.out, error.input)       # named ports
g.connect(sensor.out, error.input_b)

limiter = g.add(Clamp(min=0.0, max=1.0))
g.connect(error.out, limiter.input)

# Sink
actuator = g.add(PWM(channel=0))
g.connect(limiter.out, actuator.duty)
```

### Port Naming Convention

Blocks expose ports as attributes on the returned handle:

```python
handle = g.add(Gain(factor=2.0))
handle.input   # → PortRef(block_id=3, port_index=0, direction="in")
handle.out     # → PortRef(block_id=3, port_index=0, direction="out")
```

Port names match the `PortDef.name` field from the Rust block trait. Multi-input blocks like `Add` expose `a` and `b`.

### Simulation

```python
# Batch simulation (non-realtime)
g.run(steps=1000)
print(g.time)         # 10.0
print(g.tick_count)   # 1000

# Access block outputs
print(actuator.out.value)   # last output value

# Snapshot as dict
snap = g.snapshot()   # returns GraphSnapshot as Python dict
```

### Codegen

```python
from rustcam import compile

# Single target
files = compile(g, target="rp2040", bindings={
    "adc_0": "ADC channel 0 on GPIO26",
    "pwm_0": "PWM slice 0 on GPIO16",
})

# Write to disk
for path, content in files:
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(content)

# Multi-target workspace
workspace = compile(g, targets=[
    {"target": "host"},
    {"target": "rp2040", "binding": {...}},
    {"target": "esp32c3", "binding": {...}},
])
```

### Import/Export

```python
# Export to GraphSnapshot JSON (interoperable with block editor)
json_str = g.to_json()
Path("my_graph.json").write_text(json_str)

# Load from JSON
g2 = Graph.from_json(Path("my_graph.json").read_text())
```

## PyO3 Binding Layer

The `py-binding` crate exposes a thin PyO3 wrapper around the same types the WASM API uses:

| Python method | Rust function |
|--------------|---------------|
| `Graph.__init__(dt)` | `DataflowGraph::new()` + `Scheduler::new(dt)` |
| `Graph.add(block)` | `create_block()` + `graph.add_block()` |
| `Graph.connect(src, dst)` | `graph.connect()` |
| `Graph.disconnect(ch)` | `graph.disconnect()` |
| `Graph.run(steps)` | `graph.run()` |
| `Graph.snapshot()` | `graph.snapshot()` → serde to Python dict |
| `compile(graph, ...)` | `codegen::generate_workspace()` |

The WASM API surface in `src/lib.rs` (lines 1084-1307) serves as the template — the PyO3 bindings mirror the same function signatures, replacing `JsValue` returns with Python exceptions.

## Build Tooling

```toml
# pyproject.toml
[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"

[project]
name = "rustcam"
requires-python = ">=3.9"

[tool.maturin]
features = ["pyo3/extension-module"]
```

Build and install:
```bash
cd py-binding
maturin develop          # install in current venv
maturin build --release  # build wheel
```

## Shared IR Contract

The Python API produces and consumes the same `GraphSnapshot` JSON defined in [graph-snapshot-schema.md](graph-snapshot-schema.md). Graphs built in Python can be loaded in the block editor and vice versa.
