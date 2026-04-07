#![forbid(unsafe_code)]
//! IR → safe Rust emitter.
//!
//! Walks an [`IrModule`] and emits safe Rust source code defining:
//! - A `HardwareBridge` trait for peripheral I/O
//! - A `State` struct with one `f64` field per SSA value
//! - A `tick` function that executes the dataflow graph

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::ir::{ArithOp, Attr, DataflowOp, FuncOp, IrModule, IrOp, IrOpKind};

/// Emit safe Rust source code from an IrModule.
///
/// The generated code defines:
/// - A `State` struct with f64 fields for each SSA value
/// - A `fn tick(state: &mut State, hw: &mut dyn HardwareBridge)` dispatcher
/// - A `HardwareBridge` trait for peripheral I/O
pub fn emit_rust(module: &IrModule) -> String {
    let mut out = String::new();

    // Header
    out.push_str("//! Auto-generated from MLIR IR. Do not edit.\n");
    out.push_str("#![forbid(unsafe_code)]\n\n");

    // HardwareBridge trait
    emit_hw_bridge_trait(&mut out);

    // Find the @tick function (or first function)
    let tick_func = module
        .funcs
        .iter()
        .find(|f| f.name == "tick")
        .or_else(|| module.funcs.first());

    let Some(func) = tick_func else {
        // No functions — emit empty State and tick
        out.push_str(
            "/// Runtime state — one f64 slot per SSA value.\n\
             #[derive(Default)]\n\
             pub struct State {}\n\n\
             /// Execute one tick of the dataflow graph.\n\
             pub fn tick(_state: &mut State, _hw: &mut dyn HardwareBridge) {}\n",
        );
        return out;
    };

    // Collect all ValueIds used in this function.
    let mut value_ids = BTreeSet::new();

    // Function args get ValueIds starting from some base. We need to figure
    // out what those are. The IrBuilder allocates them sequentially, but we
    // don't have that info at this point. We'll infer arg ValueIds by looking
    // at operand references that aren't produced by any op result.
    let mut result_ids = BTreeSet::new();
    for op in &func.ops {
        for r in &op.results {
            result_ids.insert(r.0);
            value_ids.insert(r.0);
        }
        for o in &op.operands {
            value_ids.insert(o.0);
        }
    }

    // Arg ValueIds: operands not produced by any op result.
    // We pair them with the function's declared arg names.
    let arg_value_ids: Vec<u32> = {
        let mut ids: Vec<u32> = value_ids
            .iter()
            .copied()
            .filter(|id| !result_ids.contains(id))
            .collect();
        ids.sort();
        ids
    };

    // Emit State struct
    out.push_str("/// Runtime state \u{2014} one f64 slot per SSA value.\n");
    out.push_str("#[derive(Default)]\n");
    out.push_str("pub struct State {\n");
    for id in &value_ids {
        let _ = writeln!(out, "    pub v{id}: f64,");
    }
    out.push_str("}\n\n");

    // Emit tick function
    out.push_str("/// Execute one tick of the dataflow graph.\n");
    out.push_str("pub fn tick(state: &mut State, hw: &mut dyn HardwareBridge) {\n");

    // Assign function args to state fields
    for (i, arg_id) in arg_value_ids.iter().enumerate() {
        if i < func.args.len() {
            let arg_name = &func.args[i].0;
            let _ = writeln!(out, "    state.v{arg_id} = {arg_name};");
        }
    }

    // Emit each op
    for (idx, op) in func.ops.iter().enumerate() {
        emit_op(&mut out, idx, op);
    }

    out.push_str("}\n");

    out
}

/// Emit the HardwareBridge trait definition.
fn emit_hw_bridge_trait(out: &mut String) {
    out.push_str(
        "/// Hardware bridge trait for peripheral I/O.\n\
         pub trait HardwareBridge {\n\
         \x20   fn adc_read(&self, channel: u8) -> f64 { 0.0 }\n\
         \x20   fn pwm_write(&mut self, _channel: u8, _duty: f64) {}\n\
         \x20   fn gpio_read(&self, pin: u8) -> f64 { 0.0 }\n\
         \x20   fn gpio_write(&mut self, _pin: u8, _value: f64) {}\n\
         \x20   fn uart_read(&self, _port: u8) -> f64 { 0.0 }\n\
         \x20   fn uart_write(&mut self, _port: u8, _value: f64) {}\n\
         \x20   fn encoder_read(&self, _channel: u8) -> (f64, f64) { (0.0, 0.0) }\n\
         \x20   fn subscribe(&self, _topic: &str) -> f64 { 0.0 }\n\
         \x20   fn publish(&mut self, _topic: &str, _value: f64) {}\n\
         }\n\n",
    );
}

/// Format an f64 literal with `_f64` suffix.
fn fmt_f64(v: f64) -> String {
    let s = format!("{v}");
    // Ensure the literal has a decimal point
    if s.contains('.') {
        format!("{s}_f64")
    } else {
        format!("{s}.0_f64")
    }
}

/// Extract an f64 attribute value.
fn attr_f64(op: &IrOp, key: &str) -> f64 {
    match op.attrs.get(key) {
        Some(Attr::F64(v)) => *v,
        _ => 0.0,
    }
}

/// Extract an i64 attribute and return as u8.
fn attr_u8(op: &IrOp, key: &str) -> u8 {
    match op.attrs.get(key) {
        Some(Attr::I64(v)) => *v as u8,
        _ => 0,
    }
}

/// Extract a string attribute.
fn attr_str<'a>(op: &'a IrOp, key: &str) -> &'a str {
    match op.attrs.get(key) {
        Some(Attr::Str(s)) => s.as_str(),
        _ => "",
    }
}

/// Emit Rust code for a single IrOp.
fn emit_op(out: &mut String, idx: usize, op: &IrOp) {
    // Helper closures for common patterns
    let r = |i: usize| -> u32 { op.results.get(i).map(|v| v.0).unwrap_or(0) };
    let a = |i: usize| -> u32 { op.operands.get(i).map(|v| v.0).unwrap_or(0) };

    match &op.kind {
        IrOpKind::Arith(ArithOp::Constant) => {
            let val = attr_f64(op, "value");
            let _ = writeln!(out, "    // Op {idx}: arith.constant {{value = {val}}}");
            let _ = writeln!(out, "    state.v{} = {};", r(0), fmt_f64(val));
        }

        IrOpKind::Arith(ArithOp::Addf) => {
            let _ = writeln!(out, "    // Op {idx}: arith.addf(%{}, %{})", a(0), a(1));
            let _ = writeln!(out, "    state.v{} = state.v{} + state.v{};", r(0), a(0), a(1));
        }

        IrOpKind::Arith(ArithOp::Mulf) => {
            let _ = writeln!(out, "    // Op {idx}: arith.mulf(%{}, %{})", a(0), a(1));
            let _ = writeln!(out, "    state.v{} = state.v{} * state.v{};", r(0), a(0), a(1));
        }

        IrOpKind::Arith(ArithOp::Subf) => {
            let _ = writeln!(out, "    // Op {idx}: arith.subf(%{}, %{})", a(0), a(1));
            let _ = writeln!(out, "    state.v{} = state.v{} - state.v{};", r(0), a(0), a(1));
        }

        IrOpKind::Arith(ArithOp::Select) => {
            let _ = writeln!(out, "    // Op {idx}: arith.select (not yet implemented)");
        }

        IrOpKind::Dataflow(DataflowOp::Clamp) => {
            let lo = attr_f64(op, "lo");
            let hi = attr_f64(op, "hi");
            let _ = writeln!(out, "    // Op {idx}: dataflow.clamp {{lo = {lo}, hi = {hi}}}");
            let _ = writeln!(out, "    state.v{} = state.v{}.max({}).min({});", r(0), a(0), fmt_f64(lo), fmt_f64(hi));
        }

        IrOpKind::Dataflow(DataflowOp::AdcRead) => {
            let ch = attr_u8(op, "channel");
            let _ = writeln!(out, "    // Op {idx}: dataflow.adc_read {{channel = {ch}}}");
            let _ = writeln!(out, "    state.v{} = hw.adc_read({ch});", r(0));
        }

        IrOpKind::Dataflow(DataflowOp::PwmWrite) => {
            let ch = attr_u8(op, "channel");
            let _ = writeln!(out, "    // Op {idx}: dataflow.pwm_write {{channel = {ch}}}");
            let _ = writeln!(out, "    hw.pwm_write({ch}, state.v{});", a(0));
        }

        IrOpKind::Dataflow(DataflowOp::GpioRead) => {
            let pin = attr_u8(op, "pin");
            let _ = writeln!(out, "    // Op {idx}: dataflow.gpio_read {{pin = {pin}}}");
            let _ = writeln!(out, "    state.v{} = hw.gpio_read({pin});", r(0));
        }

        IrOpKind::Dataflow(DataflowOp::GpioWrite) => {
            let pin = attr_u8(op, "pin");
            let _ = writeln!(out, "    // Op {idx}: dataflow.gpio_write {{pin = {pin}}}");
            let _ = writeln!(out, "    hw.gpio_write({pin}, state.v{});", a(0));
        }

        IrOpKind::Dataflow(DataflowOp::UartRx) => {
            let port = attr_u8(op, "port");
            let _ = writeln!(out, "    // Op {idx}: dataflow.uart_rx {{port = {port}}}");
            let _ = writeln!(out, "    state.v{} = hw.uart_read({port});", r(0));
        }

        IrOpKind::Dataflow(DataflowOp::UartTx) => {
            let port = attr_u8(op, "port");
            let _ = writeln!(out, "    // Op {idx}: dataflow.uart_tx {{port = {port}}}");
            let _ = writeln!(out, "    hw.uart_write({port}, state.v{});", a(0));
        }

        IrOpKind::Dataflow(DataflowOp::EncoderRead) => {
            let ch = attr_u8(op, "channel");
            let _ = writeln!(out, "    // Op {idx}: dataflow.encoder_read {{channel = {ch}}}");
            let _ = writeln!(out, "    let (p, v) = hw.encoder_read({ch}); state.v{} = p; state.v{} = v;", r(0), r(1));
        }

        IrOpKind::Dataflow(DataflowOp::ChannelRead) => {
            let topic = attr_str(op, "topic");
            let _ = writeln!(out, "    // Op {idx}: dataflow.channel_read {{topic = \"{topic}\"}}");
            let _ = writeln!(out, "    state.v{} = hw.channel_read(\"{topic}\");", r(0));
        }

        IrOpKind::Dataflow(DataflowOp::ChannelWrite) => {
            let topic = attr_str(op, "topic");
            let _ = writeln!(out, "    // Op {idx}: dataflow.channel_write {{topic = \"{topic}\"}}");
            let _ = writeln!(out, "    hw.channel_write(\"{topic}\", state.v{});", a(0));
        }

        IrOpKind::Dataflow(DataflowOp::MessageFieldExtract) => {
            let field = attr_str(op, "field");
            let _ = writeln!(out, "    // Op {idx}: dataflow.message_field {{field = \"{field}\"}}");
            let _ = writeln!(out, "    state.v{} = hw.message_field(state.v{}, \"{field}\");", r(0), a(0));
        }

        IrOpKind::Dataflow(DataflowOp::StateMachine) => {
            let _ = writeln!(out, "    // Op {idx}: dataflow.state_machine (not yet implemented)");
        }

        IrOpKind::Func(FuncOp::Call { callee }) if callee == "subscribe" => {
            let topic = attr_str(op, "topic");
            let _ = writeln!(out, "    // Op {idx}: func.call @subscribe {{topic = \"{topic}\"}}");
            let _ = writeln!(out, "    state.v{} = hw.subscribe(\"{topic}\");", r(0));
        }

        IrOpKind::Func(FuncOp::Call { callee }) if callee == "publish" => {
            let topic = attr_str(op, "topic");
            let _ = writeln!(out, "    // Op {idx}: func.call @publish {{topic = \"{topic}\"}}");
            let _ = writeln!(out, "    hw.publish(\"{topic}\", state.v{});", a(0));
        }

        IrOpKind::Func(FuncOp::Call { callee }) => {
            let _ = writeln!(out, "    // Op {idx}: func.call @{callee} (unsupported callee)");
        }

        IrOpKind::Custom(s) => {
            let _ = writeln!(out, "    // unsupported: {s}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IrBuilder;

    #[test]
    fn test_emit_constant() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.constant_f64(42.0);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("state.v0 = 42.0_f64;"),
            "expected constant assignment, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_chain() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c1 = b.constant_f64(5.0);
        let c2 = b.constant_f64(2.0);
        let _result = b.mulf(c1, c2);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("state.v0 = 5.0_f64;"),
            "expected first constant, got:\n{code}"
        );
        assert!(
            code.contains("state.v1 = 2.0_f64;"),
            "expected second constant, got:\n{code}"
        );
        assert!(
            code.contains("state.v2 = state.v0 * state.v1;"),
            "expected mulf, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_hardware() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let adc_val = b.adc_read(3);
        b.pwm_write(1, adc_val);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("hw.adc_read(3)"),
            "expected adc_read call, got:\n{code}"
        );
        assert!(
            code.contains("hw.pwm_write(1, state.v0)"),
            "expected pwm_write call, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_pubsub() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.subscribe("sensor/temp");
        b.publish("actuator/fan", val);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("hw.subscribe(\"sensor/temp\")"),
            "expected subscribe call, got:\n{code}"
        );
        assert!(
            code.contains("hw.publish(\"actuator/fan\", state.v0)"),
            "expected publish call, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_clamp() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let input = b.constant_f64(150.0);
        let _clamped = b.clamp(input, 0.0, 100.0);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains(".max(0.0_f64).min(100.0_f64)"),
            "expected clamp chain, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_state_struct() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.constant_f64(1.0);
        b.constant_f64(2.0);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("pub struct State"),
            "expected State struct, got:\n{code}"
        );
        assert!(
            code.contains("pub v0: f64,"),
            "expected v0 field, got:\n{code}"
        );
        assert!(
            code.contains("pub v1: f64,"),
            "expected v1 field, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_hw_bridge_trait() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("pub trait HardwareBridge"),
            "expected HardwareBridge trait, got:\n{code}"
        );
        assert!(
            code.contains("fn adc_read"),
            "expected adc_read method, got:\n{code}"
        );
        assert!(
            code.contains("fn pwm_write"),
            "expected pwm_write method, got:\n{code}"
        );
        assert!(
            code.contains("fn subscribe"),
            "expected subscribe method, got:\n{code}"
        );
        assert!(
            code.contains("fn publish"),
            "expected publish method, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_encoder_read() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.encoder_read(2);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("hw.encoder_read(2)"),
            "expected encoder_read call, got:\n{code}"
        );
        assert!(
            code.contains("state.v0 = p; state.v1 = v;"),
            "expected position and velocity assignment, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_subf() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let a = b.constant_f64(10.0);
        let c = b.constant_f64(3.0);
        let _result = b.subf(a, c);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("state.v2 = state.v0 - state.v1;"),
            "expected subtraction, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_gpio_read_write() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.gpio_read(5);
        b.gpio_write(7, val);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("hw.gpio_read(5)"),
            "expected gpio_read call, got:\n{code}"
        );
        assert!(
            code.contains("hw.gpio_write(7, state.v0)"),
            "expected gpio_write call, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_uart_rx_tx() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.uart_rx(1);
        b.uart_tx(2, val);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("hw.uart_read(1)"),
            "expected uart_read call, got:\n{code}"
        );
        assert!(
            code.contains("hw.uart_write(2, state.v0)"),
            "expected uart_write call, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_unknown_op() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.custom_op("my.unknown_op", &[], &[], 0);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("// unsupported: my.unknown_op"),
            "expected unsupported comment, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_empty_module() {
        let module = IrModule { funcs: vec![] };
        let code = emit_rust(&module);
        assert!(
            code.contains("pub struct State {}"),
            "expected empty State struct, got:\n{code}"
        );
        assert!(
            code.contains("pub fn tick(_state: &mut State, _hw: &mut dyn HardwareBridge) {}"),
            "expected empty tick fn, got:\n{code}"
        );
    }

    #[test]
    fn test_emit_forbid_unsafe() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let module = b.build();
        let code = emit_rust(&module);
        assert!(
            code.contains("#![forbid(unsafe_code)]"),
            "expected forbid(unsafe_code), got:\n{code}"
        );
    }
}
