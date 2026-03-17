//! Built-in block implementations.

pub mod constant;
pub mod function;
pub mod plot;
pub mod serde_block;
pub mod udp;

use super::block::Block;

/// Create a block from its type name and JSON config.
pub fn create_block(block_type: &str, config_json: &str) -> Result<Box<dyn Block>, String> {
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
    ]
}

#[derive(Debug, serde::Serialize)]
pub struct BlockTypeInfo {
    pub block_type: &'static str,
    pub name: &'static str,
    pub category: &'static str,
}
