#![forbid(unsafe_code)]
//! Typed intermediate representation for the MLIR codegen pipeline.
//!
//! Instead of concatenating `.mlir` text strings, we build a typed AST
//! that can be printed to MLIR text or lowered to safe Rust callables.

use std::collections::HashMap;

/// SSA value reference (opaque identifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

/// Types in the IR.
#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    F64,
    I32,
    I64,
    Bool,
    Index,
}

/// Compile-time constant attribute.
#[derive(Debug, Clone, PartialEq)]
pub enum Attr {
    F64(f64),
    I64(i64),
    Str(String),
    Bool(bool),
}

// ── Dialect-namespaced operation kinds ──────────────────────────────────────

/// Dialect-namespaced operation kind (replaces string-based op identification).
#[derive(Debug, Clone, PartialEq)]
pub enum IrOpKind {
    /// Standard MLIR arithmetic dialect.
    Arith(ArithOp),
    /// Standard MLIR function dialect.
    Func(FuncOp),
    /// Custom dataflow dialect for hardware I/O.
    Dataflow(DataflowOp),
    /// Escape hatch for unknown/custom ops.
    Custom(String),
}

/// Standard MLIR `arith` dialect operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ArithOp {
    Constant,
    Addf,
    Mulf,
    Subf,
    Select,
}

/// Standard MLIR `func` dialect operations.
#[derive(Debug, Clone, PartialEq)]
pub enum FuncOp {
    /// `func.call @callee(args...)` — models pub/sub as function symbol calls.
    Call { callee: String },
}

/// Custom `dataflow` dialect for hardware I/O (no standard MLIR equivalent).
#[derive(Debug, Clone, PartialEq)]
pub enum DataflowOp {
    Clamp,
    AdcRead,
    PwmWrite,
    GpioRead,
    GpioWrite,
    UartRx,
    UartTx,
    EncoderRead,
    // NOTE: No stepper/stallguard/display — these are channel messages, not IR ops.
    /// `dataflow.channel_read "topic"` -- read from a named typed channel (subscribe).
    ChannelRead,
    /// `dataflow.channel_write "topic"` -- write to a named typed channel (publish).
    ChannelWrite,
    /// `dataflow.message_field "name"` -- extract a typed field from a message value.
    MessageFieldExtract,
    /// `dataflow.state_machine` -- region-based FSM op.
    StateMachine,
}

impl IrOpKind {
    /// Return the canonical MLIR op name string for this op kind.
    pub fn mlir_name(&self) -> String {
        match self {
            Self::Arith(ArithOp::Constant) => "arith.constant".into(),
            Self::Arith(ArithOp::Addf) => "arith.addf".into(),
            Self::Arith(ArithOp::Mulf) => "arith.mulf".into(),
            Self::Arith(ArithOp::Subf) => "arith.subf".into(),
            Self::Arith(ArithOp::Select) => "arith.select".into(),
            Self::Func(FuncOp::Call { callee }) => format!("func.call @{callee}"),
            Self::Dataflow(d) => format!("dataflow.{}", match d {
                DataflowOp::Clamp => "clamp",
                DataflowOp::AdcRead => "adc_read",
                DataflowOp::PwmWrite => "pwm_write",
                DataflowOp::GpioRead => "gpio_read",
                DataflowOp::GpioWrite => "gpio_write",
                DataflowOp::UartRx => "uart_rx",
                DataflowOp::UartTx => "uart_tx",
                DataflowOp::EncoderRead => "encoder_read",
                DataflowOp::ChannelRead => "channel_read",
                DataflowOp::ChannelWrite => "channel_write",
                DataflowOp::MessageFieldExtract => "message_field",
                DataflowOp::StateMachine => "state_machine",
            }),
            Self::Custom(s) => s.clone(),
        }
    }
}

/// A single operation in the IR.
#[derive(Debug, Clone)]
pub struct IrOp {
    /// Typed operation kind (dialect-namespaced enum).
    pub kind: IrOpKind,
    /// Input SSA values consumed by this op.
    pub operands: Vec<ValueId>,
    /// Output SSA values produced by this op.
    pub results: Vec<ValueId>,
    /// Named attributes (compile-time config).
    pub attrs: HashMap<String, Attr>,
    /// Types of results.
    pub result_types: Vec<IrType>,
}

/// A function in the IR (contains a flat list of ops).
#[derive(Debug, Clone)]
pub struct IrFunc {
    pub name: String,
    pub args: Vec<(String, IrType)>,
    pub result_types: Vec<IrType>,
    pub ops: Vec<IrOp>,
}

/// Top-level module containing functions.
#[derive(Debug, Clone)]
pub struct IrModule {
    pub funcs: Vec<IrFunc>,
}

/// Builder for constructing an [`IrModule`].
pub struct IrBuilder {
    funcs: Vec<IrFunc>,
    current_func: Option<usize>,
    next_value_id: u32,
}

impl IrBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            funcs: Vec::new(),
            current_func: None,
            next_value_id: 0,
        }
    }

    /// Start a new function. Returns function index.
    pub fn begin_func(
        &mut self,
        name: &str,
        args: &[(&str, IrType)],
        result_types: &[IrType],
    ) -> usize {
        let func_args: Vec<(String, IrType)> = args
            .iter()
            .map(|(n, t)| {
                let id = self.fresh_value();
                let _ = id; // consumed implicitly — arg ValueIds start from current next_value_id
                (n.to_string(), t.clone())
            })
            .collect();

        // The arg ValueIds were allocated in order above. We need to re-derive
        // them in func_arg(), so record the base id before we allocated them.
        // Actually, let's redo this more carefully: allocate arg ValueIds
        // explicitly so func_arg can return them.
        // We already allocated them in the map above, so the first arg is
        // (next_value_id - args.len()) and so on.

        let func = IrFunc {
            name: name.to_string(),
            args: func_args,
            result_types: result_types.to_vec(),
            ops: Vec::new(),
        };
        self.funcs.push(func);
        let idx = self.funcs.len() - 1;
        self.current_func = Some(idx);
        idx
    }

    /// Get the [`ValueId`] for a function argument by index.
    ///
    /// The argument ValueIds are allocated sequentially starting from the
    /// first value allocated when `begin_func` was called.
    pub fn func_arg(&self, arg_index: usize) -> ValueId {
        let func_idx = self.current_func.expect("no current function");
        let func = &self.funcs[func_idx];
        let num_args = func.args.len();
        assert!(
            arg_index < num_args,
            "arg_index {arg_index} out of range (function has {num_args} args)"
        );
        // The arg ValueIds were allocated right before the current func's ops.
        // They were the first `num_args` values allocated during begin_func.
        // The first arg got id = (next_value_id - num_args_at_time + 0), etc.
        // But we need to know what next_value_id was *before* begin_func allocated them.
        // That base = next_value_id_before_begin_func = current_next - num_args - (values allocated after begin_func).
        //
        // Simpler approach: count how many op results exist in the current func.
        let ops_results: u32 = func
            .ops
            .iter()
            .map(|op| op.results.len() as u32)
            .sum();
        let base = self.next_value_id - num_args as u32 - ops_results;
        ValueId(base + arg_index as u32)
    }

    /// Allocate a fresh [`ValueId`].
    fn fresh_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value_id);
        self.next_value_id += 1;
        id
    }

    // ── Core op emission ───────────────────────────────────────────

    /// Emit a generic named op with arbitrary operands, attrs, and result count.
    /// Returns the result [`ValueId`]s.
    pub fn custom_op(
        &mut self,
        name: &str,
        operands: &[ValueId],
        attrs: &[(&str, Attr)],
        num_results: usize,
    ) -> Vec<ValueId> {
        let results: Vec<ValueId> = (0..num_results).map(|_| self.fresh_value()).collect();
        let result_types = vec![IrType::F64; num_results];
        let attr_map: HashMap<String, Attr> = attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let op = IrOp {
            kind: IrOpKind::Custom(name.to_string()),
            operands: operands.to_vec(),
            results: results.clone(),
            attrs: attr_map,
            result_types,
        };
        let func_idx = self.current_func.expect("no current function");
        self.funcs[func_idx].ops.push(op);
        results
    }

    /// Emit a typed op with a dialect-namespaced [`IrOpKind`].
    /// The `name` field is derived from `kind.mlir_name()` for backward compat.
    pub fn typed_op(
        &mut self,
        kind: IrOpKind,
        operands: &[ValueId],
        attrs: &[(&str, Attr)],
        num_results: usize,
    ) -> Vec<ValueId> {
        let results: Vec<ValueId> = (0..num_results).map(|_| self.fresh_value()).collect();
        let result_types = vec![IrType::F64; num_results];
        let attr_map: HashMap<String, Attr> = attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let op = IrOp {
            kind,
            operands: operands.to_vec(),
            results: results.clone(),
            attrs: attr_map,
            result_types,
        };
        let func_idx = self.current_func.expect("no current function");
        self.funcs[func_idx].ops.push(op);
        results
    }

    // ── Arithmetic ops ─────────────────────────────────────────────

    /// Emit a constant f64 value. Returns the SSA value.
    pub fn constant_f64(&mut self, value: f64) -> ValueId {
        self.typed_op(
            IrOpKind::Arith(ArithOp::Constant),
            &[],
            &[("value", Attr::F64(value))],
            1,
        )[0]
    }

    /// Emit f64 addition: result = lhs + rhs.
    pub fn addf(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.typed_op(IrOpKind::Arith(ArithOp::Addf), &[lhs, rhs], &[], 1)[0]
    }

    /// Emit f64 multiplication: result = lhs * rhs.
    pub fn mulf(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.typed_op(IrOpKind::Arith(ArithOp::Mulf), &[lhs, rhs], &[], 1)[0]
    }

    /// Emit f64 subtraction: result = lhs - rhs.
    pub fn subf(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.typed_op(IrOpKind::Arith(ArithOp::Subf), &[lhs, rhs], &[], 1)[0]
    }

    /// Emit clamp: result = max(lo, min(hi, value)).
    pub fn clamp(&mut self, value: ValueId, lo: f64, hi: f64) -> ValueId {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::Clamp),
            &[value],
            &[("lo", Attr::F64(lo)), ("hi", Attr::F64(hi))],
            1,
        )[0]
    }

    /// Emit select: result = cond > 0 ? a : b.
    pub fn select(&mut self, cond: ValueId, a: ValueId, b: ValueId) -> ValueId {
        self.typed_op(
            IrOpKind::Arith(ArithOp::Select),
            &[cond, a, b],
            &[],
            1,
        )[0]
    }

    // ── Hardware I/O ops ───────────────────────────────────────────

    /// ADC read: result = hw.adc_read(channel).
    pub fn adc_read(&mut self, channel: u8) -> ValueId {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::AdcRead),
            &[],
            &[("channel", Attr::I64(channel as i64))],
            1,
        )[0]
    }

    /// PWM write: hw.pwm_write(channel, duty).
    pub fn pwm_write(&mut self, channel: u8, duty: ValueId) {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::PwmWrite),
            &[duty],
            &[("channel", Attr::I64(channel as i64))],
            0,
        );
    }

    /// GPIO read: result = hw.gpio_read(pin).
    pub fn gpio_read(&mut self, pin: u8) -> ValueId {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::GpioRead),
            &[],
            &[("pin", Attr::I64(pin as i64))],
            1,
        )[0]
    }

    /// GPIO write: hw.gpio_write(pin, value).
    pub fn gpio_write(&mut self, pin: u8, value: ValueId) {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::GpioWrite),
            &[value],
            &[("pin", Attr::I64(pin as i64))],
            0,
        );
    }

    /// UART receive: result = hw.uart_read(port).
    pub fn uart_rx(&mut self, port: u8) -> ValueId {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::UartRx),
            &[],
            &[("port", Attr::I64(port as i64))],
            1,
        )[0]
    }

    /// UART transmit: hw.uart_write(port, value).
    pub fn uart_tx(&mut self, port: u8, value: ValueId) {
        self.typed_op(
            IrOpKind::Dataflow(DataflowOp::UartTx),
            &[value],
            &[("port", Attr::I64(port as i64))],
            0,
        );
    }

    /// Encoder read: (position, velocity) = hw.encoder_read(channel).
    pub fn encoder_read(&mut self, channel: u8) -> (ValueId, ValueId) {
        let results = self.typed_op(
            IrOpKind::Dataflow(DataflowOp::EncoderRead),
            &[],
            &[("channel", Attr::I64(channel as i64))],
            2,
        );
        (results[0], results[1])
    }

    // ── Pub/Sub ops (modeled as func.call) ─────────────────────────

    /// Subscribe: result = func.call @subscribe(topic).
    pub fn subscribe(&mut self, topic: &str) -> ValueId {
        self.typed_op(
            IrOpKind::Func(FuncOp::Call { callee: "subscribe".into() }),
            &[],
            &[("topic", Attr::Str(topic.to_string()))],
            1,
        )[0]
    }

    /// Publish: func.call @publish(topic, value).
    pub fn publish(&mut self, topic: &str, value: ValueId) {
        self.typed_op(
            IrOpKind::Func(FuncOp::Call { callee: "publish".into() }),
            &[value],
            &[("topic", Attr::Str(topic.to_string()))],
            0,
        );
    }

    // ── Build ──────────────────────────────────────────────────────

    /// Finalize and return the [`IrModule`].
    pub fn build(self) -> IrModule {
        IrModule { funcs: self.funcs }
    }
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_constant() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.constant_f64(42.0);
        let module = b.build();

        assert_eq!(module.funcs.len(), 1);
        assert_eq!(module.funcs[0].ops.len(), 1);

        let op = &module.funcs[0].ops[0];
        assert_eq!(op.kind, IrOpKind::Arith(ArithOp::Constant));
        assert_eq!(op.operands.len(), 0);
        assert_eq!(op.results.len(), 1);
        assert_eq!(op.attrs.get("value"), Some(&Attr::F64(42.0)));
    }

    #[test]
    fn test_builder_chain() {
        // constant(5.0) -> mulf(constant, gain_factor) chain
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(5.0);
        let gain = b.constant_f64(2.0);
        let _result = b.mulf(c, gain);
        let module = b.build();

        assert_eq!(module.funcs[0].ops.len(), 3);

        // Op 0: constant 5.0
        assert_eq!(module.funcs[0].ops[0].kind, IrOpKind::Arith(ArithOp::Constant));
        // Op 1: constant 2.0 (gain factor)
        assert_eq!(module.funcs[0].ops[1].kind, IrOpKind::Arith(ArithOp::Constant));
        // Op 2: mulf
        let mulf_op = &module.funcs[0].ops[2];
        assert_eq!(mulf_op.kind, IrOpKind::Arith(ArithOp::Mulf));
        assert_eq!(mulf_op.operands.len(), 2);
        // The mulf should consume the results of the two constants
        assert_eq!(mulf_op.operands[0], module.funcs[0].ops[0].results[0]);
        assert_eq!(mulf_op.operands[1], module.funcs[0].ops[1].results[0]);
    }

    #[test]
    fn test_builder_hardware_ops() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let adc_val = b.adc_read(3);
        b.pwm_write(1, adc_val);
        let module = b.build();

        assert_eq!(module.funcs[0].ops.len(), 2);

        let adc_op = &module.funcs[0].ops[0];
        assert_eq!(adc_op.kind, IrOpKind::Dataflow(DataflowOp::AdcRead));
        assert_eq!(adc_op.attrs.get("channel"), Some(&Attr::I64(3)));
        assert_eq!(adc_op.results.len(), 1);

        let pwm_op = &module.funcs[0].ops[1];
        assert_eq!(pwm_op.kind, IrOpKind::Dataflow(DataflowOp::PwmWrite));
        assert_eq!(pwm_op.attrs.get("channel"), Some(&Attr::I64(1)));
        assert_eq!(pwm_op.operands.len(), 1);
        assert_eq!(pwm_op.operands[0], adc_val);
        assert_eq!(pwm_op.results.len(), 0);
    }

    #[test]
    fn test_builder_pubsub() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.subscribe("sensor/temp");
        b.publish("actuator/fan", val);
        let module = b.build();

        assert_eq!(module.funcs[0].ops.len(), 2);

        let sub_op = &module.funcs[0].ops[0];
        assert_eq!(sub_op.kind, IrOpKind::Func(FuncOp::Call { callee: "subscribe".into() }));
        assert_eq!(
            sub_op.attrs.get("topic"),
            Some(&Attr::Str("sensor/temp".to_string()))
        );

        let pub_op = &module.funcs[0].ops[1];
        assert_eq!(pub_op.kind, IrOpKind::Func(FuncOp::Call { callee: "publish".into() }));
        assert_eq!(
            pub_op.attrs.get("topic"),
            Some(&Attr::Str("actuator/fan".to_string()))
        );
        assert_eq!(pub_op.operands[0], val);
    }

    #[test]
    fn test_builder_clamp() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let input = b.constant_f64(150.0);
        let _clamped = b.clamp(input, 0.0, 100.0);
        let module = b.build();

        let clamp_op = &module.funcs[0].ops[1];
        assert_eq!(clamp_op.kind, IrOpKind::Dataflow(DataflowOp::Clamp));
        assert_eq!(clamp_op.attrs.get("lo"), Some(&Attr::F64(0.0)));
        assert_eq!(clamp_op.attrs.get("hi"), Some(&Attr::F64(100.0)));
        assert_eq!(clamp_op.operands.len(), 1);
        assert_eq!(clamp_op.operands[0], input);
        assert_eq!(clamp_op.results.len(), 1);
    }

    #[test]
    fn test_custom_op() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let input = b.constant_f64(1.0);
        let results = b.custom_op(
            "my.custom_op",
            &[input],
            &[
                ("alpha", Attr::F64(0.5)),
                ("label", Attr::Str("test".to_string())),
            ],
            2,
        );
        let module = b.build();

        assert_eq!(results.len(), 2);

        let op = &module.funcs[0].ops[1];
        assert_eq!(op.kind, IrOpKind::Custom("my.custom_op".into()));
        assert_eq!(op.kind, IrOpKind::Custom("my.custom_op".into()));
        assert_eq!(op.operands, vec![input]);
        assert_eq!(op.results.len(), 2);
        assert_eq!(op.attrs.get("alpha"), Some(&Attr::F64(0.5)));
        assert_eq!(
            op.attrs.get("label"),
            Some(&Attr::Str("test".to_string()))
        );
        assert_eq!(op.result_types.len(), 2);
    }

    #[test]
    fn test_default_builder() {
        let b = IrBuilder::default();
        let module = b.build();
        assert!(module.funcs.is_empty());
    }

    #[test]
    fn test_custom_op_mlir_name() {
        let kind = IrOpKind::Custom("my.custom_op".to_string());
        assert_eq!(kind.mlir_name(), "my.custom_op");
    }

    #[test]
    fn test_subf() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let a = b.constant_f64(10.0);
        let c = b.constant_f64(3.0);
        let result = b.subf(a, c);
        let module = b.build();

        let sub_op = &module.funcs[0].ops[2];
        assert_eq!(sub_op.kind, IrOpKind::Arith(ArithOp::Subf));
        assert_eq!(sub_op.operands, vec![a, c]);
        assert_eq!(sub_op.results, vec![result]);
    }

    #[test]
    fn test_gpio_read() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.gpio_read(5);
        let module = b.build();

        let op = &module.funcs[0].ops[0];
        assert_eq!(op.kind, IrOpKind::Dataflow(DataflowOp::GpioRead));
        assert_eq!(op.attrs.get("pin"), Some(&Attr::I64(5)));
        assert_eq!(op.results.len(), 1);
        assert_eq!(op.results[0], val);
    }

    #[test]
    fn test_gpio_write() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.constant_f64(1.0);
        b.gpio_write(7, val);
        let module = b.build();

        let op = &module.funcs[0].ops[1];
        assert_eq!(op.kind, IrOpKind::Dataflow(DataflowOp::GpioWrite));
        assert_eq!(op.attrs.get("pin"), Some(&Attr::I64(7)));
        assert_eq!(op.operands, vec![val]);
        assert_eq!(op.results.len(), 0);
    }

    #[test]
    fn test_uart_rx() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.uart_rx(2);
        let module = b.build();

        let op = &module.funcs[0].ops[0];
        assert_eq!(op.kind, IrOpKind::Dataflow(DataflowOp::UartRx));
        assert_eq!(op.attrs.get("port"), Some(&Attr::I64(2)));
        assert_eq!(op.results.len(), 1);
        assert_eq!(op.results[0], val);
    }

    #[test]
    fn test_uart_tx() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.constant_f64(99.0);
        b.uart_tx(3, val);
        let module = b.build();

        let op = &module.funcs[0].ops[1];
        assert_eq!(op.kind, IrOpKind::Dataflow(DataflowOp::UartTx));
        assert_eq!(op.attrs.get("port"), Some(&Attr::I64(3)));
        assert_eq!(op.operands, vec![val]);
        assert_eq!(op.results.len(), 0);
    }

    #[test]
    fn test_encoder_read() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let (pos, vel) = b.encoder_read(4);
        let module = b.build();

        let op = &module.funcs[0].ops[0];
        assert_eq!(op.kind, IrOpKind::Dataflow(DataflowOp::EncoderRead));
        assert_eq!(op.attrs.get("channel"), Some(&Attr::I64(4)));
        assert_eq!(op.results.len(), 2);
        assert_eq!(op.results[0], pos);
        assert_eq!(op.results[1], vel);
    }

    #[test]
    fn test_func_arg_with_multiple_args() {
        let mut b = IrBuilder::new();
        b.begin_func(
            "tick",
            &[("a", IrType::F64), ("b", IrType::F64), ("c", IrType::F64)],
            &[],
        );
        let a = b.func_arg(0);
        let b_arg = b.func_arg(1);
        let c = b.func_arg(2);
        // All should be unique
        assert_ne!(a, b_arg);
        assert_ne!(b_arg, c);
        assert_ne!(a, c);
    }

    #[test]
    fn test_value_id_uniqueness() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[("x", IrType::F64)], &[]);
        let arg0 = b.func_arg(0);
        let c1 = b.constant_f64(1.0);
        let c2 = b.constant_f64(2.0);
        let sum = b.addf(c1, c2);
        let prod = b.mulf(sum, arg0);
        let _clamped = b.clamp(prod, -10.0, 10.0);

        // Collect all ValueIds
        let mut all_ids = vec![arg0, c1, c2, sum, prod, _clamped];

        // Also check that builder-returned ids match the op results in the module
        let module = b.build();
        for op in &module.funcs[0].ops {
            for r in &op.results {
                if !all_ids.contains(r) {
                    all_ids.push(*r);
                }
            }
        }

        // All ids should be unique
        let mut seen = std::collections::HashSet::new();
        for id in &all_ids {
            assert!(
                seen.insert(id),
                "duplicate ValueId found: {:?}",
                id
            );
        }
    }
}
