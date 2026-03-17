//! UDP blocks: source (recv) and sink (send).
//!
//! In WASM these are stubbed — real UDP requires native execution.
//! The blocks still participate in the graph so the topology can be
//! designed in the browser and later run natively.

use crate::dataflow::block::{Block, PortDef, PortKind, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UdpConfig {
    pub address: String,
}

/// Receives UDP datagrams and emits them as Bytes.
/// Stubbed in WASM — always outputs None.
pub struct UdpSourceBlock {
    address: String,
}

impl UdpSourceBlock {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
        }
    }
}

impl Block for UdpSourceBlock {
    fn name(&self) -> &str {
        "UDP Source"
    }
    fn block_type(&self) -> &str {
        "udp_source"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("data", PortKind::Bytes)]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual UDP in WASM.
        vec![None]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&UdpConfig {
            address: self.address.clone(),
        })
        .unwrap_or_default()
    }
}

/// Sends input Bytes as UDP datagrams.
/// Stubbed in WASM.
pub struct UdpSinkBlock {
    address: String,
}

impl UdpSinkBlock {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
        }
    }
}

impl Block for UdpSinkBlock {
    fn name(&self) -> &str {
        "UDP Sink"
    }
    fn block_type(&self) -> &str {
        "udp_sink"
    }
    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("data", PortKind::Bytes)]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![]
    }
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual UDP in WASM.
        vec![]
    }
    fn config_json(&self) -> String {
        serde_json::to_string(&UdpConfig {
            address: self.address.clone(),
        })
        .unwrap_or_default()
    }
}
