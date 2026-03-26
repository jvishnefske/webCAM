//! Lower state machine blocks to MLIR region-based control flow.
//!
//! Each state machine becomes a `dataflow.state_machine` op with one region
//! per state. Guard inputs drive transitions via `cf.cond_br` within regions.

use std::fmt::Write;

use crate::dialect;
use crate::lower::BlockSnapshot;

/// Emit MLIR type declarations for a state machine block.
///
/// This produces a comment-based "type declaration" documenting the states,
/// since the actual encoding uses integer indices in the memref.
pub fn emit_state_machine_type(out: &mut String, block: &BlockSnapshot) -> Result<(), String> {
    let id = block.id;
    let states = extract_states(block)?;
    let initial = extract_initial(block, &states);

    writeln!(
        out,
        "    // State machine block {id}: states = {states:?}, initial = \"{initial}\""
    )
    .unwrap();
    Ok(())
}

/// Emit the state machine tick logic as MLIR ops.
///
/// The state machine is lowered to a `dataflow.state_machine` op that takes
/// guard inputs and produces outputs: (state_index, active_flag_0, active_flag_1, ...).
pub fn emit_state_machine_tick(
    out: &mut String,
    id: u32,
    block: &BlockSnapshot,
    inputs: &[String],
) -> Result<(), String> {
    let states = extract_states(block)?;
    let initial = extract_initial(block, &states);
    let transitions = extract_transitions(block);

    let n_outputs = 1 + states.len(); // state index + one active flag per state

    // Build the guard operand list
    let guard_operands = if inputs.is_empty() {
        String::new()
    } else {
        format!("({})", inputs.join(", "))
    };

    // Emit the state_machine op with regions
    writeln!(
        out,
        "    // FSM: {n_outputs} outputs (index + {} active flags)",
        states.len()
    )
    .unwrap();

    // Emit state machine as region-based op
    let result_types: Vec<&str> = (0..n_outputs).map(|_| "f64").collect();
    let result_type_str = result_types.join(", ");

    // Use individual SSA names for multi-output
    let result_names: Vec<String> = (0..n_outputs).map(|p| dialect::ssa_name(id, p)).collect();
    let result_name_str = result_names.join(", ");

    writeln!(
        out,
        "    {result_name_str} = {op}{guard_operands} {{",
        op = dialect::OP_STATE_MACHINE,
    )
    .unwrap();

    // Emit one region block per state
    for (state_idx, state_name) in states.iter().enumerate() {
        let label = to_snake_case(state_name);
        let is_initial = state_name == &initial;
        let initial_marker = if is_initial { " // initial" } else { "" };

        writeln!(out, "    ^{label}:{initial_marker}").unwrap();

        // Find transitions from this state
        let from_transitions: Vec<&serde_json::Value> = transitions
            .iter()
            .filter(|t| t.get("from").and_then(|v| v.as_str()) == Some(state_name))
            .collect();

        if from_transitions.is_empty() {
            // Stay in current state
            writeln!(out, "        // no transitions — stay in {state_name}").unwrap();
        } else {
            for t in &from_transitions {
                let to_state = t.get("to").and_then(|v| v.as_str()).unwrap_or(state_name);
                let guard_port = t.get("guard_port").and_then(|v| v.as_u64());

                if let Some(port) = guard_port {
                    let guard_ssa = inputs
                        .get(port as usize)
                        .map(|s| s.as_str())
                        .unwrap_or("%zero");
                    let to_label = to_snake_case(to_state);
                    writeln!(out, "        // if guard_{port} > 0.5 → {to_state}").unwrap();
                    let cmp_ssa = format!("%sm{id}_cmp_{state_idx}_{port}");
                    let thresh_ssa = format!("%sm{id}_thresh_{state_idx}");
                    writeln!(out, "        {thresh_ssa} = arith.constant 0.5 : f64").unwrap();
                    writeln!(
                        out,
                        "        {cmp_ssa} = arith.cmpf \"ogt\", {guard_ssa}, {thresh_ssa} : f64"
                    )
                    .unwrap();
                    writeln!(out, "        cf.cond_br {cmp_ssa}, ^{to_label}, ^{label}").unwrap();
                } else {
                    // Unconditional transition
                    let to_label = to_snake_case(to_state);
                    writeln!(
                        out,
                        "        cf.br ^{to_label}  // unconditional → {to_state}"
                    )
                    .unwrap();
                }
            }
        }
    }

    writeln!(out, "    }} -> ({result_type_str})").unwrap();
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_states(block: &BlockSnapshot) -> Result<Vec<String>, String> {
    let states: Vec<String> = block
        .config
        .get("states")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if states.is_empty() {
        return Err(format!("state_machine block {} has no states", block.id));
    }
    Ok(states)
}

fn extract_initial(block: &BlockSnapshot, states: &[String]) -> String {
    block
        .config
        .get("initial")
        .and_then(|v| v.as_str())
        .unwrap_or(&states[0])
        .to_string()
}

fn extract_transitions(block: &BlockSnapshot) -> Vec<serde_json::Value> {
    block
        .config
        .get("transitions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn to_snake_case(s: &str) -> String {
    s.to_lowercase().replace([' ', '-'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::PortDef;
    use module_traits::value::PortKind;

    fn make_sm_block() -> BlockSnapshot {
        BlockSnapshot {
            id: 10,
            block_type: "state_machine".to_string(),
            name: "my_fsm".to_string(),
            inputs: vec![
                PortDef {
                    name: "guard_0".to_string(),
                    kind: PortKind::Float,
                },
                PortDef {
                    name: "guard_1".to_string(),
                    kind: PortKind::Float,
                },
            ],
            outputs: vec![
                PortDef {
                    name: "state_idx".to_string(),
                    kind: PortKind::Float,
                },
                PortDef {
                    name: "active_idle".to_string(),
                    kind: PortKind::Float,
                },
                PortDef {
                    name: "active_running".to_string(),
                    kind: PortKind::Float,
                },
            ],
            config: serde_json::json!({
                "states": ["idle", "running"],
                "initial": "idle",
                "transitions": [
                    {"from": "idle", "to": "running", "guard_port": 0},
                    {"from": "running", "to": "idle", "guard_port": 1}
                ]
            }),
            output_values: vec![],
            custom_codegen: None,
        }
    }

    #[test]
    fn emit_sm_type_header() {
        let block = make_sm_block();
        let mut out = String::new();
        emit_state_machine_type(&mut out, &block).unwrap();
        assert!(out.contains("idle"));
        assert!(out.contains("running"));
    }

    #[test]
    fn emit_sm_tick_regions() {
        let block = make_sm_block();
        let inputs = vec!["%g0".to_string(), "%g1".to_string()];
        let mut out = String::new();
        emit_state_machine_tick(&mut out, 10, &block, &inputs).unwrap();
        assert!(out.contains("dataflow.state_machine"));
        assert!(out.contains("^idle"));
        assert!(out.contains("^running"));
        assert!(out.contains("cf.cond_br"));
        assert!(out.contains("%v10_p0"));
    }

    #[test]
    fn extract_states_empty_error() {
        let block = BlockSnapshot {
            id: 1,
            block_type: "state_machine".to_string(),
            name: "bad".to_string(),
            inputs: vec![],
            outputs: vec![],
            config: serde_json::json!({"states": []}),
            output_values: vec![],
            custom_codegen: None,
        };
        assert!(extract_states(&block).is_err());
    }
}
