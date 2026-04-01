//! Lower a [`ConfigurableBlock`] to DAG IL.

use dag_core::op::{Dag, DagError};
use dag_core::templates::BlockPorts;

use crate::schema::{ChannelDirection, ChannelKind, ConfigField, DeclaredChannel};

/// Result of lowering a configurable block into DAG ops.
pub struct LowerResult {
    /// Named input/output ports for wiring to other blocks.
    pub ports: BlockPorts,
    /// The DAG containing the lowered ops (caller may merge into a larger DAG).
    pub dag: Dag,
}

/// Trait implemented by configurable blocks to lower their logic to DAG IL.
///
/// The block reads its current configuration, then emits a sequence of
/// [`dag_core::op::Op`] nodes into a fresh DAG. The returned [`LowerResult`]
/// contains the DAG and named ports for external wiring.
pub trait ConfigurableBlock {
    /// Unique block type identifier (e.g. "pid", "moving_average").
    fn block_type(&self) -> &str;

    /// Human-readable display name.
    fn display_name(&self) -> &str;

    /// Category for sub-menu placement.
    fn category(&self) -> crate::schema::BlockCategory;

    /// Configuration schema — the list of editable fields.
    fn config_schema(&self) -> Vec<ConfigField>;

    /// Current configuration as JSON.
    fn config_json(&self) -> serde_json::Value;

    /// Apply a JSON configuration (partial or full update).
    fn apply_config(&mut self, config: &serde_json::Value);

    /// Declared channels (pubsub topics and hardware I/O names).
    ///
    /// These are derived from the current config — e.g. changing the
    /// "setpoint_topic" field changes the declared input channel.
    fn declared_channels(&self) -> Vec<DeclaredChannel>;

    /// Lower this block to DAG IL.
    ///
    /// Emits ops into a fresh `Dag` and returns named ports. The caller
    /// can merge this DAG into a larger graph or CBOR-encode it directly
    /// for MCU deployment.
    fn lower(&self) -> Result<LowerResult, DagError>;
}

/// Convenience: lower a block and CBOR-encode the resulting DAG.
pub fn lower_and_encode(block: &dyn ConfigurableBlock) -> Result<Vec<u8>, String> {
    let result = block.lower().map_err(|e| format!("{:?}", e))?;
    Ok(dag_core::cbor::encode_dag(&result.dag))
}

/// Lower a block and return a human-readable IL listing of the DAG ops.
pub fn lower_to_il_text(block: &dyn ConfigurableBlock) -> Result<String, String> {
    let result = block.lower().map_err(|e| format!("{:?}", e))?;
    let mut lines = Vec::new();
    for (i, op) in result.dag.nodes().iter().enumerate() {
        lines.push(format!("  %{} = {:?}", i, op));
    }

    let mut text = String::new();
    text.push_str(&format!("block @{} {{\n", block.block_type()));

    // Inputs
    let inputs: Vec<_> = block
        .declared_channels()
        .into_iter()
        .filter(|ch| ch.direction == ChannelDirection::Input)
        .collect();
    if !inputs.is_empty() {
        text.push_str("  // inputs\n");
        for ch in &inputs {
            let kind_label = match ch.kind {
                ChannelKind::PubSub => "pubsub",
                ChannelKind::Hardware => "hw",
            };
            text.push_str(&format!(
                "  //   {} \"{}\" ({})\n",
                kind_label, ch.name, "in"
            ));
        }
    }

    // Outputs
    let outputs: Vec<_> = block
        .declared_channels()
        .into_iter()
        .filter(|ch| ch.direction == ChannelDirection::Output)
        .collect();
    if !outputs.is_empty() {
        text.push_str("  // outputs\n");
        for ch in &outputs {
            let kind_label = match ch.kind {
                ChannelKind::PubSub => "pubsub",
                ChannelKind::Hardware => "hw",
            };
            text.push_str(&format!(
                "  //   {} \"{}\" ({})\n",
                kind_label, ch.name, "out"
            ));
        }
    }

    text.push('\n');
    for line in &lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("}\n");
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lower_to_il_text() {
        let pid = crate::blocks::pid::PidBlock::default();
        let text = lower_to_il_text(&pid).expect("lower failed");
        assert!(text.contains("block @pid"));
        assert!(text.contains("Subscribe"));
        assert!(text.contains("Publish"));
    }

    #[test]
    fn test_lower_and_encode() {
        let pid = crate::blocks::pid::PidBlock::default();
        let bytes = lower_and_encode(&pid).expect("encode failed");
        // Should produce valid CBOR
        let decoded = dag_core::cbor::decode_dag(&bytes).expect("decode failed");
        assert!(!decoded.is_empty());
    }
}
