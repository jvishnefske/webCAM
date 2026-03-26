//! Built-in block implementations.

pub mod constant;
pub mod embedded;
pub mod function;
pub mod plot;
pub mod pubsub;
pub mod serde_block;
pub mod state_machine;
pub mod udp;

use super::block::Module;

/// Create a block from its type name and JSON config.
pub fn create_block(block_type: &str, config_json: &str) -> Result<Box<dyn Module>, String> {
    match block_type {
        "constant" => {
            let cfg: constant::ConstantConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(constant::ConstantBlock::from_config(cfg)))
        }
        "gain" => {
            let cfg: function::FunctionConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(function::FunctionBlock::from_config(cfg)))
        }
        "add" => Ok(Box::new(function::FunctionBlock::add())),
        "multiply" => Ok(Box::new(function::FunctionBlock::multiply())),
        "clamp" => {
            let cfg: function::FunctionConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(function::FunctionBlock::from_config(cfg)))
        }
        "plot" => {
            let cfg: plot::PlotConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(plot::PlotBlock::from_config(cfg)))
        }
        "json_encode" => Ok(Box::new(serde_block::JsonEncodeBlock::new())),
        "json_decode" => Ok(Box::new(serde_block::JsonDecodeBlock::new())),
        "udp_source" => {
            let cfg: udp::UdpConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(udp::UdpSourceBlock::new(&cfg.address)))
        }
        "udp_sink" => {
            let cfg: udp::UdpConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(udp::UdpSinkBlock::new(&cfg.address)))
        }
        "adc_source" => {
            let cfg: embedded::AdcConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::AdcBlock::from_config(cfg)))
        }
        "pwm_sink" => {
            let cfg: embedded::PwmConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::PwmBlock::from_config(cfg)))
        }
        "gpio_out" => {
            let cfg: embedded::GpioOutConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::GpioOutBlock::from_config(cfg)))
        }
        "gpio_in" => {
            let cfg: embedded::GpioInConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::GpioInBlock::from_config(cfg)))
        }
        "uart_tx" => {
            let cfg: embedded::UartTxConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::UartTxBlock::from_config(cfg)))
        }
        "uart_rx" => {
            let cfg: embedded::UartRxConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::UartRxBlock::from_config(cfg)))
        }
        "state_machine" => {
            let cfg: state_machine::StateMachineConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(state_machine::StateMachineBlock::from_config(cfg)))
        }
        "pubsub_sink" => {
            let cfg: pubsub::PubSubConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(pubsub::PubSubSinkBlock::from_config(cfg)))
        }
        "pubsub_source" => {
            let cfg: pubsub::PubSubConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(pubsub::PubSubSourceBlock::from_config(cfg)))
        }
        "encoder" => {
            let cfg: embedded::EncoderConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::EncoderBlock::from_config(cfg)))
        }
        "ssd1306_display" => {
            let cfg: embedded::Ssd1306DisplayConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::Ssd1306DisplayBlock::from_config(cfg)))
        }
        "tmc2209_stepper" => {
            let cfg: embedded::Tmc2209StepperConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::Tmc2209StepperBlock::from_config(cfg)))
        }
        "tmc2209_stallguard" => {
            let cfg: embedded::Tmc2209StallGuardConfig =
                serde_json::from_str(config_json).map_err(|e| e.to_string())?;
            Ok(Box::new(embedded::Tmc2209StallGuardBlock::from_config(cfg)))
        }
        _ => Err(format!("unknown block type: {block_type}")),
    }
}

/// List all available block types for the palette.
pub fn available_block_types() -> Vec<BlockTypeInfo> {
    vec![
        BlockTypeInfo {
            block_type: "constant",
            name: "Constant",
            category: "Sources",
        },
        BlockTypeInfo {
            block_type: "gain",
            name: "Gain",
            category: "Math",
        },
        BlockTypeInfo {
            block_type: "add",
            name: "Add",
            category: "Math",
        },
        BlockTypeInfo {
            block_type: "multiply",
            name: "Multiply",
            category: "Math",
        },
        BlockTypeInfo {
            block_type: "clamp",
            name: "Clamp",
            category: "Math",
        },
        BlockTypeInfo {
            block_type: "plot",
            name: "Plot",
            category: "Sinks",
        },
        BlockTypeInfo {
            block_type: "json_encode",
            name: "JSON Encode",
            category: "Serde",
        },
        BlockTypeInfo {
            block_type: "json_decode",
            name: "JSON Decode",
            category: "Serde",
        },
        BlockTypeInfo {
            block_type: "udp_source",
            name: "UDP Source",
            category: "I/O",
        },
        BlockTypeInfo {
            block_type: "udp_sink",
            name: "UDP Sink",
            category: "I/O",
        },
        BlockTypeInfo {
            block_type: "adc_source",
            name: "ADC Source",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "pwm_sink",
            name: "PWM Sink",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "gpio_out",
            name: "GPIO Out",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "gpio_in",
            name: "GPIO In",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "uart_tx",
            name: "UART TX",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "uart_rx",
            name: "UART RX",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "state_machine",
            name: "State Machine",
            category: "Logic",
        },
        BlockTypeInfo {
            block_type: "pubsub_source",
            name: "PubSub Source",
            category: "I/O",
        },
        BlockTypeInfo {
            block_type: "pubsub_sink",
            name: "PubSub Sink",
            category: "I/O",
        },
        BlockTypeInfo {
            block_type: "encoder",
            name: "Encoder",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "ssd1306_display",
            name: "SSD1306 Display",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "tmc2209_stepper",
            name: "TMC2209 Stepper",
            category: "Embedded",
        },
        BlockTypeInfo {
            block_type: "tmc2209_stallguard",
            name: "TMC2209 StallGuard",
            category: "Embedded",
        },
    ]
}

#[derive(Debug, serde::Serialize)]
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
