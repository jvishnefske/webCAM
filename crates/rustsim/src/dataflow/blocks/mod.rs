//! Built-in block implementations.
//!
//! The primary creation path uses [`data_driven::DataDrivenBlock`] driven by
//! [`module_traits::builtin_function_defs`].  Legacy per-struct blocks
//! (embedded, state_machine, udp) are kept for features not yet expressible
//! as pure `FunctionDef` values.

pub mod constant;
pub mod data_driven;
pub mod embedded;
pub mod function;
pub mod plot;
pub mod pubsub;
pub mod registry;
pub mod serde_block;
pub mod state_machine;
pub mod udp;

use self::registry::BlockRegistration;
use super::block::Module;
use module_traits::function_def::builtin_function_defs;

/// Collect legacy block registrations (blocks not yet converted to data-driven).
fn legacy_registrations() -> Vec<BlockRegistration> {
    let mut reg = Vec::new();
    // Only register block types that are NOT covered by FunctionDef.
    // Legacy blocks: embedded peripherals (need SimModel), state_machine, udp
    embedded::register(&mut reg);
    state_machine::register(&mut reg);
    udp::register(&mut reg);
    reg
}

/// Create a block from its type name and JSON config.
///
/// Tries the data-driven [`FunctionDef`] registry first, then falls back
/// to legacy per-struct block registrations.
pub fn create_block(block_type: &str, config_json: &str) -> Result<Box<dyn Module>, String> {
    // Try data-driven first
    let defs = builtin_function_defs();
    if let Some(def) = defs.into_iter().find(|d| d.id == block_type) {
        return data_driven::DataDrivenBlock::new(def, config_json)
            .map(|b| Box::new(b) as Box<dyn Module>);
    }

    // Fall back to legacy per-struct blocks
    legacy_registrations()
        .iter()
        .find(|r| r.block_type == block_type)
        .map(|r| (r.create_from_json)(config_json))
        .unwrap_or_else(|| Err(format!("unknown block type: {block_type}")))
}

/// List all available block types for the palette.
///
/// Merges data-driven function defs with legacy block registrations.
pub fn available_block_types() -> Vec<BlockTypeInfo> {
    let mut types: Vec<BlockTypeInfo> = builtin_function_defs()
        .iter()
        .map(|d| BlockTypeInfo {
            block_type: leak_str(&d.id),
            name: leak_str(&d.display_name),
            category: leak_str(&d.category),
        })
        .collect();

    // Add legacy blocks that aren't covered by FunctionDef
    let dd_ids: std::collections::HashSet<&str> = types.iter().map(|t| t.block_type).collect();
    for r in legacy_registrations() {
        if !dd_ids.contains(r.block_type) {
            types.push(BlockTypeInfo {
                block_type: r.block_type,
                name: r.display_name,
                category: r.category,
            });
        }
    }

    types
}

/// Leak a string to get a `&'static str` — only called at palette-build time
/// (a small, bounded set of strings), not on every tick.
fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[derive(Debug, serde::Serialize, tsify_next::Tsify)]
#[tsify(into_wasm_abi)]
pub struct BlockTypeInfo {
    pub block_type: &'static str,
    pub name: &'static str,
    pub category: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::block::Module;

    #[test]
    fn create_block_constant() {
        let block = create_block("constant", r#"{"value": 42.0}"#).unwrap();
        assert_eq!(block.block_type(), "constant");
    }

    #[test]
    fn create_block_add() {
        let block = create_block("add", "{}").unwrap();
        assert_eq!(block.block_type(), "add");
    }

    #[test]
    fn create_block_multiply() {
        let block = create_block("multiply", "{}").unwrap();
        assert_eq!(block.block_type(), "multiply");
    }

    #[test]
    fn create_block_unknown() {
        let result = create_block("nonexistent", "{}");
        assert!(result.is_err());
    }

    #[test]
    fn create_block_pubsub() {
        let block = create_block("pubsub_sink", r#"{"topic":"t","port_kind":"Float"}"#).unwrap();
        assert_eq!(block.block_type(), "pubsub_sink");
        let block = create_block("pubsub_source", r#"{"topic":"t","port_kind":"Float"}"#).unwrap();
        assert_eq!(block.block_type(), "pubsub_source");
    }

    #[test]
    fn create_block_state_machine() {
        let cfg = r#"{"states":["a"],"initial":"a","transitions":[]}"#;
        let block = create_block("state_machine", cfg).unwrap();
        assert_eq!(block.block_type(), "state_machine");
    }

    #[test]
    fn create_block_state_machine_minimal() {
        let block = create_block("state_machine", r#"{"states":["idle"],"initial":"idle"}"#).unwrap();
        assert_eq!(block.block_type(), "state_machine");
    }

    #[test]
    fn create_block_state_machine_with_topics() {
        let cfg = r#"{
            "states": ["idle", "running"],
            "initial": "idle",
            "transitions": [{
                "from": "idle",
                "to": "running",
                "guard": {"type": "Topic", "topic": "cmd", "condition": {"field": "go", "op": "Gt", "value": 0.0}},
                "actions": []
            }],
            "input_topics": [{"topic": "cmd", "schema": {"name": "cmd", "fields": [{"name": "go", "field_type": "F32"}]}}],
            "output_topics": []
        }"#;
        let block = create_block("state_machine", cfg).unwrap();
        assert_eq!(block.block_type(), "state_machine");
        assert_eq!(block.input_ports().len(), 1);
        assert_eq!(block.input_ports()[0].name, "cmd");
    }

    #[test]
    fn create_block_state_machine_legacy_guard() {
        let cfg = r#"{
            "states": ["a", "b"],
            "initial": "a",
            "transitions": [{"from": "a", "to": "b", "guard": {"type": "GuardPort", "port": 0}}]
        }"#;
        let block = create_block("state_machine", cfg).unwrap();
        assert_eq!(block.input_ports().len(), 1);
        assert_eq!(block.input_ports()[0].name, "guard_0");
    }

    #[test]
    fn create_block_gain() {
        let block = create_block("gain", r#"{"op":"Gain","param1":2.0}"#).unwrap();
        assert_eq!(block.block_type(), "gain");
    }

    #[test]
    fn create_block_clamp() {
        let block = create_block("clamp", r#"{"op":"Clamp","param1":0.0,"param2":1.0}"#).unwrap();
        assert_eq!(block.block_type(), "clamp");
    }

    #[test]
    fn create_block_plot() {
        let block = create_block("plot", r#"{}"#).unwrap();
        assert_eq!(block.block_type(), "plot");
    }

    #[test]
    fn create_block_json_encode() {
        let block = create_block("json_encode", "{}").unwrap();
        assert_eq!(block.block_type(), "json_encode");
    }

    #[test]
    fn create_block_json_decode() {
        let block = create_block("json_decode", "{}").unwrap();
        assert_eq!(block.block_type(), "json_decode");
    }

    #[test]
    fn create_block_udp_source() {
        let block = create_block("udp_source", r#"{"address":"127.0.0.1:5000"}"#).unwrap();
        assert_eq!(block.block_type(), "udp_source");
    }

    #[test]
    fn create_block_udp_sink() {
        let block = create_block("udp_sink", r#"{"address":"127.0.0.1:5001"}"#).unwrap();
        assert_eq!(block.block_type(), "udp_sink");
    }

    #[test]
    fn create_block_adc_source() {
        let block = create_block("adc_source", r#"{"channel":0,"resolution_bits":12}"#).unwrap();
        assert_eq!(block.block_type(), "adc_source");
    }

    #[test]
    fn create_block_pwm_sink() {
        let block = create_block("pwm_sink", r#"{"channel":0,"frequency_hz":1000}"#).unwrap();
        assert_eq!(block.block_type(), "pwm_sink");
    }

    #[test]
    fn create_block_gpio_out() {
        let block = create_block("gpio_out", r#"{"pin":13}"#).unwrap();
        assert_eq!(block.block_type(), "gpio_out");
    }

    #[test]
    fn create_block_gpio_in() {
        let block = create_block("gpio_in", r#"{"pin":2}"#).unwrap();
        assert_eq!(block.block_type(), "gpio_in");
    }

    #[test]
    fn create_block_uart_tx() {
        let block = create_block("uart_tx", r#"{"port":0,"baud":115200}"#).unwrap();
        assert_eq!(block.block_type(), "uart_tx");
    }

    #[test]
    fn create_block_uart_rx() {
        let block = create_block("uart_rx", r#"{"port":0,"baud":115200}"#).unwrap();
        assert_eq!(block.block_type(), "uart_rx");
    }

    #[test]
    fn create_block_encoder() {
        let block = create_block("encoder", r#"{"channel":0}"#).unwrap();
        assert_eq!(block.block_type(), "encoder");
    }

    #[test]
    fn create_block_ssd1306_display() {
        let block = create_block("ssd1306_display", r#"{"i2c_bus":0,"address":60}"#).unwrap();
        assert_eq!(block.block_type(), "ssd1306_display");
    }

    #[test]
    fn create_block_tmc2209_stepper() {
        let block = create_block(
            "tmc2209_stepper",
            r#"{"uart_port":0,"uart_addr":0,"steps_per_rev":200,"microsteps":16}"#,
        )
        .unwrap();
        assert_eq!(block.block_type(), "tmc2209_stepper");
    }

    #[test]
    fn create_block_tmc2209_stallguard() {
        let block = create_block(
            "tmc2209_stallguard",
            r#"{"uart_port":0,"uart_addr":0,"threshold":50}"#,
        )
        .unwrap();
        assert_eq!(block.block_type(), "tmc2209_stallguard");
    }

    #[test]
    fn create_block_invalid_json_errors() {
        // Exercise all map_err closures by passing invalid JSON to each block type
        let bad = "not json";
        for bt in &[
            "constant",
            "gain",
            "clamp",
            "plot",
            "udp_source",
            "udp_sink",
            "adc_source",
            "pwm_sink",
            "gpio_out",
            "gpio_in",
            "uart_tx",
            "uart_rx",
            "state_machine",
            "pubsub_sink",
            "pubsub_source",
            "encoder",
            "ssd1306_display",
            "tmc2209_stepper",
            "tmc2209_stallguard",
        ] {
            assert!(create_block(bt, bad).is_err(), "expected error for {bt}");
        }
    }

    #[test]
    fn constant_block_default_trait_methods() {
        let b = constant::ConstantBlock::new(1.0);
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
    }

    #[test]
    fn constant_block_as_sim_model_none() {
        let mut b = constant::ConstantBlock::new(1.0);
        assert!(b.as_sim_model().is_none());
    }

}
