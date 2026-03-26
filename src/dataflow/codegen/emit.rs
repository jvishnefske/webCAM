//! Code emitter: generates a standalone Rust workspace from a dataflow graph snapshot.

use std::collections::HashMap;
use std::fmt::Write;

use crate::dataflow::block::BlockId;
use crate::dataflow::codegen::binding::TargetWithBinding;
use crate::dataflow::codegen::concurrency::find_parallel_groups;
use crate::dataflow::codegen::partition;
use crate::dataflow::codegen::target::TargetFamily;
use crate::dataflow::codegen::targets::generator_for;
use crate::dataflow::codegen::topo::topological_sort;
use crate::dataflow::graph::{BlockSnapshot, GraphSnapshot};

/// A generated Rust crate represented as a collection of files (legacy).
#[derive(Debug, Clone)]
pub struct GeneratedCrate {
    /// (relative path, file content) pairs.
    pub files: Vec<(String, String)>,
}

/// A generated Rust workspace with logic lib + target binaries.
#[derive(Debug, Clone)]
pub struct GeneratedWorkspace {
    /// (relative path, file content) pairs.
    pub files: Vec<(String, String)>,
}

/// Block types that are skipped during code generation (visualization-only or
/// require external dependencies not suitable for embedded targets).
const SKIPPED_BLOCK_TYPES: &[&str] = &["plot", "json_encode", "json_decode"];

/// Block types that map to peripheral trait calls (no longer stubs).
const PERIPHERAL_BLOCK_TYPES: &[&str] = &[
    "adc_source",
    "pwm_sink",
    "gpio_out",
    "gpio_in",
    "uart_tx",
    "uart_rx",
    "encoder",
    "ssd1306_display",
    "tmc2209_stepper",
    "tmc2209_stallguard",
];

/// Wrap a call argument with `state.` prefix only if it's a variable reference
/// (starts with `out_`). Literal defaults like `0.0_f64` are used as-is.
fn state_ref(arg: &str) -> String {
    if arg.starts_with("out_") {
        format!("state.{arg}")
    } else {
        arg.to_string()
    }
}

/// Block types that produce a stub with a TODO comment (legacy).
const STUB_BLOCK_TYPES: &[&str] = &[
    "udp_source",
    "udp_sink",
    "adc_source",
    "pwm_sink",
    "gpio_out",
    "gpio_in",
    "uart_tx",
    "uart_rx",
    "encoder",
    "ssd1306_display",
    "tmc2209_stepper",
    "tmc2209_stallguard",
];

/// Generate a standalone Rust project from a dataflow graph snapshot (legacy API).
///
/// `dt` is the fixed timestep in seconds for the generated main loop.
/// Returns a `GeneratedCrate` containing all files needed for the project.
pub fn generate_rust(snap: &GraphSnapshot, dt: f64) -> Result<GeneratedCrate, String> {
    let cargo_toml = generate_legacy_cargo_toml();
    let blocks_rs = generate_blocks_rs(snap)?;

    // Analyze for parallelism: use threaded code only when there are 2+ groups.
    let block_ids: Vec<BlockId> = snap.blocks.iter().map(|b| BlockId(b.id)).collect();
    let groups = find_parallel_groups(&block_ids, &snap.channels)?;
    let main_rs = if groups.len() >= 2 {
        generate_parallel_main_rs(snap, dt)?
    } else {
        generate_main_rs(snap, dt)?
    };

    Ok(GeneratedCrate {
        files: vec![
            ("Cargo.toml".to_string(), cargo_toml),
            ("src/blocks.rs".to_string(), blocks_rs),
            ("src/main.rs".to_string(), main_rs),
        ],
    })
}

/// Generate a multi-target workspace from a dataflow graph snapshot.
///
/// Produces a workspace with:
/// - `logic/` — no_std library with `State` struct and `tick()` function
/// - `target-<name>/` — per-target binary crates
/// - `dataflow-rt/` — vendored runtime copy
pub fn generate_workspace(
    snap: &GraphSnapshot,
    dt: f64,
    targets: &[TargetWithBinding],
) -> Result<GeneratedWorkspace, String> {
    let mut files: Vec<(String, String)> = Vec::new();

    // Generate workspace root Cargo.toml
    let workspace_members = build_workspace_members(targets);
    files.push((
        "Cargo.toml".to_string(),
        generate_workspace_cargo_toml(&workspace_members),
    ));

    // Generate logic crate
    let logic_cargo = generate_logic_cargo_toml();
    let logic_blocks = generate_logic_blocks_rs(snap)?;
    let logic_lib = generate_logic_lib_rs(snap)?;
    files.push(("logic/Cargo.toml".to_string(), logic_cargo));
    files.push(("logic/src/blocks.rs".to_string(), logic_blocks));
    files.push(("logic/src/lib.rs".to_string(), logic_lib));

    // Vendor dataflow-rt
    files.push((
        "dataflow-rt/Cargo.toml".to_string(),
        generate_rt_cargo_toml(),
    ));
    files.push((
        "dataflow-rt/src/lib.rs".to_string(),
        generate_rt_lib_rs(),
    ));

    // Generate target crates
    for twb in targets {
        let gen = generator_for(twb.target);
        let target_files = gen.generate(snap, &twb.binding, dt)?;
        files.extend(target_files);
    }

    Ok(GeneratedWorkspace { files })
}

// ---------------------------------------------------------------------------
// Workspace generation helpers
// ---------------------------------------------------------------------------

fn build_workspace_members(targets: &[TargetWithBinding]) -> Vec<String> {
    let mut members = vec!["logic".to_string(), "dataflow-rt".to_string()];
    for twb in targets {
        let name = match twb.target {
            TargetFamily::Host => "target-host",
            TargetFamily::Rp2040 => "target-rp2040",
            TargetFamily::Stm32f4 => "target-stm32f4",
            TargetFamily::Esp32c3 => "target-esp32c3",
            TargetFamily::Stm32g0b1 => "target-stm32g0b1",
        };
        if !members.contains(&name.to_string()) {
            members.push(name.to_string());
        }
    }
    members
}

fn generate_workspace_cargo_toml(members: &[String]) -> String {
    let mut out = String::new();
    writeln!(out, "[workspace]").unwrap();
    writeln!(out, "resolver = \"2\"").unwrap();
    write!(out, "members = [").unwrap();
    for (i, m) in members.iter().enumerate() {
        if i > 0 {
            write!(out, ", ").unwrap();
        }
        write!(out, "\"{m}\"").unwrap();
    }
    writeln!(out, "]").unwrap();
    out
}

fn generate_logic_cargo_toml() -> String {
    r#"[package]
name = "logic"
version = "0.1.0"
edition = "2021"

[lib]
name = "logic"

[dependencies]
dataflow-rt = { path = "../dataflow-rt", default-features = false }
"#
    .to_string()
}

fn generate_rt_cargo_toml() -> String {
    r#"[package]
name = "dataflow-rt"
version = "0.1.0"
edition = "2021"

[features]
default = ["std"]
std = []
"#
    .to_string()
}

fn generate_rt_lib_rs() -> String {
    r#"//! Minimal runtime for generated dataflow code.
//!
//! `no_std` by default — enable the `std` feature for hosted targets.

#![cfg_attr(not(feature = "std"), no_std)]

/// Hardware peripheral abstraction for generated dataflow code.
pub trait Peripherals {
    fn adc_read(&mut self, channel: u8) -> f32;
    fn pwm_write(&mut self, channel: u8, duty: f32);
    fn gpio_read(&self, pin: u8) -> bool;
    fn gpio_write(&mut self, pin: u8, high: bool);
    fn uart_write(&mut self, port: u8, data: &[u8]);
    fn uart_read(&mut self, port: u8, buf: &mut [u8]) -> usize;
    fn encoder_read(&mut self, channel: u8) -> i64 { 0 }
    fn display_write(&mut self, bus: u8, addr: u8, line1: &str, line2: &str) {}
    fn stepper_move(&mut self, port: u8, target: i64) {}
    fn stepper_position(&self, port: u8) -> i64 { 0 }
    fn stepper_enable(&mut self, port: u8, enabled: bool) {}
    fn stallguard_read(&mut self, port: u8, addr: u8) -> u16 { 0 }
}
"#
    .to_string()
}

/// Generate the logic crate's lib.rs with State struct and tick() function.
fn generate_logic_lib_rs(snap: &GraphSnapshot) -> Result<String, String> {
    let block_ids: Vec<BlockId> = snap.blocks.iter().map(|b| BlockId(b.id)).collect();
    let sorted = topological_sort(&block_ids, &snap.channels)?;
    let block_map: std::collections::HashMap<u32, &BlockSnapshot> =
        snap.blocks.iter().map(|b| (b.id, b)).collect();

    let mut out = String::new();
    writeln!(out, "#![no_std]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "mod blocks;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use dataflow_rt::Peripherals;").unwrap();
    writeln!(out).unwrap();

    // State struct
    writeln!(out, "pub struct State {{").unwrap();
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        if is_skipped(&block.block_type) || is_peripheral(&block.block_type) {
            continue;
        }
        if block.block_type == "state_machine" {
            writeln!(out, "    pub sm_{id}: blocks::Block{id},").unwrap();
        }
        for (port_idx, port) in block.outputs.iter().enumerate() {
            let ty = crate::dataflow::codegen::types::rust_type_no_std(&port.kind);
            writeln!(out, "    pub out_{id}_p{port_idx}: {ty},").unwrap();
        }
    }
    // Also need state vars for peripheral source blocks (adc, gpio_in, uart_rx)
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        if is_peripheral_source(&block.block_type) {
            for (port_idx, port) in block.outputs.iter().enumerate() {
                let ty = crate::dataflow::codegen::types::rust_type_no_std(&port.kind);
                writeln!(out, "    pub out_{id}_p{port_idx}: {ty},").unwrap();
            }
        }
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Default impl
    writeln!(out, "impl Default for State {{").unwrap();
    writeln!(out, "    fn default() -> Self {{").unwrap();
    writeln!(out, "        Self {{").unwrap();
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        if is_skipped(&block.block_type) || is_peripheral(&block.block_type) {
            if is_peripheral_source(&block.block_type) {
                for (port_idx, port) in block.outputs.iter().enumerate() {
                    let default =
                        crate::dataflow::codegen::types::rust_default_no_std(&port.kind);
                    writeln!(out, "            out_{id}_p{port_idx}: {default},").unwrap();
                }
            }
            continue;
        }
        if block.block_type == "state_machine" {
            writeln!(out, "            sm_{id}: blocks::Block{id}::default(),").unwrap();
        }
        for (port_idx, port) in block.outputs.iter().enumerate() {
            let default = crate::dataflow::codegen::types::rust_default_no_std(&port.kind);
            writeln!(out, "            out_{id}_p{port_idx}: {default},").unwrap();
        }
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // tick() function
    writeln!(
        out,
        "pub fn tick(hw: &mut impl Peripherals, state: &mut State) {{"
    )
    .unwrap();

    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        let bt = block.block_type.as_str();

        if is_skipped(bt) {
            continue;
        }

        writeln!(
            out,
            "    // Block {id}: {} ({bt})",
            block.name
        )
        .unwrap();

        // Build the argument list from connected channels
        let args = build_call_args(id, block, &snap.channels);

        match bt {
            // Peripheral blocks → trait calls
            "adc_source" => {
                let channel = block
                    .config
                    .get("channel")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                writeln!(
                    out,
                    "    state.out_{id}_p0 = hw.adc_read({channel}) as f64;"
                )
                .unwrap();
            }
            "pwm_sink" => {
                let channel = block
                    .config
                    .get("channel")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let arg = if args.is_empty() {
                    "0.0".to_string()
                } else {
                    state_ref(&args[0])
                };
                writeln!(
                    out,
                    "    hw.pwm_write({channel}, {arg} as f32);"
                )
                .unwrap();
            }
            "gpio_out" => {
                let pin = block
                    .config
                    .get("pin")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(13);
                let arg = if args.is_empty() {
                    "0.0".to_string()
                } else {
                    state_ref(&args[0])
                };
                writeln!(
                    out,
                    "    hw.gpio_write({pin}, {arg} > 0.5);"
                )
                .unwrap();
            }
            "gpio_in" => {
                let pin = block
                    .config
                    .get("pin")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2);
                writeln!(
                    out,
                    "    state.out_{id}_p0 = if hw.gpio_read({pin}) {{ 1.0 }} else {{ 0.0 }};"
                )
                .unwrap();
            }
            "uart_tx" => {
                let port = block
                    .config
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let arg = if args.is_empty() {
                    "&[]".to_string()
                } else {
                    let r = state_ref(&args[0]);
                    format!("&{r}")
                };
                writeln!(out, "    hw.uart_write({port}, {arg});").unwrap();
            }
            "uart_rx" => {
                let port = block
                    .config
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                writeln!(
                    out,
                    "    {{ let mut buf = [0u8; 64]; let _n = hw.uart_read({port}, &mut buf); }}"
                )
                .unwrap();
            }
            "encoder" => {
                let channel = block
                    .config
                    .get("channel")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                writeln!(
                    out,
                    "    state.out_{id}_p0 = hw.encoder_read({channel}) as f64;"
                )
                .unwrap();
                writeln!(
                    out,
                    "    state.out_{id}_p1 = 0.0; // velocity: computed by differencing"
                )
                .unwrap();
            }
            "ssd1306_display" => {
                let bus = block
                    .config
                    .get("i2c_bus")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let addr = block
                    .config
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0x3C);
                let arg0 = if args.is_empty() {
                    "\"\"".to_string()
                } else {
                    let r = state_ref(&args[0]);
                    format!("&{r}")
                };
                let arg1 = if args.len() < 2 {
                    "\"\"".to_string()
                } else {
                    let r = state_ref(&args[1]);
                    format!("&{r}")
                };
                writeln!(
                    out,
                    "    hw.display_write({bus}, {addr}, {arg0}, {arg1});"
                )
                .unwrap();
            }
            "tmc2209_stepper" => {
                let port = block
                    .config
                    .get("uart_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let arg_target = if args.is_empty() {
                    "0.0".to_string()
                } else {
                    state_ref(&args[0])
                };
                let arg_enable = if args.len() < 2 {
                    "0.0".to_string()
                } else {
                    state_ref(&args[1])
                };
                writeln!(
                    out,
                    "    hw.stepper_enable({port}, {arg_enable} > 0.5);"
                )
                .unwrap();
                writeln!(
                    out,
                    "    hw.stepper_move({port}, {arg_target} as i64);"
                )
                .unwrap();
                writeln!(
                    out,
                    "    state.out_{id}_p0 = hw.stepper_position({port}) as f64;"
                )
                .unwrap();
            }
            "tmc2209_stallguard" => {
                let port = block
                    .config
                    .get("uart_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let addr = block
                    .config
                    .get("uart_addr")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let threshold = block
                    .config
                    .get("threshold")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50);
                writeln!(
                    out,
                    "    state.out_{id}_p0 = hw.stallguard_read({port}, {addr}) as f64;"
                )
                .unwrap();
                writeln!(
                    out,
                    "    state.out_{id}_p1 = if (state.out_{id}_p0 as u16) < {threshold} {{ 1.0 }} else {{ 0.0 }};"
                )
                .unwrap();
            }
            // State machine blocks
            "state_machine" => {
                let arg_str = args
                    .iter()
                    .map(|a| state_ref(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                if block.outputs.len() <= 1 {
                    writeln!(
                        out,
                        "    state.out_{id}_p0 = state.sm_{id}.tick({arg_str});"
                    )
                    .unwrap();
                } else {
                    let vars: Vec<String> = (0..block.outputs.len())
                        .map(|p| format!("state.out_{id}_p{p}"))
                        .collect();
                    let var_str = vars.join(", ");
                    writeln!(
                        out,
                        "    ({var_str}) = state.sm_{id}.tick({arg_str});"
                    )
                    .unwrap();
                }
            }
            // Pure computation blocks → function calls
            _ => {
                let arg_str = args
                    .iter()
                    .map(|a| state_ref(a))
                    .collect::<Vec<_>>()
                    .join(", ");

                if block.outputs.is_empty() {
                    writeln!(out, "    blocks::block_{id}({arg_str});").unwrap();
                } else if block.outputs.len() == 1 {
                    writeln!(
                        out,
                        "    state.out_{id}_p0 = blocks::block_{id}({arg_str});"
                    )
                    .unwrap();
                } else {
                    let vars: Vec<String> = (0..block.outputs.len())
                        .map(|p| format!("state.out_{id}_p{p}"))
                        .collect();
                    let var_str = vars.join(", ");
                    writeln!(
                        out,
                        "    ({var_str}) = blocks::block_{id}({arg_str});"
                    )
                    .unwrap();
                }
            }
        }
    }

    writeln!(out, "}}").unwrap();
    Ok(out)
}

/// Generate the logic crate's blocks.rs with pure computation functions.
fn generate_logic_blocks_rs(snap: &GraphSnapshot) -> Result<String, String> {
    let mut out = String::new();
    writeln!(out, "//! Generated block functions (pure computation).").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![allow(unused)]").unwrap();
    writeln!(out).unwrap();

    for block in &snap.blocks {
        let bt = block.block_type.as_str();
        let id = block.id;

        if is_skipped(bt) || is_peripheral(bt) {
            continue;
        }

        writeln!(out, "/// Block {id}: {} ({bt})", block.name).unwrap();

        match bt {
            "constant" => {
                let value = config_float(block, "value");
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    {value}_f64").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "gain" => {
                let factor = config_float(block, "param1");
                writeln!(out, "pub fn block_{id}(input: f64) -> f64 {{").unwrap();
                writeln!(out, "    input * {factor}_f64").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "add" => {
                writeln!(out, "pub fn block_{id}(a: f64, b: f64) -> f64 {{").unwrap();
                writeln!(out, "    a + b").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "multiply" => {
                writeln!(out, "pub fn block_{id}(a: f64, b: f64) -> f64 {{").unwrap();
                writeln!(out, "    a * b").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "clamp" => {
                let min = config_float(block, "param1");
                let max = config_float(block, "param2");
                writeln!(out, "pub fn block_{id}(input: f64) -> f64 {{").unwrap();
                writeln!(out, "    input.clamp({min}_f64, {max}_f64)").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "state_machine" => {
                emit_state_machine_block(&mut out, block)?;
            }
            "udp_source" => {
                let addr = block
                    .config
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0.0:0");
                writeln!(out, "// TODO: implement UDP receive from {addr}").unwrap();
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    0.0").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "udp_sink" => {
                let addr = block
                    .config
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0.0:0");
                writeln!(out, "// TODO: implement UDP send to {addr}").unwrap();
                writeln!(out, "pub fn block_{id}(_data: f64) {{").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "pubsub_sink" => {
                let topic = block.config.get("topic").and_then(|v| v.as_str()).unwrap_or("unknown");
                writeln!(out, "// pubsub_sink: topic=\"{topic}\"").unwrap();
                writeln!(out, "pub fn block_{id}(_value: f64) {{").unwrap();
                writeln!(out, "    // TODO: pubsub::encode + transport.send").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "pubsub_source" => {
                let topic = block.config.get("topic").and_then(|v| v.as_str()).unwrap_or("unknown");
                writeln!(out, "// pubsub_source: topic=\"{topic}\"").unwrap();
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    // TODO: transport.recv + pubsub::decode").unwrap();
                writeln!(out, "    0.0").unwrap();
                writeln!(out, "}}").unwrap();
            }
            other => {
                return Err(format!("unsupported block type for codegen: {other}"));
            }
        }
        writeln!(out).unwrap();
    }

    Ok(out)
}

/// Emit a state machine block as a struct with enum and tick method.
fn emit_state_machine_block(out: &mut String, block: &BlockSnapshot) -> Result<(), String> {
    let id = block.id;
    let config = &block.config;

    let states: Vec<String> = config
        .get("states")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if states.is_empty() {
        return Err(format!("state_machine block {id} has no states"));
    }

    let initial = config
        .get("initial")
        .and_then(|v| v.as_str())
        .unwrap_or(&states[0]);

    let transitions: Vec<serde_json::Value> = config
        .get("transitions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Generate state enum
    writeln!(out, "#[derive(Clone, Copy, Default)]").unwrap();
    writeln!(out, "pub enum Block{id}State {{").unwrap();
    for (i, state) in states.iter().enumerate() {
        let variant = to_pascal_case(state);
        if state == initial {
            writeln!(out, "    #[default]").unwrap();
        }
        // If this is the first variant and initial wasn't found, mark it default
        if i == 0 && !states.contains(&initial.to_string()) {
            writeln!(out, "    #[default]").unwrap();
        }
        writeln!(out, "    {variant},").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Generate block struct
    writeln!(out, "#[derive(Clone)]").unwrap();
    writeln!(out, "pub struct Block{id} {{").unwrap();
    writeln!(out, "    pub state: Block{id}State,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "impl Default for Block{id} {{").unwrap();
    writeln!(out, "    fn default() -> Self {{").unwrap();
    writeln!(out, "        Self {{ state: Block{id}State::default() }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Generate tick method
    // Inputs: one f64 per guard port
    let n_guards = block.inputs.len();
    let guard_params: Vec<String> = (0..n_guards).map(|i| format!("guard_{i}: f64")).collect();
    let guard_param_str = guard_params.join(", ");

    // Outputs: state index + one per state (active_<name>)
    let n_outputs = 1 + states.len(); // state index + active flags
    let output_type = if n_outputs == 1 {
        "f64".to_string()
    } else {
        format!("({})", vec!["f64"; n_outputs].join(", "))
    };

    writeln!(out, "impl Block{id} {{").unwrap();
    writeln!(
        out,
        "    pub fn tick(&mut self, {guard_param_str}) -> {output_type} {{"
    )
    .unwrap();
    writeln!(out, "        self.state = match self.state {{").unwrap();

    for state in &states {
        let variant = to_pascal_case(state);
        // Find transitions from this state
        let from_transitions: Vec<&serde_json::Value> = transitions
            .iter()
            .filter(|t| t.get("from").and_then(|v| v.as_str()) == Some(state))
            .collect();

        write!(out, "            Block{id}State::{variant} => ").unwrap();

        if from_transitions.is_empty() {
            // Stay in current state
            writeln!(out, "Block{id}State::{variant},").unwrap();
        } else {
            let mut first = true;
            for t in &from_transitions {
                let to_state = t
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or(state);
                let to_variant = to_pascal_case(to_state);
                let guard_port = t.get("guard_port").and_then(|v| v.as_u64());

                if let Some(port) = guard_port {
                    if first {
                        writeln!(
                            out,
                            "if guard_{port} > 0.5 {{ Block{id}State::{to_variant} }}"
                        )
                        .unwrap();
                        first = false;
                    } else {
                        writeln!(
                            out,
                            "            else if guard_{port} > 0.5 {{ Block{id}State::{to_variant} }}"
                        )
                        .unwrap();
                    }
                } else {
                    // Unconditional transition
                    if first {
                        writeln!(out, "Block{id}State::{to_variant},").unwrap();
                    } else {
                        writeln!(
                            out,
                            "            else {{ Block{id}State::{to_variant} }},"
                        )
                        .unwrap();
                    }
                    first = false;
                }
            }
            // If all transitions were conditional, add else clause to stay
            if from_transitions
                .iter()
                .all(|t| t.get("guard_port").and_then(|v| v.as_u64()).is_some())
            {
                writeln!(
                    out,
                    "            else {{ Block{id}State::{variant} }},"
                )
                .unwrap();
            }
        }
    }

    writeln!(out, "        }};").unwrap();
    writeln!(out, "        let idx = self.state as u8 as f64;").unwrap();

    // Output active flags
    if n_outputs == 1 {
        writeln!(out, "        idx").unwrap();
    } else {
        let mut active_exprs = vec!["idx".to_string()];
        for state in &states {
            let variant = to_pascal_case(state);
            active_exprs.push(format!(
                "if matches!(self.state, Block{id}State::{variant}) {{ 1.0 }} else {{ 0.0 }}"
            ));
        }
        let expr = active_exprs.join(", ");
        writeln!(out, "        ({expr})").unwrap();
    }

    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

fn is_skipped(block_type: &str) -> bool {
    SKIPPED_BLOCK_TYPES.contains(&block_type)
}

fn is_stub(block_type: &str) -> bool {
    STUB_BLOCK_TYPES.contains(&block_type)
}

fn is_peripheral(block_type: &str) -> bool {
    PERIPHERAL_BLOCK_TYPES.contains(&block_type)
}

fn is_peripheral_source(block_type: &str) -> bool {
    matches!(block_type, "adc_source" | "gpio_in" | "uart_rx" | "encoder" | "tmc2209_stallguard" | "tmc2209_stepper")
}

// ---------------------------------------------------------------------------
// Legacy single-crate generation (preserved for backward compatibility)
// ---------------------------------------------------------------------------

fn generate_legacy_cargo_toml() -> String {
    r#"[package]
name = "dataflow-generated"
version = "0.1.0"
edition = "2021"

[dependencies]
dataflow-rt = { path = "../dataflow-rt" }

[profile.release]
opt-level = "z"
lto = true
"#
    .to_string()
}

/// Extract a named float parameter from a block's config JSON object.
fn config_float(block: &BlockSnapshot, key: &str) -> f64 {
    block
        .config
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

fn generate_blocks_rs(snap: &GraphSnapshot) -> Result<String, String> {
    let mut out = String::new();
    writeln!(out, "//! Generated block functions.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![allow(unused)]").unwrap();
    writeln!(out).unwrap();

    for block in &snap.blocks {
        let bt = block.block_type.as_str();
        let id = block.id;

        if is_skipped(bt) {
            continue;
        }

        // Comment header
        writeln!(out, "/// Block {id}: {} ({bt})", block.name).unwrap();

        // Check for custom codegen from the Codegen trait
        if let Some(ref custom_code) = block.custom_codegen {
            writeln!(out, "{custom_code}").unwrap();
            writeln!(out).unwrap();
            continue;
        }

        match bt {
            "constant" => {
                let value = config_float(block, "value");
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    {value}_f64").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "gain" => {
                let factor = config_float(block, "param1");
                writeln!(out, "pub fn block_{id}(input: f64) -> f64 {{").unwrap();
                writeln!(out, "    input * {factor}_f64").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "add" => {
                writeln!(out, "pub fn block_{id}(a: f64, b: f64) -> f64 {{").unwrap();
                writeln!(out, "    a + b").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "multiply" => {
                writeln!(out, "pub fn block_{id}(a: f64, b: f64) -> f64 {{").unwrap();
                writeln!(out, "    a * b").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "clamp" => {
                let min = config_float(block, "param1");
                let max = config_float(block, "param2");
                writeln!(out, "pub fn block_{id}(input: f64) -> f64 {{").unwrap();
                writeln!(out, "    input.clamp({min}_f64, {max}_f64)").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "udp_source" => {
                let addr = block
                    .config
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0.0:0");
                writeln!(out, "// TODO: implement UDP receive from {addr}").unwrap();
                writeln!(out, "pub fn block_{id}() -> Vec<u8> {{").unwrap();
                writeln!(out, "    Vec::new()").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "udp_sink" => {
                let addr = block
                    .config
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0.0:0");
                writeln!(out, "// TODO: implement UDP send to {addr}").unwrap();
                writeln!(out, "pub fn block_{id}(_data: &[u8]) {{").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "adc_source" => {
                let channel = block.config.get("channel").and_then(|v| v.as_u64()).unwrap_or(0);
                let bits = block.config.get("resolution_bits").and_then(|v| v.as_u64()).unwrap_or(12);
                writeln!(out, "// TODO: Read ADC channel {channel} ({bits}-bit resolution)").unwrap();
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    0.0 // stub: ADC read").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "pwm_sink" => {
                let channel = block.config.get("channel").and_then(|v| v.as_u64()).unwrap_or(0);
                let freq = block.config.get("frequency_hz").and_then(|v| v.as_u64()).unwrap_or(1000);
                writeln!(out, "// TODO: Set PWM channel {channel} at {freq}Hz").unwrap();
                writeln!(out, "pub fn block_{id}(_duty: f64) {{").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "gpio_out" => {
                let pin = block.config.get("pin").and_then(|v| v.as_u64()).unwrap_or(13);
                writeln!(out, "// TODO: Set GPIO pin {pin} output").unwrap();
                writeln!(out, "pub fn block_{id}(_state: f64) {{").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "gpio_in" => {
                let pin = block.config.get("pin").and_then(|v| v.as_u64()).unwrap_or(2);
                writeln!(out, "// TODO: Read GPIO pin {pin} input").unwrap();
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    0.0 // stub: GPIO read").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "uart_tx" => {
                let port = block.config.get("port").and_then(|v| v.as_u64()).unwrap_or(0);
                let baud = block.config.get("baud").and_then(|v| v.as_u64()).unwrap_or(115200);
                writeln!(out, "// TODO: Transmit on UART{port} at {baud} baud").unwrap();
                writeln!(out, "pub fn block_{id}(_data: &[u8]) {{").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "uart_rx" => {
                let port = block.config.get("port").and_then(|v| v.as_u64()).unwrap_or(0);
                let baud = block.config.get("baud").and_then(|v| v.as_u64()).unwrap_or(115200);
                writeln!(out, "// TODO: Receive from UART{port} at {baud} baud").unwrap();
                writeln!(out, "pub fn block_{id}() -> Vec<u8> {{").unwrap();
                writeln!(out, "    Vec::new()").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "encoder" => {
                let channel = block.config.get("channel").and_then(|v| v.as_u64()).unwrap_or(0);
                writeln!(out, "// TODO: Read quadrature encoder channel {channel}").unwrap();
                writeln!(out, "pub fn block_{id}() -> (f64, f64) {{").unwrap();
                writeln!(out, "    (0.0, 0.0) // stub: (position, velocity)").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "ssd1306_display" => {
                let bus = block.config.get("i2c_bus").and_then(|v| v.as_u64()).unwrap_or(0);
                let addr = block.config.get("address").and_then(|v| v.as_u64()).unwrap_or(0x3C);
                writeln!(out, "// TODO: Write to SSD1306 display on I2C bus {bus}, addr 0x{addr:02X}").unwrap();
                writeln!(out, "pub fn block_{id}(_line1: &str, _line2: &str) {{").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "tmc2209_stepper" => {
                let port = block.config.get("uart_port").and_then(|v| v.as_u64()).unwrap_or(0);
                writeln!(out, "// TODO: TMC2209 stepper on UART port {port}").unwrap();
                writeln!(out, "pub fn block_{id}(_target: f64, _enable: f64) -> f64 {{").unwrap();
                writeln!(out, "    0.0 // stub: actual position").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "tmc2209_stallguard" => {
                let port = block.config.get("uart_port").and_then(|v| v.as_u64()).unwrap_or(0);
                writeln!(out, "// TODO: TMC2209 StallGuard on UART port {port}").unwrap();
                writeln!(out, "pub fn block_{id}() -> (f64, f64) {{").unwrap();
                writeln!(out, "    (0.0, 0.0) // stub: (sg_value, stall_detected)").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "pubsub_sink" => {
                let topic = block.config.get("topic").and_then(|v| v.as_str()).unwrap_or("unknown");
                writeln!(out, "// pubsub_sink: topic=\"{topic}\"").unwrap();
                writeln!(out, "pub fn block_{id}(_value: f64) {{").unwrap();
                writeln!(out, "    // TODO: pubsub::encode + transport.send").unwrap();
                writeln!(out, "}}").unwrap();
            }
            "pubsub_source" => {
                let topic = block.config.get("topic").and_then(|v| v.as_str()).unwrap_or("unknown");
                writeln!(out, "// pubsub_source: topic=\"{topic}\"").unwrap();
                writeln!(out, "pub fn block_{id}() -> f64 {{").unwrap();
                writeln!(out, "    // TODO: transport.recv + pubsub::decode").unwrap();
                writeln!(out, "    0.0").unwrap();
                writeln!(out, "}}").unwrap();
            }
            other => {
                return Err(format!("unsupported block type for codegen: {other}"));
            }
        }
        writeln!(out).unwrap();
    }

    Ok(out)
}

fn generate_main_rs(snap: &GraphSnapshot, dt: f64) -> Result<String, String> {
    // Collect block IDs and run topological sort.
    let block_ids: Vec<BlockId> = snap.blocks.iter().map(|b| BlockId(b.id)).collect();
    let sorted = topological_sort(&block_ids, &snap.channels)?;

    // Build a lookup from block ID to snapshot.
    let block_map: std::collections::HashMap<u32, &BlockSnapshot> =
        snap.blocks.iter().map(|b| (b.id, b)).collect();

    let mut out = String::new();
    writeln!(out, "//! Generated dataflow main loop.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "mod blocks;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "fn main() {{").unwrap();
    writeln!(out, "    let dt: f64 = {dt}_f64;").unwrap();
    writeln!(out).unwrap();

    // Declare state variables for each non-skipped block's output ports.
    writeln!(out, "    // State variables for block outputs.").unwrap();
    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        if is_skipped(&block.block_type) {
            continue;
        }
        for (port_idx, port) in block.outputs.iter().enumerate() {
            let default = crate::dataflow::codegen::types::rust_default(&port.kind);
            let ty = crate::dataflow::codegen::types::rust_type(&port.kind);
            writeln!(
                out,
                "    let mut out_{id}_p{port_idx}: {ty} = {default};"
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();

    // Main loop.
    writeln!(out, "    loop {{").unwrap();

    for &BlockId(id) in &sorted {
        let block = block_map[&id];
        let bt = block.block_type.as_str();

        if is_skipped(bt) {
            writeln!(
                out,
                "        // Block {id}: {} ({bt}) -- skipped",
                block.name
            )
            .unwrap();
            continue;
        }

        writeln!(
            out,
            "        // Block {id}: {} ({bt})",
            block.name
        )
        .unwrap();

        // Build the argument list from connected channels.
        let args = build_call_args(id, block, &snap.channels);

        if is_stub(bt) && matches!(bt, "udp_sink" | "uart_tx") {
            // These stubs take a byte-slice reference, no output.
            writeln!(
                out,
                "        blocks::block_{id}({});",
                if args.is_empty() {
                    "&[]".to_string()
                } else {
                    format!("&{}", args[0])
                }
            )
            .unwrap();
        } else if block.outputs.is_empty() {
            // No outputs (e.g. pwm_sink, gpio_out)
            let arg_str = args.join(", ");
            writeln!(out, "        blocks::block_{id}({arg_str});").unwrap();
        } else if block.outputs.len() == 1 {
            let arg_str = args.join(", ");
            writeln!(
                out,
                "        out_{id}_p0 = blocks::block_{id}({arg_str});"
            )
            .unwrap();
        } else {
            // Multiple outputs: use tuple destructuring.
            let arg_str = args.join(", ");
            let vars: Vec<String> = (0..block.outputs.len())
                .map(|p| format!("out_{id}_p{p}"))
                .collect();
            let var_str = vars.join(", ");
            writeln!(
                out,
                "        ({var_str}) = blocks::block_{id}({arg_str});"
            )
            .unwrap();
        }
    }

    writeln!(out).unwrap();
    writeln!(
        out,
        "        // Fixed timestep delay."
    )
    .unwrap();
    writeln!(
        out,
        "        std::thread::sleep(std::time::Duration::from_secs_f64(dt));"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(out)
}

fn generate_parallel_main_rs(snap: &GraphSnapshot, dt: f64) -> Result<String, String> {
    let block_ids: Vec<BlockId> = snap.blocks.iter().map(|b| BlockId(b.id)).collect();
    let groups = find_parallel_groups(&block_ids, &snap.channels)?;

    let block_map: std::collections::HashMap<u32, &BlockSnapshot> =
        snap.blocks.iter().map(|b| (b.id, b)).collect();

    let mut out = String::new();
    writeln!(out, "//! Generated dataflow main loop (parallel).").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "mod blocks;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "fn main() {{").unwrap();
    writeln!(out, "    let dt: f64 = {dt}_f64;").unwrap();
    writeln!(out).unwrap();

    // Declare state variables for all non-skipped blocks.
    writeln!(out, "    // State variables for block outputs.").unwrap();
    let all_sorted = topological_sort(&block_ids, &snap.channels)?;
    for &BlockId(id) in &all_sorted {
        let block = block_map[&id];
        if is_skipped(&block.block_type) {
            continue;
        }
        for (port_idx, port) in block.outputs.iter().enumerate() {
            let default = crate::dataflow::codegen::types::rust_default(&port.kind);
            let ty = crate::dataflow::codegen::types::rust_type(&port.kind);
            writeln!(
                out,
                "    let mut out_{id}_p{port_idx}: {ty} = {default};"
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();

    // Main loop with thread::scope.
    writeln!(out, "    loop {{").unwrap();
    writeln!(out, "        std::thread::scope(|s| {{").unwrap();

    for (group_idx, group) in groups.iter().enumerate() {
        let block_id_strs: Vec<String> = group.blocks.iter().map(|b| b.0.to_string()).collect();
        writeln!(
            out,
            "            // Group {group_idx} (blocks {})",
            block_id_strs.join(", ")
        )
        .unwrap();
        writeln!(out, "            s.spawn(|| {{").unwrap();

        for &BlockId(id) in &group.blocks {
            let block = block_map[&id];
            let bt = block.block_type.as_str();

            if is_skipped(bt) {
                writeln!(
                    out,
                    "                // Block {id}: {} ({bt}) -- skipped",
                    block.name
                )
                .unwrap();
                continue;
            }

            writeln!(
                out,
                "                // Block {id}: {} ({bt})",
                block.name
            )
            .unwrap();

            let args = build_call_args(id, block, &snap.channels);
            emit_block_call(&mut out, id, block, bt, &args, "                ");
        }

        writeln!(out, "            }});").unwrap();
    }

    writeln!(out, "        }});").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "        // Fixed timestep delay."
    )
    .unwrap();
    writeln!(
        out,
        "        std::thread::sleep(std::time::Duration::from_secs_f64(dt));"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(out)
}

/// Emit a single block call statement at the given indentation level.
fn emit_block_call(
    out: &mut String,
    id: u32,
    block: &BlockSnapshot,
    bt: &str,
    args: &[String],
    indent: &str,
) {
    if is_stub(bt) && matches!(bt, "udp_sink" | "uart_tx") {
        writeln!(
            out,
            "{indent}blocks::block_{id}({});",
            if args.is_empty() {
                "&[]".to_string()
            } else {
                format!("&{}", args[0])
            }
        )
        .unwrap();
    } else if block.outputs.is_empty() {
        let arg_str = args.join(", ");
        writeln!(out, "{indent}blocks::block_{id}({arg_str});").unwrap();
    } else if block.outputs.len() == 1 {
        let arg_str = args.join(", ");
        writeln!(
            out,
            "{indent}out_{id}_p0 = blocks::block_{id}({arg_str});"
        )
        .unwrap();
    } else {
        let arg_str = args.join(", ");
        let vars: Vec<String> = (0..block.outputs.len())
            .map(|p| format!("out_{id}_p{p}"))
            .collect();
        let var_str = vars.join(", ");
        writeln!(
            out,
            "{indent}({var_str}) = blocks::block_{id}({arg_str});"
        )
        .unwrap();
    }
}

/// Build the list of argument expressions for a block call, based on channel
/// connections. Unconnected input ports use the variable's current (default)
/// value with a 0.0 literal.
fn build_call_args(
    block_id: u32,
    block: &BlockSnapshot,
    channels: &[crate::dataflow::channel::Channel],
) -> Vec<String> {
    let n_inputs = block.inputs.len();
    let mut args: Vec<Option<String>> = vec![None; n_inputs];

    for ch in channels {
        if ch.to_block.0 == block_id && ch.to_port < n_inputs {
            args[ch.to_port] = Some(format!("out_{}_p{}", ch.from_block.0, ch.from_port));
        }
    }

    args.into_iter()
        .enumerate()
        .map(|(i, arg)| {
            arg.unwrap_or_else(|| {
                crate::dataflow::codegen::types::rust_default(&block.inputs[i].kind).to_string()
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Distributed multi-MCU workspace generation
// ---------------------------------------------------------------------------

/// Configuration for distributed multi-MCU code generation.
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    /// Per-target bindings (one per MCU in the system).
    pub targets: Vec<TargetWithBinding>,
    /// Fixed timestep in seconds.
    pub dt: f64,
    /// Transport configuration for pubsub bridges.
    pub transport: TransportConfig,
}

/// Transport layer configuration for inter-MCU communication.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// CAN bus -- all nodes share a bus.
    Can,
    /// UDP/IP -- each node has an IP address.
    Ip {
        addresses: HashMap<TargetFamily, String>,
    },
}

/// Result of distributed code generation -- one workspace per target.
#[derive(Debug, Clone)]
pub struct DistributedWorkspace {
    /// Per-target generated workspaces.
    pub workspaces: HashMap<TargetFamily, GeneratedWorkspace>,
}

/// Generate separate firmware workspaces for a distributed multi-MCU system.
///
/// Each target gets its own workspace containing only the blocks assigned to it,
/// plus pubsub bridge blocks for cross-target communication.
///
/// # Errors
///
/// Returns an error if:
/// - Any block has no target assignment (`target` is `None`)
/// - Partition or code generation fails for any target
pub fn generate_distributed_workspace(
    snap: &GraphSnapshot,
    config: &DistributedConfig,
) -> Result<DistributedWorkspace, String> {
    // 1. Partition the graph by target assignment.
    let partition_result = partition::partition_graph(snap)
        .map_err(|e| format!("partition error: {e:?}"))?;

    let has_bridges = !partition_result.bridges.is_empty();

    // 2. Generate one workspace per partition.
    let mut workspaces: HashMap<TargetFamily, GeneratedWorkspace> = HashMap::new();
    for (target_family, sub_snap) in &partition_result.partitions {
        // Find the binding for this target.
        let twb = config
            .targets
            .iter()
            .find(|t| t.target == *target_family)
            .ok_or_else(|| format!("no binding for target {target_family:?}"))?;

        let mut ws = generate_workspace(sub_snap, config.dt, std::slice::from_ref(twb))?;

        // 3. If there are bridges, add pubsub dependency to logic Cargo.toml.
        if has_bridges {
            for file in &mut ws.files {
                if file.0 == "logic/Cargo.toml" {
                    file.1.push_str("\n[dependencies.pubsub]\npath = \"../pubsub\"\n");
                }
            }
        }

        // 4/5. Inject pubsub bridge code into logic/src/lib.rs for this partition's bridges.
        for bridge in &partition_result.bridges {
            for file in &mut ws.files {
                if file.0 == "logic/src/lib.rs" {
                    if bridge.source_target == *target_family {
                        // This partition is the sender -- emit pubsub_sink encode+send.
                        file.1.push_str(&format!(
                            "\n    // pubsub_sink: topic=\"{}\"\n    // pubsub::encode(&value); transport.send();\n",
                            bridge.topic
                        ));
                    }
                    if bridge.sink_target == *target_family {
                        // This partition is the receiver -- emit pubsub_source recv+decode.
                        file.1.push_str(&format!(
                            "\n    // pubsub_source: topic=\"{}\"\n    // let value = pubsub::decode(&transport.recv());\n",
                            bridge.topic
                        ));
                    }
                }
            }
        }

        workspaces.insert(*target_family, ws);
    }

    Ok(DistributedWorkspace { workspaces })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::block::{PortDef, PortKind, Value};
    use crate::dataflow::channel::{Channel, ChannelId};
    use crate::dataflow::codegen::binding::{Binding, TargetWithBinding};
    use crate::dataflow::codegen::target::TargetFamily;
    use crate::dataflow::graph::{BlockSnapshot, GraphSnapshot};

    fn make_constant_snapshot(id: u32, value: f64) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "constant".to_string(),
            name: format!("Constant_{id}"),
            inputs: vec![],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({ "value": value }),
            output_values: vec![Some(Value::Float(value))],
            target: None,
        custom_codegen: None,
        }
    }

    fn make_gain_snapshot(id: u32, factor: f64) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "gain".to_string(),
            name: format!("Gain_{id}"),
            inputs: vec![PortDef::new("in", PortKind::Float)],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({ "op": "Gain", "param1": factor, "param2": 0.0 }),
            output_values: vec![Some(Value::Float(0.0))],
            target: None,
        custom_codegen: None,
        }
    }

    fn make_add_snapshot(id: u32) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "add".to_string(),
            name: format!("Add_{id}"),
            inputs: vec![
                PortDef::new("a", PortKind::Float),
                PortDef::new("b", PortKind::Float),
            ],
            outputs: vec![PortDef::new("out", PortKind::Float)],
            config: serde_json::json!({ "op": "Add", "param1": 0.0, "param2": 0.0 }),
            output_values: vec![Some(Value::Float(0.0))],
            target: None,
        custom_codegen: None,
        }
    }

    fn make_plot_snapshot(id: u32) -> BlockSnapshot {
        BlockSnapshot {
            id,
            block_type: "plot".to_string(),
            name: format!("Plot_{id}"),
            inputs: vec![PortDef::new("in", PortKind::Float)],
            outputs: vec![PortDef::new("series", PortKind::Series)],
            config: serde_json::json!({ "max_samples": 1000 }),
            output_values: vec![],
            target: None,
        custom_codegen: None,
        }
    }

    fn ch(id: u32, from: u32, from_port: usize, to: u32, to_port: usize) -> Channel {
        Channel {
            id: ChannelId(id),
            from_block: BlockId(from),
            from_port,
            to_block: BlockId(to),
            to_port,
        }
    }

    // Legacy tests -----------------------------------------------------------

    #[test]
    fn constant_only_graph() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 42.0)],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        assert_eq!(result.files.len(), 3);

        let blocks_rs = &result.files[1].1;
        assert!(blocks_rs.contains("pub fn block_1() -> f64"));
        assert!(blocks_rs.contains("42"));

        let main_rs = &result.files[2].1;
        assert!(main_rs.contains("mod blocks;"));
        assert!(main_rs.contains("out_1_p0 = blocks::block_1()"));
        assert!(main_rs.contains("std::thread::sleep"));
    }

    #[test]
    fn constant_to_gain_chain() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 5.0), make_gain_snapshot(2, 3.0)],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let main_rs = &result.files[2].1;

        // Constant should appear before gain.
        let const_pos = main_rs.find("blocks::block_1()").unwrap();
        let gain_pos = main_rs.find("blocks::block_2(").unwrap();
        assert!(const_pos < gain_pos);

        // Gain should receive the constant's output variable.
        assert!(main_rs.contains("blocks::block_2(out_1_p0)"));
    }

    #[test]
    fn two_constants_to_add() {
        let snap = GraphSnapshot {
            blocks: vec![
                make_constant_snapshot(1, 2.0),
                make_constant_snapshot(2, 3.0),
                make_add_snapshot(3),
            ],
            channels: vec![ch(1, 1, 0, 3, 0), ch(2, 2, 0, 3, 1)],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let main_rs = &result.files[2].1;

        // Add block should receive both constant outputs.
        assert!(main_rs.contains("blocks::block_3(out_1_p0, out_2_p0)"));
    }

    #[test]
    fn plot_blocks_are_skipped() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 1.0), make_plot_snapshot(2)],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();

        let blocks_rs = &result.files[1].1;
        assert!(!blocks_rs.contains("block_2"));

        let main_rs = &result.files[2].1;
        assert!(main_rs.contains("skipped"));
        // No state variable for the plot block.
        assert!(!main_rs.contains("out_2_p0"));
    }

    #[test]
    fn generated_cargo_toml_is_valid() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 1.0)],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let cargo_toml = &result.files[0].1;

        assert!(cargo_toml.contains("[package]"));
        assert!(cargo_toml.contains("name = \"dataflow-generated\""));
        assert!(cargo_toml.contains("edition = \"2021\""));
        assert!(cargo_toml.contains("[profile.release]"));
        assert!(cargo_toml.contains("opt-level = \"z\""));
        assert!(cargo_toml.contains("lto = true"));
    }

    #[test]
    fn dt_appears_in_main() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 1.0)],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.02).unwrap();
        let main_rs = &result.files[2].1;
        assert!(main_rs.contains("0.02"));
    }

    #[test]
    fn clamp_block_emits_params() {
        let snap = GraphSnapshot {
            blocks: vec![BlockSnapshot {
                id: 1,
                block_type: "clamp".to_string(),
                name: "Clamp_1".to_string(),
                inputs: vec![PortDef::new("in", PortKind::Float)],
                outputs: vec![PortDef::new("out", PortKind::Float)],
                config: serde_json::json!({ "op": "Clamp", "param1": -1.0, "param2": 1.0 }),
                output_values: vec![],
                target: None,
            custom_codegen: None,
            }],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let blocks_rs = &result.files[1].1;
        assert!(blocks_rs.contains("clamp(-1"));
        assert!(blocks_rs.contains("1_f64"));
    }

    #[test]
    fn udp_blocks_emit_stubs() {
        let snap = GraphSnapshot {
            blocks: vec![BlockSnapshot {
                id: 1,
                block_type: "udp_source".to_string(),
                name: "UDP Source".to_string(),
                inputs: vec![],
                outputs: vec![PortDef::new("data", PortKind::Bytes)],
                config: serde_json::json!({ "address": "127.0.0.1:9000" }),
                output_values: vec![],
                target: None,
            custom_codegen: None,
            }],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let blocks_rs = &result.files[1].1;
        assert!(blocks_rs.contains("TODO"));
        assert!(blocks_rs.contains("127.0.0.1:9000"));
        assert!(blocks_rs.contains("pub fn block_1() -> Vec<u8>"));
    }

    #[test]
    fn parallel_two_groups_emits_thread_scope() {
        // Two disconnected chains: (1 -> 2) and (3 -> 4).
        let snap = GraphSnapshot {
            blocks: vec![
                make_constant_snapshot(1, 1.0),
                make_gain_snapshot(2, 2.0),
                make_constant_snapshot(3, 3.0),
                make_gain_snapshot(4, 4.0),
            ],
            channels: vec![ch(1, 1, 0, 2, 0), ch(2, 3, 0, 4, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let main_rs = &result.files[2].1;

        assert!(main_rs.contains("std::thread::scope"));
        assert!(main_rs.contains("s.spawn"));
        assert!(main_rs.contains("Group 0"));
        assert!(main_rs.contains("Group 1"));
        // Both chains should still have correct calls.
        assert!(main_rs.contains("blocks::block_1()"));
        assert!(main_rs.contains("blocks::block_2(out_1_p0)"));
        assert!(main_rs.contains("blocks::block_3()"));
        assert!(main_rs.contains("blocks::block_4(out_3_p0)"));
    }

    #[test]
    fn single_group_no_threads() {
        // Fully connected: 1 -> 2 -> 3.
        let snap = GraphSnapshot {
            blocks: vec![
                make_constant_snapshot(1, 1.0),
                make_gain_snapshot(2, 2.0),
                make_gain_snapshot(3, 3.0),
            ],
            channels: vec![ch(1, 1, 0, 2, 0), ch(2, 2, 0, 3, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let main_rs = &result.files[2].1;

        // Should NOT contain thread scope since it's a single group.
        assert!(!main_rs.contains("std::thread::scope"));
        assert!(!main_rs.contains("s.spawn"));
        // Should still have sequential calls.
        assert!(main_rs.contains("blocks::block_1()"));
        assert!(main_rs.contains("blocks::block_2(out_1_p0)"));
        assert!(main_rs.contains("blocks::block_3(out_2_p0)"));
    }

    #[test]
    fn embedded_blocks_emit_stubs() {
        let snap = GraphSnapshot {
            blocks: vec![
                BlockSnapshot {
                    id: 1,
                    block_type: "adc_source".to_string(),
                    name: "ADC Source".to_string(),
                    inputs: vec![],
                    outputs: vec![PortDef::new("value", PortKind::Float)],
                    config: serde_json::json!({ "channel": 2, "resolution_bits": 10 }),
                    output_values: vec![],
                    target: None,
                custom_codegen: None,
                },
                BlockSnapshot {
                    id: 2,
                    block_type: "pwm_sink".to_string(),
                    name: "PWM Sink".to_string(),
                    inputs: vec![PortDef::new("duty", PortKind::Float)],
                    outputs: vec![],
                    config: serde_json::json!({ "channel": 1, "frequency_hz": 2000 }),
                    output_values: vec![],
                    target: None,
                custom_codegen: None,
                },
            ],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let result = generate_rust(&snap, 0.01).unwrap();
        let blocks_rs = &result.files[1].1;

        // ADC stub
        assert!(blocks_rs.contains("TODO: Read ADC channel 2 (10-bit resolution)"));
        assert!(blocks_rs.contains("pub fn block_1() -> f64"));
        assert!(blocks_rs.contains("0.0 // stub: ADC read"));

        // PWM stub
        assert!(blocks_rs.contains("TODO: Set PWM channel 1 at 2000Hz"));
        assert!(blocks_rs.contains("pub fn block_2(_duty: f64)"));
    }

    // Workspace tests --------------------------------------------------------

    #[test]
    fn workspace_generates_all_files() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 42.0), make_gain_snapshot(2, 3.0)],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let paths: Vec<&str> = ws.files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"Cargo.toml"));
        assert!(paths.contains(&"logic/Cargo.toml"));
        assert!(paths.contains(&"logic/src/lib.rs"));
        assert!(paths.contains(&"logic/src/blocks.rs"));
        assert!(paths.contains(&"dataflow-rt/Cargo.toml"));
        assert!(paths.contains(&"dataflow-rt/src/lib.rs"));
        assert!(paths.contains(&"target-host/Cargo.toml"));
        assert!(paths.contains(&"target-host/src/main.rs"));
    }

    #[test]
    fn workspace_logic_lib_has_tick() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 42.0), make_gain_snapshot(2, 3.0)],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let lib_rs = ws
            .files
            .iter()
            .find(|(p, _)| p == "logic/src/lib.rs")
            .unwrap()
            .1
            .as_str();

        assert!(lib_rs.contains("pub fn tick(hw: &mut impl Peripherals, state: &mut State)"));
        assert!(lib_rs.contains("pub struct State"));
        assert!(lib_rs.contains("blocks::block_1()"));
        assert!(lib_rs.contains("blocks::block_2(state.out_1_p0)"));
    }

    #[test]
    fn workspace_adc_to_pwm_uses_peripherals() {
        let snap = GraphSnapshot {
            blocks: vec![
                BlockSnapshot {
                    id: 1,
                    block_type: "adc_source".to_string(),
                    name: "ADC".to_string(),
                    inputs: vec![],
                    outputs: vec![PortDef::new("value", PortKind::Float)],
                    config: serde_json::json!({ "channel": 0 }),
                    output_values: vec![],
                    target: None,
                custom_codegen: None,
                },
                make_gain_snapshot(2, 2.5),
                BlockSnapshot {
                    id: 3,
                    block_type: "pwm_sink".to_string(),
                    name: "PWM".to_string(),
                    inputs: vec![PortDef::new("duty", PortKind::Float)],
                    outputs: vec![],
                    config: serde_json::json!({ "channel": 0 }),
                    output_values: vec![],
                    target: None,
                custom_codegen: None,
                },
            ],
            channels: vec![ch(1, 1, 0, 2, 0), ch(2, 2, 0, 3, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let lib_rs = ws
            .files
            .iter()
            .find(|(p, _)| p == "logic/src/lib.rs")
            .unwrap()
            .1
            .as_str();

        assert!(lib_rs.contains("hw.adc_read(0)"));
        assert!(lib_rs.contains("hw.pwm_write(0,"));
    }

    #[test]
    fn workspace_multi_target() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 1.0)],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![
            TargetWithBinding {
                target: TargetFamily::Host,
                binding: Binding::host_default(),
            },
            TargetWithBinding {
                target: TargetFamily::Rp2040,
                binding: Binding {
                    target: TargetFamily::Rp2040,
                    pins: vec![],
                },
            },
        ];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let paths: Vec<&str> = ws.files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"target-host/Cargo.toml"));
        assert!(paths.contains(&"target-rp2040/Cargo.toml"));
        assert!(paths.contains(&"target-rp2040/memory.x"));
        assert!(paths.contains(&"target-rp2040/.cargo/config.toml"));
    }

    #[test]
    fn workspace_cargo_toml_has_members() {
        let snap = GraphSnapshot {
            blocks: vec![make_constant_snapshot(1, 1.0)],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let cargo = ws
            .files
            .iter()
            .find(|(p, _)| p == "Cargo.toml")
            .unwrap()
            .1
            .as_str();

        assert!(cargo.contains("[workspace]"));
        assert!(cargo.contains("\"logic\""));
        assert!(cargo.contains("\"target-host\""));
    }

    #[test]
    fn state_machine_codegen() {
        let snap = GraphSnapshot {
            blocks: vec![
                make_constant_snapshot(1, 1.0),
                BlockSnapshot {
                    id: 5,
                    block_type: "state_machine".to_string(),
                    name: "SM".to_string(),
                    inputs: vec![
                        PortDef::new("guard_0", PortKind::Float),
                        PortDef::new("guard_1", PortKind::Float),
                    ],
                    outputs: vec![
                        PortDef::new("state", PortKind::Float),
                        PortDef::new("active_idle", PortKind::Float),
                        PortDef::new("active_running", PortKind::Float),
                        PortDef::new("active_error", PortKind::Float),
                    ],
                    config: serde_json::json!({
                        "states": ["idle", "running", "error"],
                        "initial": "idle",
                        "transitions": [
                            { "from": "idle", "to": "running", "guard_port": 0 },
                            { "from": "running", "to": "error", "guard_port": 1 },
                            { "from": "error", "to": "idle", "guard_port": null }
                        ]
                    }),
                    output_values: vec![],
                    target: None,
                custom_codegen: None,
                },
            ],
            channels: vec![ch(1, 1, 0, 5, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let blocks_rs = ws
            .files
            .iter()
            .find(|(p, _)| p == "logic/src/blocks.rs")
            .unwrap()
            .1
            .as_str();

        assert!(blocks_rs.contains("Block5State"));
        assert!(blocks_rs.contains("Idle"));
        assert!(blocks_rs.contains("Running"));
        assert!(blocks_rs.contains("Error"));
        assert!(blocks_rs.contains("pub fn tick(&mut self"));

        let lib_rs = ws
            .files
            .iter()
            .find(|(p, _)| p == "logic/src/lib.rs")
            .unwrap()
            .1
            .as_str();

        assert!(lib_rs.contains("sm_5"));
        assert!(lib_rs.contains("state.sm_5.tick("));
    }

    #[test]
    fn unconnected_port_uses_literal_default_not_state_prefix() {
        // Gain block with no input connected — should use literal 0.0_f64, not state.0.0_f64
        let snap = GraphSnapshot {
            blocks: vec![make_gain_snapshot(2, 3.0)],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let lib_rs = ws
            .files
            .iter()
            .find(|(p, _)| p == "logic/src/lib.rs")
            .unwrap()
            .1
            .as_str();

        // Should contain the literal default, not prefixed with state.
        assert!(
            lib_rs.contains("blocks::block_2(0.0_f64)"),
            "Expected literal default 0.0_f64 for unconnected port, got:\n{lib_rs}"
        );
        assert!(
            !lib_rs.contains("state.0.0_f64"),
            "Bug: unconnected port produced state.0.0_f64"
        );
    }

    #[test]
    fn state_machine_has_output_fields_in_state_struct() {
        // State machine with 4 outputs — State struct must have both sm_5 AND out_5_p0..p3
        let snap = GraphSnapshot {
            blocks: vec![
                make_constant_snapshot(1, 1.0),
                BlockSnapshot {
                    id: 5,
                    block_type: "state_machine".to_string(),
                    name: "SM".to_string(),
                    inputs: vec![PortDef::new("guard_0", PortKind::Float)],
                    outputs: vec![
                        PortDef::new("state", PortKind::Float),
                        PortDef::new("active_idle", PortKind::Float),
                        PortDef::new("active_running", PortKind::Float),
                        PortDef::new("active_error", PortKind::Float),
                    ],
                    config: serde_json::json!({
                        "states": ["idle", "running", "error"],
                        "initial": "idle",
                        "transitions": [
                            { "from": "idle", "to": "running", "guard_port": 0 },
                            { "from": "running", "to": "error", "guard_port": null }
                        ]
                    }),
                    output_values: vec![],
                    target: None,
                custom_codegen: None,
                },
            ],
            channels: vec![ch(1, 1, 0, 5, 0)],
            tick_count: 0,
            time: 0.0,
        };
        let targets = vec![TargetWithBinding {
            target: TargetFamily::Host,
            binding: Binding::host_default(),
        }];
        let ws = generate_workspace(&snap, 0.01, &targets).unwrap();

        let lib_rs = ws
            .files
            .iter()
            .find(|(p, _)| p == "logic/src/lib.rs")
            .unwrap()
            .1
            .as_str();

        // State struct must have the state machine instance field
        assert!(lib_rs.contains("sm_5: blocks::Block5"), "Missing sm_5 field");
        // AND the output fields for tick() to write to
        assert!(lib_rs.contains("out_5_p0: f64"), "Missing out_5_p0 field");
        assert!(lib_rs.contains("out_5_p1: f64"), "Missing out_5_p1 field");
        assert!(lib_rs.contains("out_5_p2: f64"), "Missing out_5_p2 field");
        assert!(lib_rs.contains("out_5_p3: f64"), "Missing out_5_p3 field");

        // Default impl must also have both
        assert!(
            lib_rs.contains("sm_5: blocks::Block5::default()"),
            "Missing sm_5 default"
        );
        assert!(
            lib_rs.contains("out_5_p0: 0.0_f64"),
            "Missing out_5_p0 default"
        );
    }

    // Distributed multi-MCU tests -----------------------------------------------

    /// Helper: build a two-MCU graph with a cross-partition channel.
    ///
    /// Block 1: constant(5.0) on Rp2040
    /// Block 2: gain(2.0) on Stm32f4
    /// Channel: block1:0 -> block2:0 (cross-partition, forces pubsub bridge)
    fn make_distributed_graph() -> GraphSnapshot {
        let mut const_block = make_constant_snapshot(1, 5.0);
        const_block.target = Some(TargetFamily::Rp2040);

        let mut gain_block = make_gain_snapshot(2, 2.0);
        gain_block.target = Some(TargetFamily::Stm32f4);

        GraphSnapshot {
            blocks: vec![const_block, gain_block],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        }
    }

    /// Helper: build a DistributedConfig for two targets (Rp2040 + Stm32f4).
    fn make_two_target_config() -> DistributedConfig {
        DistributedConfig {
            targets: vec![
                TargetWithBinding {
                    target: TargetFamily::Rp2040,
                    binding: Binding {
                        target: TargetFamily::Rp2040,
                        pins: vec![],
                    },
                },
                TargetWithBinding {
                    target: TargetFamily::Stm32f4,
                    binding: Binding {
                        target: TargetFamily::Stm32f4,
                        pins: vec![],
                    },
                },
            ],
            dt: 0.01,
            transport: TransportConfig::Can,
        }
    }

    /// Helper: extract a file's content from a GeneratedWorkspace by path.
    fn ws_file<'a>(ws: &'a GeneratedWorkspace, path: &str) -> Option<&'a str> {
        ws.files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, content)| content.as_str())
    }

    // --- Test 1 ---
    #[test]
    fn distributed_produces_two_workspaces() {
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();
        assert_eq!(
            result.workspaces.len(),
            2,
            "Expected 2 workspaces, got {}",
            result.workspaces.len()
        );
    }

    // --- Test 2 ---
    #[test]
    fn distributed_workspace_keys_match_targets() {
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();
        assert!(
            result.workspaces.contains_key(&TargetFamily::Rp2040),
            "Missing Rp2040 workspace"
        );
        assert!(
            result.workspaces.contains_key(&TargetFamily::Stm32f4),
            "Missing Stm32f4 workspace"
        );
    }

    // --- Test 3 ---
    #[test]
    fn distributed_rp2040_has_logic_crate() {
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();
        let rp_ws = &result.workspaces[&TargetFamily::Rp2040];
        assert!(
            ws_file(rp_ws, "logic/src/lib.rs").is_some(),
            "Rp2040 workspace missing logic/src/lib.rs"
        );
    }

    // --- Test 4 ---
    #[test]
    fn distributed_stm32_has_logic_crate() {
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();
        let stm_ws = &result.workspaces[&TargetFamily::Stm32f4];
        assert!(
            ws_file(stm_ws, "logic/src/lib.rs").is_some(),
            "Stm32f4 workspace missing logic/src/lib.rs"
        );
    }

    // --- Test 5 ---
    #[test]
    fn distributed_rp2040_logic_has_pubsub_sink() {
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();
        let rp_ws = &result.workspaces[&TargetFamily::Rp2040];
        let lib_rs = ws_file(rp_ws, "logic/src/lib.rs").expect("missing logic/src/lib.rs");
        assert!(
            lib_rs.contains("pubsub_sink") || lib_rs.contains("pubsub::encode"),
            "Rp2040 logic lib.rs should contain pubsub sink/encode code, got:\n{lib_rs}"
        );
    }

    // --- Test 6 ---
    #[test]
    fn distributed_stm32_logic_has_pubsub_source() {
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();
        let stm_ws = &result.workspaces[&TargetFamily::Stm32f4];
        let lib_rs = ws_file(stm_ws, "logic/src/lib.rs").expect("missing logic/src/lib.rs");
        assert!(
            lib_rs.contains("pubsub_source") || lib_rs.contains("pubsub::decode"),
            "Stm32f4 logic lib.rs should contain pubsub source/decode code, got:\n{lib_rs}"
        );
    }

    // --- Test 7 ---
    #[test]
    fn distributed_single_target_no_bridges() {
        // All blocks on one target -- should produce one workspace with no pubsub code.
        let mut const_block = make_constant_snapshot(1, 5.0);
        const_block.target = Some(TargetFamily::Rp2040);

        let mut gain_block = make_gain_snapshot(2, 2.0);
        gain_block.target = Some(TargetFamily::Rp2040);

        let snap = GraphSnapshot {
            blocks: vec![const_block, gain_block],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };

        let config = DistributedConfig {
            targets: vec![TargetWithBinding {
                target: TargetFamily::Rp2040,
                binding: Binding {
                    target: TargetFamily::Rp2040,
                    pins: vec![],
                },
            }],
            dt: 0.01,
            transport: TransportConfig::Can,
        };

        let result = generate_distributed_workspace(&snap, &config).unwrap();
        assert_eq!(result.workspaces.len(), 1, "Expected 1 workspace for single target");

        let rp_ws = &result.workspaces[&TargetFamily::Rp2040];
        let lib_rs = ws_file(rp_ws, "logic/src/lib.rs").expect("missing logic/src/lib.rs");
        assert!(
            !lib_rs.contains("pubsub_sink") && !lib_rs.contains("pubsub_source"),
            "Single-target workspace should have no pubsub code, got:\n{lib_rs}"
        );
    }

    // --- Test 8 ---
    #[test]
    fn distributed_three_targets() {
        // Blocks spread across 3 targets produces 3 workspaces.
        let mut b1 = make_constant_snapshot(1, 1.0);
        b1.target = Some(TargetFamily::Rp2040);

        let mut b2 = make_gain_snapshot(2, 2.0);
        b2.target = Some(TargetFamily::Stm32f4);

        let mut b3 = make_gain_snapshot(3, 3.0);
        b3.target = Some(TargetFamily::Esp32c3);

        let snap = GraphSnapshot {
            blocks: vec![b1, b2, b3],
            channels: vec![ch(1, 1, 0, 2, 0), ch(2, 2, 0, 3, 0)],
            tick_count: 0,
            time: 0.0,
        };

        let config = DistributedConfig {
            targets: vec![
                TargetWithBinding {
                    target: TargetFamily::Rp2040,
                    binding: Binding {
                        target: TargetFamily::Rp2040,
                        pins: vec![],
                    },
                },
                TargetWithBinding {
                    target: TargetFamily::Stm32f4,
                    binding: Binding {
                        target: TargetFamily::Stm32f4,
                        pins: vec![],
                    },
                },
                TargetWithBinding {
                    target: TargetFamily::Esp32c3,
                    binding: Binding {
                        target: TargetFamily::Esp32c3,
                        pins: vec![],
                    },
                },
            ],
            dt: 0.01,
            transport: TransportConfig::Can,
        };

        let result = generate_distributed_workspace(&snap, &config).unwrap();
        assert_eq!(result.workspaces.len(), 3, "Expected 3 workspaces");
        assert!(result.workspaces.contains_key(&TargetFamily::Rp2040));
        assert!(result.workspaces.contains_key(&TargetFamily::Stm32f4));
        assert!(result.workspaces.contains_key(&TargetFamily::Esp32c3));
    }

    // --- Test 9 ---
    #[test]
    fn distributed_unassigned_block_errors() {
        // Block with target=None should return an error.
        let mut const_block = make_constant_snapshot(1, 5.0);
        const_block.target = None; // deliberately unassigned

        let snap = GraphSnapshot {
            blocks: vec![const_block],
            channels: vec![],
            tick_count: 0,
            time: 0.0,
        };

        let config = DistributedConfig {
            targets: vec![TargetWithBinding {
                target: TargetFamily::Rp2040,
                binding: Binding {
                    target: TargetFamily::Rp2040,
                    pins: vec![],
                },
            }],
            dt: 0.01,
            transport: TransportConfig::Can,
        };

        let result = generate_distributed_workspace(&snap, &config);
        assert!(
            result.is_err(),
            "Expected error for unassigned block, got Ok"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("partition") || err.contains("unassigned") || err.contains("Unassigned"),
            "Error should mention partition/unassigned, got: {err}"
        );
    }

    // --- Test 10 ---
    #[test]
    fn distributed_workspace_has_pubsub_dependency() {
        // When bridges exist, generated Cargo.toml should include pubsub dependency.
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();

        for (target, ws) in &result.workspaces {
            let cargo = ws_file(ws, "logic/Cargo.toml")
                .unwrap_or_else(|| panic!("missing logic/Cargo.toml for {target:?}"));
            assert!(
                cargo.contains("pubsub"),
                "logic/Cargo.toml for {target:?} should contain pubsub dependency, got:\n{cargo}"
            );
        }
    }

    // --- Test 11 ---
    #[test]
    fn distributed_no_bridges_no_pubsub_dep() {
        // Single-target workspace should have no pubsub dependency.
        let mut b1 = make_constant_snapshot(1, 5.0);
        b1.target = Some(TargetFamily::Rp2040);

        let mut b2 = make_gain_snapshot(2, 2.0);
        b2.target = Some(TargetFamily::Rp2040);

        let snap = GraphSnapshot {
            blocks: vec![b1, b2],
            channels: vec![ch(1, 1, 0, 2, 0)],
            tick_count: 0,
            time: 0.0,
        };

        let config = DistributedConfig {
            targets: vec![TargetWithBinding {
                target: TargetFamily::Rp2040,
                binding: Binding {
                    target: TargetFamily::Rp2040,
                    pins: vec![],
                },
            }],
            dt: 0.01,
            transport: TransportConfig::Can,
        };

        let result = generate_distributed_workspace(&snap, &config).unwrap();
        let rp_ws = &result.workspaces[&TargetFamily::Rp2040];
        let cargo = ws_file(rp_ws, "logic/Cargo.toml")
            .expect("missing logic/Cargo.toml");
        assert!(
            !cargo.contains("pubsub"),
            "Single-target workspace should not have pubsub dependency, got:\n{cargo}"
        );
    }

    // --- Test 12 ---
    #[test]
    fn distributed_bridge_topic_appears_in_code() {
        // The bridge topic name should appear in generated code.
        let snap = make_distributed_graph();
        let config = make_two_target_config();
        let result = generate_distributed_workspace(&snap, &config).unwrap();

        // The partition module generates topic names like "bridge_1_0".
        // At least one workspace should contain the topic string.
        let mut found = false;
        for ws in result.workspaces.values() {
            if let Some(lib_rs) = ws_file(ws, "logic/src/lib.rs") {
                if lib_rs.contains("bridge_1_0") {
                    found = true;
                    break;
                }
            }
        }
        assert!(
            found,
            "Expected bridge topic 'bridge_1_0' to appear in at least one workspace's logic/src/lib.rs"
        );
    }
}
