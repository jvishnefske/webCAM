//! Lower a `GraphSnapshot` to textual MLIR in the `dataflow` dialect.
//!
//! This module replaces the string-interpolation Rust codegen in `emit.rs`
//! with structured MLIR output. The generated `.mlir` is fed to `mlir-opt`
//! and `mlir-translate` by the pipeline module.

use std::collections::HashMap;
use std::fmt::Write;

use module_traits::value::PortKind;
use serde_json::Value as JsonValue;

use crate::dialect;
use crate::state_machine;

// ---------------------------------------------------------------------------
// Public types mirroring the main crate's graph types.
// These are deserialized from JSON so the mlir-codegen crate can stay
// decoupled from the main rustcam crate.
// ---------------------------------------------------------------------------

/// Mirrors `BlockSnapshot` from the main crate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BlockSnapshot {
    pub id: u32,
    pub block_type: String,
    pub name: String,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    #[serde(default)]
    pub config: JsonValue,
    #[serde(default)]
    pub output_values: Vec<Option<JsonValue>>,
    #[serde(default)]
    pub custom_codegen: Option<String>,
}

/// Mirrors `PortDef` from module-traits.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PortDef {
    pub name: String,
    pub kind: PortKind,
}

/// Mirrors `Channel` from the main crate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub from_block: BlockId,
    pub from_port: usize,
    pub to_block: BlockId,
    pub to_port: usize,
}

/// Mirrors `ChannelId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct ChannelId(pub u32);

/// Mirrors `BlockId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct BlockId(pub u32);

/// Mirrors `GraphSnapshot`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GraphSnapshot {
    pub blocks: Vec<BlockSnapshot>,
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub tick_count: u64,
    #[serde(default)]
    pub time: f64,
}

// ---------------------------------------------------------------------------
// Block classification (mirrors emit.rs)
// ---------------------------------------------------------------------------

const SKIPPED_BLOCK_TYPES: &[&str] = &["plot", "json_encode", "json_decode"];

fn is_skipped(bt: &str) -> bool {
    SKIPPED_BLOCK_TYPES.contains(&bt)
}

// ---------------------------------------------------------------------------
// Topological sort (inlined — same Kahn's algorithm as topo.rs)
// ---------------------------------------------------------------------------

fn topological_sort(
    block_ids: &[BlockId],
    channels: &[Channel],
) -> Result<Vec<BlockId>, String> {
    use std::collections::VecDeque;

    let mut in_degree: HashMap<BlockId, usize> = block_ids.iter().map(|&id| (id, 0)).collect();
    let mut adj: HashMap<BlockId, Vec<BlockId>> = block_ids.iter().map(|&id| (id, Vec::new())).collect();

    for ch in channels {
        if in_degree.contains_key(&ch.from_block) && in_degree.contains_key(&ch.to_block) {
            *in_degree.entry(ch.to_block).or_insert(0) += 1;
            adj.entry(ch.from_block).or_default().push(ch.to_block);
        }
    }

    let mut queue: VecDeque<BlockId> = {
        let mut sources: Vec<BlockId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        sources.sort_by_key(|id| id.0);
        sources.into_iter().collect()
    };

    let mut result = Vec::with_capacity(block_ids.len());
    while let Some(node) = queue.pop_front() {
        result.push(node);
        let mut neighbors: Vec<BlockId> = adj.get(&node).cloned().unwrap_or_default();
        neighbors.sort_by_key(|id| id.0);
        neighbors.dedup();
        for &neighbor in &neighbors {
            let edge_count = adj
                .get(&node)
                .map(|v| v.iter().filter(|&&n| n == neighbor).count())
                .unwrap_or(0);
            let deg = in_degree.get_mut(&neighbor).expect("block in adj but not in_degree");
            *deg = deg.saturating_sub(edge_count);
            if *deg == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if result.len() != block_ids.len() {
        Err("cycle detected in dataflow graph".to_string())
    } else {
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Input resolution (mirrors build_call_args in emit.rs)
// ---------------------------------------------------------------------------

/// For a given block, return the SSA name for each input port.
/// Connected inputs get the SSA name of the upstream output;
/// unconnected inputs get a zero-constant SSA name.
fn resolve_inputs(
    block_id: u32,
    block: &BlockSnapshot,
    channels: &[Channel],
    zero_ssa: &str,
) -> Vec<String> {
    let n = block.inputs.len();
    let mut args: Vec<Option<String>> = vec![None; n];
    for ch in channels {
        if ch.to_block.0 == block_id && ch.to_port < n {
            args[ch.to_port] = Some(dialect::ssa_name(ch.from_block.0, ch.from_port));
        }
    }
    args.into_iter()
        .map(|a| a.unwrap_or_else(|| zero_ssa.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

fn config_float(block: &BlockSnapshot, key: &str) -> f64 {
    block.config.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn config_u64(block: &BlockSnapshot, key: &str) -> u64 {
    block.config.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn config_str<'a>(block: &'a BlockSnapshot, key: &str) -> &'a str {
    block.config.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Main lowering entry point
// ---------------------------------------------------------------------------

/// Lower a `GraphSnapshot` (deserialized from JSON) to textual `.mlir`.
///
/// The output contains:
/// 1. A module-level comment describing the graph
/// 2. A `func.func @tick(%state: memref<?xf64>)` function
/// 3. Each block emitted as dataflow dialect ops in topological order
pub fn lower_graph(snap: &GraphSnapshot) -> Result<String, String> {
    let block_ids: Vec<BlockId> = snap.blocks.iter().map(|b| BlockId(b.id)).collect();
    let sorted = topological_sort(&block_ids, &snap.channels)?;
    let block_map: HashMap<u32, &BlockSnapshot> = snap.blocks.iter().map(|b| (b.id, b)).collect();

    // Collect state slots: each non-skipped block output needs a state memref index.
    let mut state_slots: Vec<(u32, usize)> = Vec::new(); // (block_id, port_idx)
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        if is_skipped(&block.block_type) {
            continue;
        }
        for (port_idx, _port) in block.outputs.iter().enumerate() {
            state_slots.push((id, port_idx));
        }
    }

    let mut out = String::with_capacity(4096);

    // Module header
    writeln!(out, "// Auto-generated MLIR from dataflow graph").unwrap();
    writeln!(out, "// Blocks: {}, Channels: {}", snap.blocks.len(), snap.channels.len()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "module {{").unwrap();
    writeln!(out).unwrap();

    // Emit state machine type declarations (before the tick function)
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        if block.block_type == "state_machine" {
            state_machine::emit_state_machine_type(&mut out, block)?;
        }
    }

    // Tick function
    writeln!(
        out,
        "func.func @tick({} : {}) {{",
        dialect::state_arg(),
        dialect::MLIR_MEMREF_F64,
    )
    .unwrap();

    // Emit a zero constant for unconnected inputs
    let zero_ssa = "%zero";
    writeln!(out, "    {zero_ssa} = arith.constant 0.0 : f64").unwrap();
    writeln!(out).unwrap();

    // Emit each block in topological order
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        let bt = block.block_type.as_str();

        if is_skipped(bt) {
            continue;
        }

        writeln!(out, "    // Block {id}: {} ({bt})", block.name).unwrap();

        let inputs = resolve_inputs(id, block, &snap.channels, zero_ssa);

        match bt {
            "constant" => emit_constant(&mut out, id, block)?,
            "gain" => emit_gain(&mut out, id, block, &inputs)?,
            "add" => emit_add(&mut out, id, &inputs)?,
            "multiply" => emit_multiply(&mut out, id, &inputs)?,
            "clamp" => emit_clamp(&mut out, id, block, &inputs)?,
            "adc_source" => emit_adc_read(&mut out, id, block)?,
            "pwm_sink" => emit_pwm_write(&mut out, id, block, &inputs)?,
            "gpio_out" => emit_gpio_write(&mut out, id, block, &inputs)?,
            "gpio_in" => emit_gpio_read(&mut out, id, block)?,
            "uart_tx" => emit_uart_tx(&mut out, id, block, &inputs)?,
            "uart_rx" => emit_uart_rx(&mut out, id, block)?,
            "encoder" => emit_encoder_read(&mut out, id, block)?,
            "ssd1306_display" => emit_display_write(&mut out, id, block, &inputs)?,
            "tmc2209_stepper" => emit_stepper(&mut out, id, block, &inputs)?,
            "tmc2209_stallguard" => emit_stallguard(&mut out, id, block)?,
            "state_machine" => state_machine::emit_state_machine_tick(&mut out, id, block, &inputs)?,
            "pubsub_sink" => emit_pubsub_sink(&mut out, id, block, &inputs)?,
            "pubsub_source" => emit_pubsub_source(&mut out, id, block)?,
            "udp_source" => emit_udp_source(&mut out, id)?,
            "udp_sink" => emit_udp_sink(&mut out, id, &inputs)?,
            other => {
                return Err(format!("unsupported block type for MLIR codegen: {other}"));
            }
        }
        writeln!(out).unwrap();
    }

    writeln!(out, "    return").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "}} // end module").unwrap();

    Ok(out)
}

// ---------------------------------------------------------------------------
// Per-block MLIR emission
// ---------------------------------------------------------------------------

fn emit_constant(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let value = config_float(block, "value");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op} {attr}",
        op = dialect::OP_CONSTANT,
        attr = dialect::float_attr(value),
    ).unwrap();
    Ok(())
}

fn emit_gain(out: &mut String, id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let factor = config_float(block, "param1");
    let input = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op}({input}) {{ factor = {attr} }} : f64",
        op = dialect::OP_GAIN,
        attr = dialect::float_attr(factor),
    ).unwrap();
    Ok(())
}

fn emit_add(out: &mut String, id: u32, inputs: &[String]) -> Result<(), String> {
    let a = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    let b = inputs.get(1).map(|s| s.as_str()).unwrap_or("%zero");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op}({a}, {b}) : f64",
        op = dialect::OP_ADD,
    ).unwrap();
    Ok(())
}

fn emit_multiply(out: &mut String, id: u32, inputs: &[String]) -> Result<(), String> {
    let a = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    let b = inputs.get(1).map(|s| s.as_str()).unwrap_or("%zero");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op}({a}, {b}) : f64",
        op = dialect::OP_MUL,
    ).unwrap();
    Ok(())
}

fn emit_clamp(out: &mut String, id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let min = config_float(block, "param1");
    let max = config_float(block, "param2");
    let input = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op}({input}) {{ min = {min_attr}, max = {max_attr} }} : f64",
        op = dialect::OP_CLAMP,
        min_attr = dialect::float_attr(min),
        max_attr = dialect::float_attr(max),
    ).unwrap();
    Ok(())
}

fn emit_adc_read(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let channel = config_u64(block, "channel");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op} {{ channel = {attr} }} : f64",
        op = dialect::OP_ADC_READ,
        attr = dialect::i32_attr(channel as i32),
    ).unwrap();
    Ok(())
}

fn emit_pwm_write(out: &mut String, _id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let channel = config_u64(block, "channel");
    let duty = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    writeln!(out, "    {op}({duty}) {{ channel = {attr} }}",
        op = dialect::OP_PWM_WRITE,
        attr = dialect::i32_attr(channel as i32),
    ).unwrap();
    Ok(())
}

fn emit_gpio_write(out: &mut String, _id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let pin = config_u64(block, "pin");
    let val = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    writeln!(out, "    {op}({val}) {{ pin = {attr} }}",
        op = dialect::OP_GPIO_WRITE,
        attr = dialect::i32_attr(pin as i32),
    ).unwrap();
    Ok(())
}

fn emit_gpio_read(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let pin = config_u64(block, "pin");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op} {{ pin = {attr} }} : f64",
        op = dialect::OP_GPIO_READ,
        attr = dialect::i32_attr(pin as i32),
    ).unwrap();
    Ok(())
}

fn emit_uart_tx(out: &mut String, _id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let port = config_u64(block, "port");
    let data = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    writeln!(out, "    {op}({data}) {{ port = {attr} }}",
        op = dialect::OP_UART_TX,
        attr = dialect::i32_attr(port as i32),
    ).unwrap();
    Ok(())
}

fn emit_uart_rx(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let port = config_u64(block, "port");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op} {{ port = {attr} }} : f64",
        op = dialect::OP_UART_RX,
        attr = dialect::i32_attr(port as i32),
    ).unwrap();
    Ok(())
}

fn emit_encoder_read(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let channel = config_u64(block, "channel");
    let ssa_pos = dialect::ssa_name(id, 0);
    let ssa_vel = dialect::ssa_name(id, 1);
    writeln!(out, "    {ssa_pos} = {op} {{ channel = {attr} }} : f64",
        op = dialect::OP_ENCODER_READ,
        attr = dialect::i32_attr(channel as i32),
    ).unwrap();
    // Velocity is a derived value — emit zero placeholder
    writeln!(out, "    {ssa_vel} = arith.constant 0.0 : f64  // velocity placeholder").unwrap();
    Ok(())
}

fn emit_display_write(out: &mut String, _id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let bus = config_u64(block, "i2c_bus");
    let addr = config_u64(block, "address");
    let line1 = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    let line2 = inputs.get(1).map(|s| s.as_str()).unwrap_or("%zero");
    writeln!(out, "    {op}({line1}, {line2}) {{ bus = {bus_attr}, addr = {addr_attr} }}",
        op = dialect::OP_DISPLAY_WRITE,
        bus_attr = dialect::i32_attr(bus as i32),
        addr_attr = dialect::i32_attr(addr as i32),
    ).unwrap();
    Ok(())
}

fn emit_stepper(out: &mut String, id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let port = config_u64(block, "uart_port");
    let target = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    let enable = inputs.get(1).map(|s| s.as_str()).unwrap_or("%zero");
    let port_attr = dialect::i32_attr(port as i32);

    writeln!(out, "    {op}({enable}) {{ port = {port_attr} }}",
        op = dialect::OP_STEPPER_ENABLE,
    ).unwrap();
    writeln!(out, "    {op}({target}) {{ port = {port_attr} }}",
        op = dialect::OP_STEPPER_MOVE,
    ).unwrap();
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op} {{ port = {port_attr} }} : f64",
        op = dialect::OP_STEPPER_POSITION,
    ).unwrap();
    Ok(())
}

fn emit_stallguard(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let port = config_u64(block, "uart_port");
    let addr = config_u64(block, "uart_addr");
    let threshold = config_u64(block, "threshold");
    let ssa_val = dialect::ssa_name(id, 0);
    let ssa_detect = dialect::ssa_name(id, 1);

    writeln!(out, "    {ssa_val} = {op} {{ port = {port_attr}, addr = {addr_attr} }} : f64",
        op = dialect::OP_STALLGUARD_READ,
        port_attr = dialect::i32_attr(port as i32),
        addr_attr = dialect::i32_attr(addr as i32),
    ).unwrap();
    // Stall detection: value < threshold → 1.0
    writeln!(out, "    // stall detection: threshold = {threshold}").unwrap();
    let threshold_ssa = format!("%sg_thresh_{id}");
    writeln!(out, "    {threshold_ssa} = arith.constant {}.0 : f64", threshold).unwrap();
    let cmp_ssa = format!("%sg_cmp_{id}");
    writeln!(out, "    {cmp_ssa} = arith.cmpf \"olt\", {ssa_val}, {threshold_ssa} : f64").unwrap();
    let one_ssa = format!("%sg_one_{id}");
    writeln!(out, "    {one_ssa} = arith.constant 1.0 : f64").unwrap();
    writeln!(out, "    {ssa_detect} = arith.select {cmp_ssa}, {one_ssa}, %zero : f64").unwrap();
    Ok(())
}

fn emit_pubsub_sink(out: &mut String, _id: u32, block: &BlockSnapshot, inputs: &[String]) -> Result<(), String> {
    let topic = config_str(block, "topic");
    let val = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    writeln!(out, "    {op}({val}) {{ topic = {attr} }}",
        op = dialect::OP_PUBLISH,
        attr = dialect::string_attr(if topic.is_empty() { "unknown" } else { topic }),
    ).unwrap();
    Ok(())
}

fn emit_pubsub_source(out: &mut String, id: u32, block: &BlockSnapshot) -> Result<(), String> {
    let topic = config_str(block, "topic");
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = {op} {{ topic = {attr} }} : f64",
        op = dialect::OP_SUBSCRIBE,
        attr = dialect::string_attr(if topic.is_empty() { "unknown" } else { topic }),
    ).unwrap();
    Ok(())
}

fn emit_udp_source(out: &mut String, id: u32) -> Result<(), String> {
    let ssa = dialect::ssa_name(id, 0);
    writeln!(out, "    {ssa} = arith.constant 0.0 : f64  // TODO: UDP source").unwrap();
    Ok(())
}

fn emit_udp_sink(out: &mut String, _id: u32, inputs: &[String]) -> Result<(), String> {
    let _val = inputs.first().map(|s| s.as_str()).unwrap_or("%zero");
    writeln!(out, "    // TODO: UDP sink").unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_block(id: u32, block_type: &str, config: JsonValue) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: block_type.to_string(),
            name: format!("test_{block_type}_{id}"),
            inputs: vec![],
            outputs: vec![PortDef {
                name: "out".to_string(),
                kind: PortKind::Float,
            }],
            config,
            output_values: vec![],
            custom_codegen: None,
        }
    }

    fn make_channel(id: u32, from: u32, from_port: usize, to: u32, to_port: usize) -> Channel {
        Channel {
            id: ChannelId(id),
            from_block: BlockId(from),
            from_port,
            to_block: BlockId(to),
            to_port,
        }
    }

    #[test]
    fn lower_constant_block() {
        let snap = GraphSnapshot {
            blocks: vec![make_block(1, "constant", serde_json::json!({"value": 42.0}))],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.constant"));
        assert!(mlir.contains("42"));
        assert!(mlir.contains("%v1_p0"));
    }

    #[test]
    fn lower_gain_chain() {
        let blocks = vec![
            make_block(1, "constant", serde_json::json!({"value": 10.0})),
            {
                let mut b = make_block(2, "gain", serde_json::json!({"param1": 3.0}));
                b.inputs = vec![PortDef {
                    name: "in".to_string(),
                    kind: PortKind::Float,
                }];
                b
            },
        ];
        let channels = vec![make_channel(1, 1, 0, 2, 0)];
        let snap = GraphSnapshot {
            blocks,
            channels,
            tick_count: 0,
            time: 0.0,
        };
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.gain(%v1_p0)"));
        assert!(mlir.contains("factor = 3"));
    }

    #[test]
    fn lower_add_block() {
        let blocks = vec![
            make_block(1, "constant", serde_json::json!({"value": 1.0})),
            make_block(2, "constant", serde_json::json!({"value": 2.0})),
            {
                let mut b = make_block(3, "add", serde_json::json!({}));
                b.inputs = vec![
                    PortDef { name: "a".to_string(), kind: PortKind::Float },
                    PortDef { name: "b".to_string(), kind: PortKind::Float },
                ];
                b
            },
        ];
        let channels = vec![
            make_channel(1, 1, 0, 3, 0),
            make_channel(2, 2, 0, 3, 1),
        ];
        let snap = GraphSnapshot { blocks, channels, tick_count: 0, time: 0.0 };
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.add(%v1_p0, %v2_p0)"));
    }

    #[test]
    fn lower_peripheral_adc_pwm() {
        let blocks = vec![
            {
                let mut b = make_block(1, "adc_source", serde_json::json!({"channel": 0}));
                b.inputs = vec![];
                b
            },
            {
                let mut b = make_block(2, "pwm_sink", serde_json::json!({"channel": 1}));
                b.inputs = vec![PortDef { name: "duty".to_string(), kind: PortKind::Float }];
                b.outputs = vec![];
                b
            },
        ];
        let channels = vec![make_channel(1, 1, 0, 2, 0)];
        let snap = GraphSnapshot { blocks, channels, tick_count: 0, time: 0.0 };
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.adc_read"));
        assert!(mlir.contains("dataflow.pwm_write(%v1_p0)"));
    }

    #[test]
    fn lower_cycle_detection() {
        let blocks = vec![
            {
                let mut b = make_block(1, "gain", serde_json::json!({"param1": 1.0}));
                b.inputs = vec![PortDef { name: "in".to_string(), kind: PortKind::Float }];
                b
            },
            {
                let mut b = make_block(2, "gain", serde_json::json!({"param1": 1.0}));
                b.inputs = vec![PortDef { name: "in".to_string(), kind: PortKind::Float }];
                b
            },
        ];
        let channels = vec![
            make_channel(1, 1, 0, 2, 0),
            make_channel(2, 2, 0, 1, 0),
        ];
        let snap = GraphSnapshot { blocks, channels, tick_count: 0, time: 0.0 };
        let result = lower_graph(&snap);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cycle"));
    }

    #[test]
    fn lower_skipped_blocks_excluded() {
        let blocks = vec![
            make_block(1, "constant", serde_json::json!({"value": 1.0})),
            make_block(2, "plot", serde_json::json!({})),
        ];
        let snap = GraphSnapshot { blocks, channels: vec![], tick_count: 0, time: 0.0 };
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.constant"));
        assert!(!mlir.contains("plot"));
    }

    #[test]
    fn lower_pubsub() {
        let blocks = vec![
            {
                let mut b = make_block(1, "pubsub_source", serde_json::json!({"topic": "bridge_1_0"}));
                b.inputs = vec![];
                b
            },
            {
                let mut b = make_block(2, "pubsub_sink", serde_json::json!({"topic": "bridge_1_0"}));
                b.inputs = vec![PortDef { name: "value".to_string(), kind: PortKind::Float }];
                b.outputs = vec![];
                b
            },
        ];
        let channels = vec![make_channel(1, 1, 0, 2, 0)];
        let snap = GraphSnapshot { blocks, channels, tick_count: 0, time: 0.0 };
        let mlir = lower_graph(&snap).unwrap();
        assert!(mlir.contains("dataflow.subscribe"));
        assert!(mlir.contains("dataflow.publish"));
        assert!(mlir.contains("bridge_1_0"));
    }
}
