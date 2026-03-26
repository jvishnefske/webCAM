use alloc::collections::BTreeMap;
use alloc::string::String;

use dag_core::eval::{ChannelReader, ChannelWriter};

/// Simple channel store backed by a BTreeMap.
///
/// On a real MCU, this would be replaced with HAL peripheral access.
pub struct MapChannels {
    values: BTreeMap<String, f64>,
}

impl MapChannels {
    /// Create an empty channel store.
    pub fn new() -> Self {
        MapChannels {
            values: BTreeMap::new(),
        }
    }

    /// Set a channel value.
    pub fn set(&mut self, name: &str, value: f64) {
        self.values.insert(name.into(), value);
    }

    /// Get a channel value, returning 0.0 if not set.
    pub fn get(&self, name: &str) -> f64 {
        self.values.get(name).copied().unwrap_or(0.0)
    }
}

impl Default for MapChannels {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelReader for MapChannels {
    fn read(&self, name: &str) -> f64 {
        self.get(name)
    }
}

impl ChannelWriter for MapChannels {
    fn write(&mut self, name: &str, value: f64) {
        self.set(name, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_channels_set_get() {
        let mut ch = MapChannels::new();
        ch.set("adc0", 3.14);
        assert_eq!(ch.get("adc0"), 3.14);
        ch.set("adc0", 6.28);
        assert_eq!(ch.get("adc0"), 6.28);
    }

    #[test]
    fn test_map_channels_default() {
        let ch = MapChannels::new();
        assert_eq!(ch.get("nonexistent"), 0.0);
    }

    #[test]
    fn test_map_channels_reader_writer() {
        let mut ch = MapChannels::new();
        let writer: &mut dyn ChannelWriter = &mut ch;
        writer.write("pwm0", 42.0);

        let reader: &dyn ChannelReader = &ch;
        assert_eq!(reader.read("pwm0"), 42.0);
        assert_eq!(reader.read("unknown"), 0.0);
    }
}
