//! Plot block: accumulates time-series data for frontend visualization.

use crate::dataflow::block::{Module, PortDef, PortKind, Tick, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PlotConfig {
    /// Maximum number of samples to keep.
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,
}

fn default_max_samples() -> usize {
    1000
}

pub struct PlotBlock {
    max_samples: usize,
    buffer: Vec<f64>,
}

impl PlotBlock {
    pub fn new(max_samples: usize) -> Self {
        Self {
            max_samples,
            buffer: Vec::new(),
        }
    }

    pub fn from_config(cfg: PlotConfig) -> Self {
        Self::new(cfg.max_samples)
    }
}

impl Module for PlotBlock {
    fn name(&self) -> &str {
        "Plot"
    }

    fn block_type(&self) -> &str {
        "plot"
    }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("in", PortKind::Float)]
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef::new("series", PortKind::Series)]
    }

    fn config_json(&self) -> String {
        serde_json::to_string(&PlotConfig {
            max_samples: self.max_samples,
        })
        .unwrap_or_default()
    }

    fn as_tick(&mut self) -> Option<&mut dyn Tick> {
        Some(self)
    }
}

impl Tick for PlotBlock {
    fn tick(&mut self, inputs: &[Option<&Value>], _dt: f64) -> Vec<Option<Value>> {
        if let Some(v) = inputs.first().and_then(|i| i.and_then(|v| v.as_float())) {
            self.buffer.push(v);
            if self.buffer.len() > self.max_samples {
                self.buffer.remove(0);
            }
        }
        vec![Some(Value::Series(self.buffer.clone()))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_samples() {
        let mut p = PlotBlock::new(5);
        for i in 0..7 {
            let v = Value::Float(i as f64);
            p.tick(&[Some(&v)], 0.01);
        }
        let out = p.tick(&[Some(&Value::Float(7.0))], 0.01);
        let series = out[0].as_ref().unwrap().as_series().unwrap();
        assert_eq!(series.len(), 5);
        assert_eq!(series, &[3.0, 4.0, 5.0, 6.0, 7.0]);
    }

    #[test]
    fn plot_module_trait() {
        let mut b = PlotBlock::new(100);
        assert_eq!(b.name(), "Plot");
        assert_eq!(b.block_type(), "plot");
        assert_eq!(b.input_ports().len(), 1);
        assert_eq!(b.output_ports().len(), 1);
        assert!(b.config_json().contains("max_samples"));
        assert!(b.as_analysis().is_none());
        assert!(b.as_codegen().is_none());
        assert!(b.as_sim_model().is_none());
        assert!(b.as_tick().is_some());
    }

    #[test]
    fn from_config_and_default_max_samples() {
        let cfg: PlotConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.max_samples, 1000);
        let b = PlotBlock::from_config(cfg);
        assert_eq!(b.max_samples, 1000);
    }
}
