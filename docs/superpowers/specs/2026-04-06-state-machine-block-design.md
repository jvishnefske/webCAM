# State Machine Block: Event-Driven FSM with Typed PubSub and MLIR Codegen

**Date**: 2026-04-06
**Status**: Draft
**Approach**: A — refactor existing `state_machine` block

## Overview

Refactor the existing state machine block into a deployable event-driven FSM with:
- Structured form editor for states, transitions, and topic bindings
- PubSub topic integration: transitions guarded by subscribed messages, actions publish messages
- `PortKind::Message` with typed field schemas
- MLIR codegen with region-based ops designed for future formal safety analysis (deadlock freedom, reachability, liveness)

## 1. Data Model: Message Types

### 1.1 FieldType and MessageSchema (`module-traits/src/value.rs`)

Add to the existing `no_std` value module:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    F32,
    F64,
    U8,
    U16,
    U32,
    I32,
    Bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageField {
    pub name: String,
    pub field_type: FieldType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageSchema {
    pub name: String,
    pub fields: Vec<MessageField>,
}
```

### 1.2 PortKind extension

```rust
pub enum PortKind {
    Float,
    Bytes,
    Text,
    Series,
    Any,
    Message(MessageSchema),  // NEW
}
```

### 1.3 Value extension

```rust
pub enum Value {
    Float(f64),
    Bytes(Vec<u8>),
    Text(String),
    Series(Vec<f64>),
    Message(MessageData),  // NEW
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageData {
    pub schema_name: String,
    pub fields: Vec<(String, f64)>,  // MVP: all fields stored as f64
}
```

MVP simplification: all message field values are stored as `f64` at runtime (bools as 0.0/1.0, integers cast to f64). The `FieldType` in the schema is used for codegen type checking and MLIR emission, not runtime dispatch. This avoids a recursive Value enum.

### 1.4 TypeScript mirror (`www/src/dataflow/types.ts`)

Add corresponding TS types:

```typescript
export interface MessageField {
  name: string;
  field_type: 'F32' | 'F64' | 'U8' | 'U16' | 'U32' | 'I32' | 'Bool';
}

export interface MessageSchema {
  name: string;
  fields: MessageField[];
}
```

## 2. State Machine Config

### 2.1 Rust config (`src/dataflow/blocks/state_machine.rs`)

Replace the current config with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineConfig {
    pub states: Vec<String>,
    pub initial: String,
    #[serde(default)]
    pub transitions: Vec<TransitionConfig>,
    #[serde(default)]
    pub input_topics: Vec<TopicBinding>,
    #[serde(default)]
    pub output_topics: Vec<TopicBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionConfig {
    pub from: String,
    pub to: String,
    pub guard: TransitionGuard,
    #[serde(default)]
    pub actions: Vec<TransitionAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TransitionGuard {
    /// Fire when a message arrives on a subscribed topic
    Topic {
        topic: String,
        /// Optional field condition: field_name op value
        condition: Option<FieldCondition>,
    },
    /// Fire unconditionally (epsilon transition)
    Unconditional,
    /// Legacy: fire when float guard port > 0.5
    GuardPort { port: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCondition {
    pub field: String,
    pub op: CompareOp,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompareOp { Eq, Ne, Gt, Lt, Ge, Le }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionAction {
    pub topic: String,
    pub message: Vec<(String, f64)>,  // field_name → value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicBinding {
    pub topic: String,
    pub schema: MessageSchema,
}
```

### 2.2 Backward compatibility

The `TransitionGuard::GuardPort` variant preserves backward compat with the old `guard_port: Option<usize>` field. Existing serialized configs can be migrated via a serde deserialize fallback or a one-time migration in `from_config()`.

### 2.3 Port generation

Ports are derived dynamically from the config:

- **Input ports**: One per `input_topics` entry, `PortKind::Message(schema)`
- **Output ports**:
  - `state` (Float) — current state index
  - `active_{name}` (Float) — one per state, 0.0 or 1.0
  - One per `output_topics` entry, `PortKind::Message(schema)`

## 3. Block Execution

### 3.1 Tick behavior

On each tick:

1. Check input ports for arriving messages (one port per subscribed topic)
2. For each transition from the current state (in config order):
   - If guard is `Topic { topic, condition }`: check if the matching input port has a message. If `condition` is set, evaluate `message.field op value`.
   - If guard is `Unconditional`: always fires.
   - If guard is `GuardPort { port }`: legacy float > 0.5 check.
3. First matching transition wins — update `current_state`.
4. Execute transition actions: build `MessageData` and set on output ports.
5. Emit outputs: state index, active flags, and any action messages.

### 3.2 Integration with PubSub

The state machine block does NOT directly call pubsub subscribe/publish. Instead:
- Input ports of type `Message` are wired to `PubSubSourceBlock` instances in the graph
- Output ports of type `Message` are wired to `PubSubSinkBlock` instances
- The dataflow graph handles the pubsub transport — the state machine is a pure computation block

This keeps the block composable and testable without pubsub infrastructure.

## 4. Form Editor UI

### 4.1 Location

The editor is a config panel that appears when a state machine block is selected. It replaces the default single-line config display with a multi-section form.

### 4.2 Sections

**States table:**
| State Name | Initial |
|-----------|---------|
| idle      | (*)     |
| running   |         |
| error     |         |
| [+ Add]   |         |

- Text input per row for state name
- Radio button for initial state
- Delete button per row (disabled if only one state remains)

**Input Topics table:**
| Topic Name  | Schema Fields          |
|-------------|------------------------|
| motor_cmd   | speed: F32, dir: Bool  |
| sensor_data | temp: F32, pressure: F32 |
| [+ Add]     |                        |

- Text input for topic name
- Inline field editor: `name: type` with add/remove buttons
- Type dropdown: F32, F64, U8, U16, U32, I32, Bool

**Output Topics table:**
Same structure as Input Topics.

**Transitions table:**
| From    | To      | Guard           | Condition        | Actions         |
|---------|---------|-----------------|------------------|-----------------|
| idle    | running | Topic: motor_cmd| speed > 0        | status: running=1|
| running | error   | Topic: sensor_data| temp > 100     |                 |
| error   | idle    | Unconditional   |                  | status: error=0 |
| [+ Add] |         |                 |                  |                 |

- Dropdowns for From/To (populated from States list)
- Guard type selector: Topic (dropdown from Input Topics) / Unconditional
- Condition: field dropdown (from selected topic schema) + op dropdown + value input
- Actions: output topic dropdown + field values

### 4.3 Implementation

New file: `www/src/dataflow/state-machine-editor.ts`

The editor is mounted into the node DOM when a state machine block is selected. It reads config from the block, presents the form, and writes updated config back via `mgr.updateBlockConfig(blockId, newConfig)`.

A WASM export `dataflow_update_block_config(graph_id, block_id, config_json)` is needed to update block config after creation. This re-creates the block with the new config, preserving connections.

## 5. MLIR Codegen

### 5.1 IR representation

Extend the existing `dataflow.state_machine` op to handle typed channels:

```mlir
// Subscribe to input topics (typed channel reads)
%motor_cmd = dataflow.channel_read "motor_cmd" : !dataflow.message<"motor_cmd", {speed: f32, dir: i1}>
%sensor_data = dataflow.channel_read "sensor_data" : !dataflow.message<"sensor_data", {temp: f32, pressure: f32}>

// State machine with typed inputs
%state, %active_idle, %active_running, %active_error, %status_out =
  dataflow.state_machine initial("idle") inputs(%motor_cmd, %sensor_data) {

  ^idle:  // state: idle
    %speed = dataflow.message_field %motor_cmd, "speed" : f32
    %threshold = arith.constant 0.0 : f32
    %go = arith.cmpf "ogt", %speed, %threshold : f32
    cf.cond_br %go, ^running, ^idle

  ^running:  // state: running
    %temp = dataflow.message_field %sensor_data, "temp" : f32
    %max_temp = arith.constant 100.0 : f32
    %overheat = arith.cmpf "ogt", %temp, %max_temp : f32
    cf.cond_br %overheat, ^error, ^running

  ^error:  // state: error
    cf.br ^idle  // unconditional reset

} -> (index, f64, f64, f64, !dataflow.message<"status">)

// Publish output messages
dataflow.channel_write "status", %status_out : !dataflow.message<"status">
```

### 5.2 New MLIR ops

| Op | Semantics |
|----|-----------|
| `dataflow.channel_read` | Read from a named typed channel (subscribe) |
| `dataflow.channel_write` | Write to a named typed channel (publish) |
| `dataflow.message_field` | Extract a typed field from a message value |
| `dataflow.state_machine` | Region-based FSM (existing, extended with typed inputs) |

### 5.3 Safety analysis hooks (future, not MVP)

The IR is designed so that future verification passes can:

- **Reachability**: Walk regions to confirm all states are reachable from `initial`
- **Deadlock freedom**: Verify every state has at least one outgoing transition (or is a designated terminal state)
- **Liveness**: Under fairness assumptions on input topics, verify progress (no infinite loops in a single state without external input)
- **Determinism**: Check that guards from the same state are mutually exclusive (or document priority order)

These are MLIR verification passes, not runtime checks. The information needed (state list, transition graph, guard predicates, topic schemas) is all present in the IR attributes.

### 5.4 Rust codegen (emit.rs)

Update the special-case state machine handling in `emit.rs` to:
- Generate typed message structs from schemas
- Generate pubsub subscribe/publish calls
- Generate match-based FSM tick with field extraction from messages
- Host target first; embedded targets follow the same pattern

### 5.5 DagRuntime (runtime.rs)

Extend `BlockFn::StateMachine` to support:
- `Message` inputs (not just float guards)
- Field extraction and comparison
- `Message` outputs for actions

## 6. WASM API Changes

### 6.1 New exports (`src/lib.rs`)

```rust
#[wasm_bindgen]
pub fn dataflow_update_block_config(
    graph_id: u32,
    block_id: u32,
    config_json: &str,
) -> Result<(), JsValue>
```

Replaces the block in the graph with a new instance using the updated config. Preserves existing channel connections where port names match.

### 6.2 Updated exports

`dataflow_block_types()` — already returns `BlockTypeInfo` with category. No change needed.

`dataflow_snapshot()` — already serializes block config. The new config fields will be included automatically via serde.

## 7. MVP Scope

**In scope:**
1. `PortKind::Message` + `Value::Message` + `MessageSchema` in `module-traits`
2. Refactored `StateMachineConfig` with topic bindings (backward compat via `GuardPort` variant)
3. Form editor UI: states, input/output topics with schemas, transitions
4. `dataflow_update_block_config` WASM export
5. Default palette config (done)
6. Updated MLIR emission with `channel_read`, `channel_write`, `message_field` ops
7. Updated `DagRuntime` execution with message support
8. Host-target Rust codegen

**Not in scope (future):**
- Formal safety analysis MLIR passes
- Per-MCU embedded codegen for event FSMs
- Visual state diagram preview
- Hierarchical/nested state machines
- Entry/exit actions on states
- History states

## 8. Files to Create/Modify

### New files:
- `www/src/dataflow/state-machine-editor.ts` — form editor component

### Modified files:
- `module-traits/src/value.rs` — add `FieldType`, `MessageField`, `MessageSchema`, `MessageData`, `PortKind::Message`, `Value::Message`
- `module-traits/src/lib.rs` — re-export new types
- `src/dataflow/block.rs` — re-export new types
- `src/dataflow/blocks/state_machine.rs` — new config model, topic-driven tick
- `src/dataflow/blocks/mod.rs` — update `create_block` for new config
- `src/lib.rs` — add `dataflow_update_block_config` export
- `src/dataflow/graph.rs` — add `update_block_config` method
- `www/src/dataflow/types.ts` — TS mirror types
- `www/src/dataflow/node-view.ts` — mount state machine editor on selection
- `www/src/dataflow/graph.ts` — add `updateBlockConfig` method
- `www/src/dataflow/palette.ts` — default config (done)
- `mlir-codegen/src/state_machine.rs` — typed channel ops, message field extraction
- `mlir-codegen/src/lower.rs` — wire up new state machine emitter
- `mlir-codegen/src/ir.rs` — add `ChannelRead`, `ChannelWrite`, `MessageField` op kinds
- `mlir-codegen/src/runtime.rs` — extend `BlockFn::StateMachine` for messages
- `src/dataflow/codegen/emit.rs` — update Rust codegen for topic-driven FSM

## 9. Build Sequence

1. **module-traits**: Message types, PortKind::Message, Value::Message
2. **state_machine.rs**: New config model + tick logic (Rust tests)
3. **graph.rs + lib.rs**: update_block_config WASM export
4. **mlir-codegen**: New ops, updated state machine emission
5. **runtime.rs**: Message support in DagRuntime
6. **emit.rs**: Updated Rust codegen
7. **www types.ts**: TS mirror types
8. **state-machine-editor.ts**: Form editor UI
9. **node-view.ts**: Mount editor on block selection
10. **Integration test**: End-to-end: create FSM, configure via editor, deploy, tick
