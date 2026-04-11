# Decoupled Model Architecture: Compiler-Driven Platform

**Date**: 2026-04-10
**Status**: Draft

## Overview

Refactor the RustCAM architecture from a monolithic `rustsim` crate into a layered compiler-driven platform with clean separation between:

- **Logical model** (what the dataflow does)
- **Deployment** (where blocks run)
- **Targets** (MCU capabilities)
- **Codegen** (how code is generated)

The key outcome: one logical graph can have multiple deployment configurations, blocks and targets plug in via traits, and all codegen is safe Rust.

## Problem

Today, `BlockSnapshot` in `rustsim/dataflow/graph.rs` mixes concerns:

```rust
pub struct BlockSnapshot {
    // Model (correct)
    id: u32, block_type: String, name: String,
    inputs: Vec<PortDef>, outputs: Vec<PortDef>,
    config: Value, is_delay: bool,
    
    // Deployment concern (wrong layer)
    target: Option<TargetFamily>,
    
    // Codegen artifact (wrong layer)
    custom_codegen: Option<String>,
    
    // Simulation state (wrong layer)
    output_values: Vec<Option<Value>>,
}
```

This causes:
- `graph.rs` imports `codegen::target::TargetFamily` — model depends on codegen
- `mlir-codegen` duplicates `BlockSnapshot` because it can't import from `rustsim`
- Adding a new target requires touching the model layer
- No way to have one graph with multiple deployment configs

## Architecture

### Layer Structure

```
L1    FOUNDATION        no_std, zero coupling
L1.5  RUNTIME           no_std, shared sim/firmware
L2    REGISTRIES         compile-time block/target catalogs
L3    DEPLOYMENT         logical → physical mapping
L3.5  VALIDATION         static analysis before codegen
L4    CODEGEN            forbid(unsafe_code)
L5    APPLICATION        simulation, WASM, firmware
```

### Crate Map

| Layer | Crate | Purpose | Dependencies |
|-------|-------|---------|-------------|
| L1 | `module-traits` | Block/Target trait interfaces, Value types | (none) |
| L1 | `graph-model` (NEW) | BlockSnapshot, GraphSnapshot, Channel, BlockId | module-traits |
| L1 | `dag-core` | Expression DAG IR, CBOR | (none) |
| L1.5 | `dataflow-rt` (EXISTS) | Embedded runtime: Block trait, Peripherals, RingBuffer | (none) |
| L2 | `block-registry` (NEW) | All block implementations, FunctionDef | module-traits |
| L2 | `target-registry` (NEW) | McuDef, TargetDef, per-MCU codegen | module-traits |
| L3 | `deployment` (NEW) | DeploymentManifest, PartitionMap, BoundaryAnalysis | graph-model, module-traits |
| L3.5 | (in deployment) | validate(graph, manifest) → Diagnostics | graph-model, deployment, target-registry |
| L4 | `mlir-codegen` | MLIR lowering, safe Rust emission | graph-model, module-traits, block-registry, deployment |
| L4 | `codegen-emit` (EXTRACT) | Rust workspace gen, per-target generators | graph-model, target-registry, deployment, block-registry |
| L5 | `rustsim` | DataflowGraph, WASM API, browser sim | graph-model, block-registry, deployment, mlir-codegen |
| L5 | `board-support-*` | Embassy firmware, impl Peripherals | dataflow-rt |

Dependencies flow strictly downward. No crate imports from a higher layer.

## L1: Foundation

### graph-model (NEW crate)

The single canonical definition of the dataflow model. Both `mlir-codegen` and `rustsim` import this — no more type duplication.

```rust
// graph-model/src/lib.rs
#![no_std]
extern crate alloc;

pub use module_traits::value::{Value, PortDef, PortKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub from_block: BlockId,
    pub from_port: usize,
    pub to_block: BlockId,
    pub to_port: usize,
}

/// Pure logical block description. No deployment, no simulation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSnapshot {
    pub id: BlockId,
    pub block_type: String,
    pub name: String,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    pub config: serde_json::Value,
    pub is_delay: bool,
}

/// Pure logical graph. Target-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub blocks: Vec<BlockSnapshot>,
    pub channels: Vec<Channel>,
}
```

`output_values`, `target`, and `custom_codegen` are gone. They belong to their respective layers.

### module-traits (EXTENDED)

Add registry trait interfaces. These define the plug-in API for blocks and targets.

```rust
// module-traits/src/registry.rs

/// Catalog of available block types. Implement to provide blocks.
pub trait BlockRegistry {
    /// List all block types this registry provides.
    fn block_types(&self) -> Vec<BlockTypeInfo>;
    
    /// Create a block instance from its type ID and config JSON.
    fn create(&self, type_id: &str, config: &str) -> Result<Box<dyn Module>, String>;
}

/// Catalog of available targets. Implement to provide MCU support.
pub trait TargetRegistry {
    /// List all targets this registry provides.
    fn targets(&self) -> Vec<TargetInfo>;
    
    /// Get the MCU definition for a target ID.
    fn mcu_def(&self, id: &str) -> Option<&McuDef>;
    
    /// Get the code generator for a target ID.
    fn codegen(&self, id: &str) -> Option<&dyn TargetCodegen>;
}

pub struct BlockTypeInfo {
    pub id: String,
    pub display_name: String,
    pub category: String,
}

pub struct TargetInfo {
    pub id: String,
    pub display_name: String,
    pub rust_target: String,
}

/// Per-target code generation. One implementation per MCU family.
pub trait TargetCodegen {
    fn generate(
        &self,
        snap: &GraphSnapshot,
        binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String>;
}
```

These traits use `&str` for target IDs (not an enum), so adding a target never touches the trait definition.

## L2: Registries

### block-registry (NEW crate)

Extracts all block implementations from `rustsim/dataflow/blocks/` into a standalone crate.

```rust
// block-registry/src/lib.rs
pub struct BuiltinBlocks;

impl BlockRegistry for BuiltinBlocks {
    fn block_types(&self) -> Vec<BlockTypeInfo> {
        let mut types = Vec::new();
        // Data-driven blocks from FunctionDef
        for def in builtin_function_defs() {
            types.push(BlockTypeInfo {
                id: def.id.clone(),
                display_name: def.display_name.clone(),
                category: def.category.clone(),
            });
        }
        // Legacy blocks
        types.extend(legacy_block_types());
        types
    }
    
    fn create(&self, type_id: &str, config: &str) -> Result<Box<dyn Module>, String> {
        // Try FunctionDef first, then legacy
        if let Some(def) = lookup_function_def(type_id) {
            return Ok(Box::new(DataDrivenBlock::from_def(def, config)?));
        }
        create_legacy_block(type_id, config)
    }
}
```

Contains: `data_driven.rs`, `state_machine.rs`, `register.rs`, `embedded.rs`, `pubsub.rs`, `udp.rs`, `serde_block.rs`.

### target-registry (NEW crate)

Extracts target definitions and per-MCU code generators from `rustsim/dataflow/codegen/`.

```rust
// target-registry/src/lib.rs
pub struct BuiltinTargets;

impl TargetRegistry for BuiltinTargets {
    fn targets(&self) -> Vec<TargetInfo> {
        vec![
            TargetInfo { id: "host".into(), display_name: "Host Simulation".into(), rust_target: "".into() },
            TargetInfo { id: "rp2040".into(), display_name: "RP2040 (Pico)".into(), rust_target: "thumbv6m-none-eabi".into() },
            TargetInfo { id: "stm32f4".into(), display_name: "STM32F401CC".into(), rust_target: "thumbv7em-none-eabihf".into() },
            TargetInfo { id: "esp32c3".into(), display_name: "ESP32-C3".into(), rust_target: "riscv32imc-unknown-none-elf".into() },
            TargetInfo { id: "stm32g0b1".into(), display_name: "STM32G0B1CB".into(), rust_target: "thumbv6m-none-eabi".into() },
        ]
    }
    
    fn mcu_def(&self, id: &str) -> Option<&McuDef> { ... }
    fn codegen(&self, id: &str) -> Option<&dyn TargetCodegen> { ... }
}
```

Contains: `McuDef` definitions (from `module-traits/inventory.rs`), per-target generators (from `rustsim/codegen/targets/`), `TargetCodegen` impls.

**Adding a new target**: Create a new crate that implements `TargetRegistry` with its own `McuDef` and `TargetCodegen`. No changes to any existing crate.

**Adding a new block**: Create a new crate that implements `BlockRegistry` with its own `Module`/`Tick`/`Codegen` impls. No changes to any existing crate.

## L3: Deployment

### Three-Tier Manifest

```rust
// deployment/src/lib.rs
use graph_model::{BlockId, ChannelId};

/// Unique identifier for a hardware node in the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Complete deployment specification.
pub struct DeploymentManifest {
    /// Mapping tier: which block runs on which node
    pub assignments: HashMap<BlockId, NodeId>,
    
    /// Physical tier: hardware nodes
    pub nodes: Vec<TargetNode>,
    
    /// Physical tier: wiring between nodes
    pub topology: Vec<PhysicalLink>,
    
    /// Pin/peripheral bindings per node
    pub bindings: HashMap<NodeId, HardwareConfig>,
}

pub struct TargetNode {
    pub id: NodeId,
    pub target_id: String,     // references TargetRegistry
    pub role: Option<String>,  // "main_controller", "sensor_hub", etc.
}

pub struct PhysicalLink {
    pub id: String,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub protocol: Protocol,
}

pub enum Protocol {
    Spi { clock_hz: u32 },
    Uart { baud: u32 },
    I2c { freq_hz: u32 },
    Can { bitrate: u32 },
    SharedMemory,
    UdpMulticast { group: String, port: u16 },
}
```

### Boundary Analysis

```rust
pub enum ChannelPlacement {
    /// Both endpoints on the same node — local buffer
    IntraNode { node: NodeId },
    /// Endpoints on different nodes — needs transport
    InterNode { 
        source_node: NodeId, 
        sink_node: NodeId,
        link: PhysicalLink,
    },
}

pub fn analyze_boundaries(
    graph: &GraphSnapshot,
    manifest: &DeploymentManifest,
) -> HashMap<ChannelId, ChannelPlacement> { ... }
```

## L3.5: Validation

```rust
pub enum Severity { Error, Warning, Info }

pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub block_ids: Vec<BlockId>,
}

pub fn validate(
    graph: &GraphSnapshot,
    manifest: &DeploymentManifest,
    targets: &dyn TargetRegistry,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_all_blocks_assigned(graph, manifest, &mut diags);
    check_memory_budget(graph, manifest, targets, &mut diags);
    check_peripheral_conflicts(graph, manifest, targets, &mut diags);
    check_topology_coverage(graph, manifest, &mut diags);
    diags
}
```

Validation checks:
- All blocks assigned to a node
- Node memory budget not exceeded
- No two blocks on the same node claim the same peripheral
- Inter-node channels have a PhysicalLink
- No orphan nodes (nodes with no blocks)

## L4: Codegen

### mlir-codegen (UPDATED)

Now imports `graph-model` directly — no more type duplication.

```rust
// mlir-codegen/src/lower.rs
use graph_model::{BlockSnapshot, GraphSnapshot, Channel, BlockId, ChannelId};
use deployment::{DeploymentManifest, ChannelPlacement, analyze_boundaries};

pub fn lower_graph(
    snap: &GraphSnapshot,
    manifest: Option<&DeploymentManifest>,
    blocks: &dyn BlockRegistry,
) -> Result<String, String> {
    let boundaries = manifest.map(|m| analyze_boundaries(snap, m));
    // ... lowering with boundary-aware channel emission
}
```

### codegen-emit (EXTRACTED from rustsim)

The Rust workspace generation code moves out of `rustsim/dataflow/codegen/` into its own crate.

```rust
pub fn generate_workspace(
    snap: &GraphSnapshot,
    manifest: &DeploymentManifest,
    targets: &dyn TargetRegistry,
    blocks: &dyn BlockRegistry,
    dt: f64,
) -> Result<GeneratedWorkspace, String> { ... }
```

## Compiler Pipeline

```
User UI / DSL
    │
    ▼
GraphSnapshot (Logical IR)          ← graph-model
    │
    ├── + user target assignments
    ▼
DeploymentManifest (Physical IR)    ← deployment
    │
    ├── validate(graph, manifest, targets)
    ▼                                 ← validation
Per-node subgraphs + boundaries     ← analyze_boundaries
    │
    ├── lower_graph(snap, manifest, blocks)
    ▼                                 ← mlir-codegen
MLIR + safe Rust source
    │
    ├── generate_workspace(snap, manifest, targets, blocks, dt)
    ▼                                 ← codegen-emit
Cargo workspace with per-target crates
    │
    ├── cargo build --target thumbv6m-none-eabi
    ▼
ELF binaries → flash to MCUs
```

## Migration Path

This is a large refactoring. Incremental migration in this order:

### Phase A: Extract graph-model
1. Create `graph-model` crate with `BlockSnapshot` (clean, no target/codegen/output_values)
2. `rustsim` uses `graph-model::BlockSnapshot` + extends it with a wrapper for simulation state
3. `mlir-codegen` imports `graph-model` directly, delete duplicate types
4. All existing tests continue to work

### Phase B: Extract block-registry
1. Move block implementations from `rustsim/dataflow/blocks/` to `block-registry`
2. `rustsim` depends on `block-registry` instead of owning block code
3. `BlockRegistry` trait in `module-traits`

### Phase C: Extract target-registry
1. Move `TargetFamily` enum, `McuDef`, per-target generators to `target-registry`
2. Replace enum-based dispatch with trait-based dispatch
3. `TargetRegistry` trait in `module-traits`

### Phase D: Extract deployment
1. Create `deployment` crate with `DeploymentManifest`
2. Move `partition.rs` logic to use `DeploymentManifest` instead of `BlockSnapshot.target`
3. Remove `target` field from `BlockSnapshot`
4. Add validation pass

### Phase E: Extract codegen-emit
1. Move `emit.rs`, `targets/`, `binding.rs` to `codegen-emit` crate
2. `rustsim` becomes a thin simulation + WASM API layer

Each phase results in a compilable, testable state. Phase A is the most important — it establishes the contract everything else signs.

## Safe Rust Enforcement

| Crate | Safety |
|-------|--------|
| graph-model | `#![forbid(unsafe_code)]` |
| module-traits | `#![no_std]`, safe |
| block-registry | `#![forbid(unsafe_code)]` |
| target-registry | `#![forbid(unsafe_code)]` |
| deployment | `#![forbid(unsafe_code)]` |
| mlir-codegen | `#![forbid(unsafe_code)]` (already) |
| codegen-emit | `#![forbid(unsafe_code)]` |
| dataflow-rt | `#![no_std]`, safe |
| rustsim | safe (WASM boundary uses wasm-bindgen) |

## Verification

After full migration:
- `cargo test --workspace` — all existing tests pass
- `cargo clippy --workspace --all-targets -- -D warnings` — no warnings
- `cargo build --target thumbv6m-none-eabi -p board-support-pico2` — firmware still builds
- `wasm-pack build --target web` — WASM still builds
- `cd www && npm test` — frontend tests pass
- Create a graph in browser, assign targets, generate code, verify output matches pre-refactor
