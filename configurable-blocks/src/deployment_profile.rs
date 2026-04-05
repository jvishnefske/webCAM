//! Deployment profile types for configurable block sets.
//!
//! A single block set may have multiple [`DeploymentProfile`]s — each profile
//! remaps channels differently, assigns blocks to nodes, and binds hardware
//! peripherals. This enables the same logical control flow to be deployed to
//! different hardware configurations without modifying the block definitions.

use std::collections::HashMap;

use module_traits::deployment::PeripheralBinding;
use serde::{Deserialize, Serialize};

/// Maps logical channel names to deployment-specific channel names.
///
/// When a block declares a channel named `"motor/setpoint"`, a deployment
/// profile can remap it to `"robot_arm/joint1/setpoint"` for a specific
/// deployment target. If no mapping exists, the original name is returned.
///
/// # Example
///
/// ```rust
/// use configurable_blocks::deployment_profile::ChannelMap;
///
/// let mut map = ChannelMap::new();
/// map.insert("motor/setpoint".into(), "robot/joint1/setpoint".into());
/// assert_eq!(map.remap("motor/setpoint"), "robot/joint1/setpoint");
/// assert_eq!(map.remap("unknown"), "unknown");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChannelMap(HashMap<String, String>);

impl ChannelMap {
    /// Create an empty channel map.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a logical → deployment-specific channel name mapping.
    pub fn insert(&mut self, logical: String, deployment: String) {
        self.0.insert(logical, deployment);
    }

    /// Return the remapped name for `name`, or `name` itself if no mapping exists.
    pub fn remap<'a>(&'a self, name: &'a str) -> &'a str {
        self.0.get(name).map(String::as_str).unwrap_or(name)
    }

    /// Returns `true` if no mappings have been added.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(String, String)> for ChannelMap {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Describes one complete deployment of a configurable block set.
///
/// A deployment profile captures everything needed to take an abstract set of
/// configurable blocks and place them onto specific hardware:
///
/// - **channel_map** — remaps logical channel names to deployment-specific ones
/// - **node_assignments** — maps block IDs to the node (MCU/board) they run on
/// - **peripheral_assignments** — binds block ports to hardware peripherals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentProfile {
    /// Human-readable name for this deployment (e.g. "robot_arm_v1", "bench_test").
    pub name: String,
    /// Logical → deployment-specific channel name remappings.
    pub channel_map: ChannelMap,
    /// Maps block_id → node_id (which MCU/board runs this block).
    pub node_assignments: HashMap<u32, String>,
    /// Hardware peripheral bindings (block port → MCU peripheral + pins).
    pub peripheral_assignments: Vec<PeripheralBinding>,
}

impl DeploymentProfile {
    /// Create a new deployment profile with the given name and empty defaults.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_map: ChannelMap::new(),
            node_assignments: HashMap::new(),
            peripheral_assignments: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChannelMap tests ────────────────────────────────────────────────────

    #[test]
    fn channel_map_new_is_empty() {
        let map = ChannelMap::new();
        // remap on an empty map returns the original name
        assert_eq!(map.remap("motor/setpoint"), "motor/setpoint");
    }

    #[test]
    fn channel_map_insert_and_remap() {
        let mut map = ChannelMap::new();
        map.insert("motor/setpoint".into(), "robot/joint1/setpoint".into());
        assert_eq!(map.remap("motor/setpoint"), "robot/joint1/setpoint");
    }

    #[test]
    fn channel_map_remap_returns_original_when_missing() {
        let mut map = ChannelMap::new();
        map.insert("a".into(), "b".into());
        // "c" has no mapping → original is returned
        assert_eq!(map.remap("c"), "c");
    }

    #[test]
    fn channel_map_from_iter() {
        let map: ChannelMap = vec![
            ("foo".to_string(), "bar".to_string()),
            ("baz".to_string(), "qux".to_string()),
        ]
        .into_iter()
        .collect();

        assert_eq!(map.remap("foo"), "bar");
        assert_eq!(map.remap("baz"), "qux");
        assert_eq!(map.remap("unknown"), "unknown");
    }

    #[test]
    fn channel_map_serde_roundtrip() {
        let mut map = ChannelMap::new();
        map.insert("in".into(), "deployment_in".into());

        let json = serde_json::to_string(&map).expect("serialize");
        let decoded: ChannelMap = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(map, decoded);
        assert_eq!(decoded.remap("in"), "deployment_in");
    }

    // ── DeploymentProfile tests ─────────────────────────────────────────────

    #[test]
    fn deployment_profile_new_defaults() {
        let profile = DeploymentProfile::new("my_deployment");
        assert_eq!(profile.name, "my_deployment");
        assert!(profile.channel_map.is_empty());
        assert!(profile.node_assignments.is_empty());
        assert!(profile.peripheral_assignments.is_empty());
    }

    #[test]
    fn deployment_profile_serde_roundtrip() {
        let mut profile = DeploymentProfile::new("test_profile");
        profile
            .channel_map
            .insert("sensor/temp".into(), "node0/temp".into());
        profile
            .node_assignments
            .insert(1, "mcu_0".to_string());
        profile
            .node_assignments
            .insert(2, "mcu_1".to_string());

        let json = serde_json::to_string(&profile).expect("serialize");
        let decoded: DeploymentProfile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.name, "test_profile");
        assert_eq!(decoded.channel_map.remap("sensor/temp"), "node0/temp");
        assert_eq!(decoded.node_assignments.get(&1).map(String::as_str), Some("mcu_0"));
        assert_eq!(decoded.node_assignments.get(&2).map(String::as_str), Some("mcu_1"));
        assert!(decoded.peripheral_assignments.is_empty());
    }

    #[test]
    fn deployment_profile_channel_map_remap_after_roundtrip() {
        let mut profile = DeploymentProfile::new("roundtrip_test");
        profile.channel_map.insert("x".into(), "y".into());

        let json = serde_json::to_string(&profile).expect("serialize");
        let decoded: DeploymentProfile = serde_json::from_str(&json).expect("deserialize");

        // Mapped name returns mapped value
        assert_eq!(decoded.channel_map.remap("x"), "y");
        // Unmapped name returns itself
        assert_eq!(decoded.channel_map.remap("z"), "z");
    }
}
