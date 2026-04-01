use alloc::collections::BTreeMap;
use alloc::string::String;

use dag_core::eval::{PubSubReader, PubSubWriter};

/// Simple in-memory pub/sub store backed by a BTreeMap.
pub struct SimplePubSub {
    topics: BTreeMap<String, f64>,
}

impl SimplePubSub {
    /// Create an empty pub/sub store.
    pub fn new() -> Self {
        SimplePubSub {
            topics: BTreeMap::new(),
        }
    }

    /// Set a topic value.
    pub fn set(&mut self, topic: &str, value: f64) {
        self.topics.insert(topic.into(), value);
    }

    /// Get a topic value, returning 0.0 if not set.
    pub fn get(&self, topic: &str) -> f64 {
        self.topics.get(topic).copied().unwrap_or(0.0)
    }
}

impl Default for SimplePubSub {
    fn default() -> Self {
        Self::new()
    }
}

impl PubSubReader for SimplePubSub {
    fn read(&self, topic: &str) -> f64 {
        self.get(topic)
    }
}

impl PubSubWriter for SimplePubSub {
    fn write(&mut self, topic: &str, value: f64) {
        self.set(topic, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_pubsub_set_get() {
        let mut ps = SimplePubSub::new();
        ps.set("sensor/temp", 25.0);
        assert_eq!(ps.get("sensor/temp"), 25.0);
        ps.set("sensor/temp", 30.0);
        assert_eq!(ps.get("sensor/temp"), 30.0);
    }

    #[test]
    fn test_simple_pubsub_default() {
        let ps = SimplePubSub::new();
        assert_eq!(ps.get("nonexistent"), 0.0);
    }

    #[test]
    fn test_simple_pubsub_default_trait() {
        let ps = SimplePubSub::default();
        assert_eq!(ps.get("x"), 0.0);
    }

    #[test]
    fn test_simple_pubsub_reader_writer() {
        let mut ps = SimplePubSub::new();
        let writer: &mut dyn PubSubWriter = &mut ps;
        writer.write("topic/a", 99.0);

        let reader: &dyn PubSubReader = &ps;
        assert_eq!(reader.read("topic/a"), 99.0);
        assert_eq!(reader.read("unknown"), 0.0);
    }
}
