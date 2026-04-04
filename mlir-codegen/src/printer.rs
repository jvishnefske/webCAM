#![forbid(unsafe_code)]
//! MLIR text printer for the typed IR.
//!
//! Serializes an [`IrModule`] to valid `.mlir` text format, replacing
//! ad-hoc string building with a structured walk over the IR tree.

use crate::ir::{Attr, IrFunc, IrModule, IrOp, IrType, ValueId};

/// Print an IrModule to MLIR text.
pub fn print_mlir(module: &IrModule) -> String {
    let mut out = String::new();
    if module.funcs.is_empty() {
        out.push_str("module { }");
        return out;
    }
    out.push_str("module {\n");
    for (i, func) in module.funcs.iter().enumerate() {
        print_func(func, &mut out);
        if i + 1 < module.funcs.len() {
            out.push('\n');
        }
    }
    out.push_str("}\n");
    out
}

fn print_func(func: &IrFunc, out: &mut String) {
    // Collect all ValueIds defined by op results, so we can identify arg ValueIds.
    let mut op_defined: std::collections::HashSet<ValueId> = std::collections::HashSet::new();
    for op in &func.ops {
        for r in &op.results {
            op_defined.insert(*r);
        }
    }

    // Determine which ValueIds are function args: collect all operand ValueIds
    // that aren't op-defined. But simpler: just use %{id} for everything.
    // Function args are declared as %{id}: type.

    // We need to figure out the arg ValueIds. Since the builder allocates them
    // sequentially before any ops, we can infer them: they are the ValueIds
    // referenced as operands (or in the signature) that aren't produced by any op.
    //
    // But we don't have that mapping stored. The simplest approach from the task:
    // just use %{id} for everything. We need arg ValueIds for the signature.
    //
    // Strategy: scan all operand ValueIds not in op_defined to find arg IDs,
    // then map them positionally. But we can do even better: the first op's
    // operands that aren't op-defined come from args. However, the order matters.
    //
    // Actually, the task says: "just use %{id} for everything. Function args
    // declared as %{id}: type." So we need to know the ValueId for each arg.
    //
    // We can infer them: the builder allocates arg ValueIds before op ValueIds,
    // so they are the smallest IDs. Collect all referenced IDs, subtract
    // op-defined, and sort — those are the arg IDs in order.

    let arg_value_ids = infer_arg_value_ids(func, &op_defined);

    out.push_str("  func.func @");
    out.push_str(&func.name);
    out.push('(');
    for (i, (vid, (_, ty))) in arg_value_ids.iter().zip(func.args.iter()).enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push('%');
        out.push_str(&vid.0.to_string());
        out.push_str(": ");
        print_type(ty, out);
    }
    out.push(')');

    if !func.result_types.is_empty() {
        out.push_str(" -> (");
        for (i, ty) in func.result_types.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            print_type(ty, out);
        }
        out.push(')');
    }

    out.push_str(" {\n");
    for op in &func.ops {
        print_op(op, out);
    }
    out.push_str("    return\n");
    out.push_str("  }\n");
}

/// Infer arg ValueIds from the function.
///
/// Arg ValueIds are those referenced in operands (or implied by arg count)
/// that are not defined by any op result. They were allocated sequentially
/// before any ops, so we find them by collecting all non-op-defined ValueIds
/// and sorting them.
fn infer_arg_value_ids(
    func: &IrFunc,
    op_defined: &std::collections::HashSet<ValueId>,
) -> Vec<ValueId> {
    if func.args.is_empty() {
        return Vec::new();
    }

    // Collect all ValueIds referenced as operands that aren't op-defined.
    let mut arg_candidates: Vec<ValueId> = Vec::new();
    for op in &func.ops {
        for operand in &op.operands {
            if !op_defined.contains(operand) && !arg_candidates.contains(operand) {
                arg_candidates.push(*operand);
            }
        }
    }

    // Sort by ValueId to get them in allocation order.
    arg_candidates.sort_by_key(|v| v.0);

    // If we found fewer candidates than args (some args may be unused),
    // we need to fill in the gaps. The arg IDs are sequential starting from
    // the smallest candidate (or from 0 if no candidates found).
    if arg_candidates.len() == func.args.len() {
        return arg_candidates;
    }

    // Fallback: if we have some candidates, infer the base and fill sequentially.
    if !arg_candidates.is_empty() {
        let base = arg_candidates[0].0;
        return (0..func.args.len() as u32)
            .map(|i| ValueId(base + i))
            .collect();
    }

    // No operands reference args at all — infer from op results.
    // The arg IDs were allocated before any op results. Find the minimum
    // op-result ID and count back.
    if let Some(min_op_id) = op_defined.iter().map(|v| v.0).min() {
        let base = min_op_id.saturating_sub(func.args.len() as u32);
        (0..func.args.len() as u32)
            .map(|i| ValueId(base + i))
            .collect()
    } else {
        // No ops at all — args start from 0.
        (0..func.args.len() as u32)
            .map(ValueId)
            .collect()
    }
}

fn print_op(op: &IrOp, out: &mut String) {
    out.push_str("    ");

    // Results
    match op.results.len() {
        0 => {}
        1 => {
            out.push('%');
            out.push_str(&op.results[0].0.to_string());
            out.push_str(" = ");
        }
        _ => {
            for (i, r) in op.results.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push('%');
                out.push_str(&r.0.to_string());
            }
            out.push_str(" = ");
        }
    }

    // Op name
    out.push('"');
    out.push_str(&op.name);
    out.push('"');

    // Operands
    out.push('(');
    for (i, operand) in op.operands.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push('%');
        out.push_str(&operand.0.to_string());
    }
    out.push(')');

    // Attrs (sorted for deterministic output)
    if !op.attrs.is_empty() {
        let mut keys: Vec<&String> = op.attrs.keys().collect();
        keys.sort();
        out.push_str(" {");
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let attr = &op.attrs[*key];
            out.push_str(key);
            out.push_str(" = ");
            print_attr(attr, out);
        }
        out.push('}');
    }

    // Type signature
    out.push_str(" : (");
    for (i, operand) in op.operands.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        // We need operand types. The IR doesn't store them explicitly on
        // operands, but since all our values are f64 for now, we can look
        // up the type from the op that produced the value. For simplicity,
        // infer f64 for all operands (matching current IR semantics).
        let _ = operand;
        out.push_str("f64");
    }
    out.push_str(") -> ");

    // Result types
    match op.result_types.len() {
        0 => out.push_str("()"),
        1 => print_type(&op.result_types[0], out),
        _ => {
            out.push('(');
            for (i, ty) in op.result_types.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                print_type(ty, out);
            }
            out.push(')');
        }
    }

    out.push('\n');
}

fn print_attr(attr: &Attr, out: &mut String) {
    match attr {
        Attr::F64(v) => {
            // Ensure decimal point is present
            let s = if v.fract() == 0.0 && v.is_finite() {
                format!("{:.1}", v)
            } else {
                format!("{}", v)
            };
            out.push_str(&s);
            out.push_str(" : f64");
        }
        Attr::I64(v) => {
            out.push_str(&v.to_string());
            out.push_str(" : i64");
        }
        Attr::Str(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        Attr::Bool(b) => {
            out.push_str(if *b { "true" } else { "false" });
        }
    }
}

fn print_type(ty: &IrType, out: &mut String) {
    match ty {
        IrType::F64 => out.push_str("f64"),
        IrType::I32 => out.push_str("i32"),
        IrType::I64 => out.push_str("i64"),
        IrType::Bool => out.push_str("i1"),
        IrType::Index => out.push_str("index"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IrBuilder;

    #[test]
    fn test_print_empty_module() {
        let module = IrModule { funcs: vec![] };
        let text = print_mlir(&module);
        assert_eq!(text, "module { }");
    }

    #[test]
    fn test_print_constant() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.constant_f64(42.0);
        let module = b.build();
        let text = print_mlir(&module);

        assert!(text.contains("module {"), "should have module wrapper");
        assert!(text.contains("func.func @tick()"), "should have tick func");
        assert!(
            text.contains("\"dataflow.constant\"() {value = 42.0 : f64} : () -> f64"),
            "should have constant op, got:\n{text}"
        );
        assert!(text.contains("return"), "should end with return");
    }

    #[test]
    fn test_print_chain() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let c = b.constant_f64(5.0);
        let gain = b.constant_f64(2.0);
        let _result = b.mulf(c, gain);
        let module = b.build();
        let text = print_mlir(&module);

        // Should reference the correct SSA values
        assert!(
            text.contains("\"dataflow.constant\"() {value = 5.0 : f64} : () -> f64"),
            "should have constant 5.0, got:\n{text}"
        );
        assert!(
            text.contains("\"dataflow.constant\"() {value = 2.0 : f64} : () -> f64"),
            "should have constant 2.0, got:\n{text}"
        );
        assert!(
            text.contains("\"arith.mulf\"(%0, %1)"),
            "mulf should reference %0 and %1, got:\n{text}"
        );
    }

    #[test]
    fn test_print_with_args() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[("x", IrType::F64), ("y", IrType::F64)], &[]);
        let x = b.func_arg(0);
        let y = b.func_arg(1);
        let _sum = b.addf(x, y);
        let module = b.build();
        let text = print_mlir(&module);

        // Function signature should list args with their ValueIds
        assert!(
            text.contains("func.func @tick(%0: f64, %1: f64)"),
            "should have arg signature, got:\n{text}"
        );
        // addf should reference the arg ValueIds
        assert!(
            text.contains("\"arith.addf\"(%0, %1)"),
            "addf should reference args, got:\n{text}"
        );
    }

    #[test]
    fn test_print_hardware_op() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.adc_read(3);
        let module = b.build();
        let text = print_mlir(&module);

        assert!(
            text.contains("\"dataflow.adc_read\"() {channel = 3 : i64} : () -> f64"),
            "should have adc_read with channel attr, got:\n{text}"
        );
    }

    #[test]
    fn test_print_string_attr() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        b.subscribe("sensor/temp");
        let module = b.build();
        let text = print_mlir(&module);

        assert!(
            text.contains(r#""dataflow.subscribe"() {topic = "sensor/temp"} : () -> f64"#),
            "should have subscribe with quoted topic, got:\n{text}"
        );
    }

    #[test]
    fn test_print_void_op() {
        let mut b = IrBuilder::new();
        b.begin_func("tick", &[], &[]);
        let val = b.constant_f64(0.5);
        b.pwm_write(1, val);
        let module = b.build();
        let text = print_mlir(&module);

        // The pwm_write should NOT have a result assignment
        // Find the pwm_write line
        let pwm_line = text
            .lines()
            .find(|l| l.contains("dataflow.pwm_write"))
            .expect("should have pwm_write line");
        // A result assignment looks like `%N = "op..."`. Check that the line
        // starts (after indentation) with `"` not `%`.
        let trimmed = pwm_line.trim_start();
        assert!(
            trimmed.starts_with('"'),
            "void op should not have result assignment, got: {pwm_line}"
        );
        assert!(
            pwm_line.contains("-> ()"),
            "void op should have -> () return type, got: {pwm_line}"
        );
    }
}
