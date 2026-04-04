//! UDP blocks: source (recv) and sink (send).
//!
//! In WASM these are stubbed — real UDP requires native execution.
//! The blocks still participate in the graph so the topology can be
//! designed in the browser and later run natively.

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
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

impl Module for UdpSourceBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&UdpConfig {
            address: self.address.clone(),
        })
        .unwrap_or_default()
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for UdpSourceBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual UDP in WASM.
        vec![None]
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

impl Module for UdpSinkBlock {
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
    fn config_json(&self) -> String {
        serde_json::to_string(&UdpConfig {
            address: self.address.clone(),
        })
        .unwrap_or_default()
    }
    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for UdpSinkBlock {
    fn tick(&mut self, _inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        // Stub: no actual UDP in WASM.
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_source_module_trait() {
        let mut b = UdpSourceBlock::new("127.0.0.1:9000");
        assert_eq!(b.name(), "UDP Source");
        assert_eq!(b.block_type(), "udp_source");
        assert!(b.input_ports().is_empty());
        assert_eq!(b.output_ports().len(), 1);
        assert!(b.config_json().contains("127.0.0.1:9000"));
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
        assert!(b.as_sim_model().is_none());
        assert!(b.as_tick().is_some());
    }

    #[test]
    fn udp_source_tick() {
        let mut b = UdpSourceBlock::new("127.0.0.1:9000");
        let out = b.tick(&[], 0.01);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_none());
    }

    #[test]
    fn udp_sink_module_trait() {
        let mut b = UdpSinkBlock::new("127.0.0.1:9001");
        assert_eq!(b.name(), "UDP Sink");
        assert_eq!(b.block_type(), "udp_sink");
        assert_eq!(b.input_ports().len(), 1);
        assert!(b.output_ports().is_empty());
        assert!(b.config_json().contains("127.0.0.1:9001"));
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
        assert!(b.as_sim_model().is_none());
        assert!(b.as_tick().is_some());
    }

    #[test]
    fn udp_sink_tick() {
        let mut b = UdpSinkBlock::new("127.0.0.1:9001");
        let out = b.tick(&[], 0.01);
        assert!(out.is_empty());
    }
}
