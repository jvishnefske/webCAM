//! Deployment profile types for configurable block sets.
//!
//! A single block set may have multiple [`DeploymentProfile`]s — each profile
//! remaps channels differently, assigns blocks to nodes, and binds hardware
//! peripherals. This enables the same logical control flow to be deployed to
//! different hardware configurations without modifying the block definitions.

use std::collections::HashMap;
use std::fmt;

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

// ── Validation ───────────────────────────────────────────────────────────────

/// Errors that can be detected when validating a [`DeploymentProfile`] against
/// a set of configurable blocks.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// A `Hardware`-kind declared channel has no corresponding [`PeripheralBinding`].
    MissingPeripheralBinding {
        channel_name: String,
        block_type: String,
    },
    /// A `node_assignments` entry references an MCU family not found in the
    /// inventory (i.e. [`module_traits::inventory::mcu_for`] returned `None`).
    UnknownMcu { family: String },
    /// Two [`PeripheralBinding`]s on the same node share the same physical pin.
    PinConflict {
        pin: String,
        node: String,
        binding_a: String,
        binding_b: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::MissingPeripheralBinding {
                channel_name,
                block_type,
            } => write!(
                f,
                "block '{}' declares hardware channel '{}' but no PeripheralBinding was found",
                block_type, channel_name
            ),
            ValidationError::UnknownMcu { family } => {
                write!(f, "unknown MCU family '{}' in node_assignments", family)
            }
            ValidationError::PinConflict {
                pin,
                node,
                binding_a,
                binding_b,
            } => write!(
                f,
                "pin conflict on node '{}': pin '{}' used by both '{}' and '{}'",
                node, pin, binding_a, binding_b
            ),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a [`DeploymentProfile`] against a set of `(block_type, config_json)` pairs.
///
/// Checks performed:
/// 1. **MissingPeripheralBinding** — every `Hardware`-kind declared channel must
///    have a matching [`PeripheralBinding`] in `profile.peripheral_assignments`.
///    Matching is by `block_id` (the index of the block in `blocks`) and
///    `port_name` (the channel name).
/// 2. **UnknownMcu** — every node referenced in `profile.node_assignments` must
///    be a recognised MCU family (`module_traits::inventory::mcu_for` returns
///    `Some`).
/// 3. **PinConflict** — within a single node, no two [`PeripheralBinding`]s may
///    share the same physical pin string.
///
/// Returns `Ok(())` if all checks pass, or `Err(errors)` with every error that
/// was found (all checks are always run).
pub fn validate_profile(
    profile: &DeploymentProfile,
    blocks: &[(String, serde_json::Value)],
) -> Result<(), Vec<ValidationError>> {
    let mut errors: Vec<ValidationError> = Vec::new();

    // ── 1. MissingPeripheralBinding ──────────────────────────────────────────
    for (block_idx, (block_type, config)) in blocks.iter().enumerate() {
        let block_id = block_idx as u32;

        let mut block_instance = match crate::registry::create_block(block_type) {
            Some(b) => b,
            None => continue, // unknown block type — not our job to report here
        };
        block_instance.apply_config(config);

        for channel in block_instance.declared_channels() {
            if channel.kind != crate::schema::ChannelKind::Hardware {
                continue;
            }

            let has_binding = profile.peripheral_assignments.iter().any(|pb| {
                pb.block_id == block_id && pb.port_name == channel.name
            });

            if !has_binding {
                errors.push(ValidationError::MissingPeripheralBinding {
                    channel_name: channel.name.clone(),
                    block_type: block_type.clone(),
                });
            }
        }
    }

    // ── 2. UnknownMcu ────────────────────────────────────────────────────────
    let unique_nodes: std::collections::HashSet<&String> =
        profile.node_assignments.values().collect();

    for node in &unique_nodes {
        if module_traits::inventory::mcu_for(node).is_none() {
            errors.push(ValidationError::UnknownMcu {
                family: node.to_string(),
            });
        }
    }

    // ── 3. PinConflict ───────────────────────────────────────────────────────
    // Group peripheral_assignments by node.
    let mut bindings_by_node: HashMap<&str, Vec<&PeripheralBinding>> = HashMap::new();
    for pb in &profile.peripheral_assignments {
        bindings_by_node.entry(pb.node.as_str()).or_default().push(pb);
    }

    for (node, bindings) in &bindings_by_node {
        // Collect (pin, binding_label) pairs from all bindings on this node.
        let mut pin_owners: Vec<(String, String)> = Vec::new();

        for pb in bindings {
            let label = format!("{}/{}", pb.block_id, pb.port_name);
            for pin_binding in &pb.pins {
                pin_owners.push((pin_binding.pin.clone(), label.clone()));
            }
        }

        // O(n²) conflict check — binding lists are small in practice.
        for i in 0..pin_owners.len() {
            for j in (i + 1)..pin_owners.len() {
                if pin_owners[i].0 == pin_owners[j].0 {
                    errors.push(ValidationError::PinConflict {
                        pin: pin_owners[i].0.clone(),
                        node: node.to_string(),
                        binding_a: pin_owners[i].1.clone(),
                        binding_b: pin_owners[j].1.clone(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
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

    // ── validate_profile tests ──────────────────────────────────────────────

    use module_traits::deployment::{PeripheralBinding, PeripheralConfig, PinBinding};

    /// Build a minimal PeripheralBinding for test use.
    fn make_binding(block_id: u32, port_name: &str, node: &str, pins: Vec<&str>) -> PeripheralBinding {
        PeripheralBinding {
            block_id,
            port_name: port_name.into(),
            node: node.into(),
            peripheral: "ADC1".into(),
            pins: pins.into_iter().map(|p| PinBinding { signal: "IN".into(), pin: p.into(), af: None }).collect(),
            dma: None,
            config: PeripheralConfig::Adc { channel: 0, resolution_bits: 12, sample_time: 0 },
        }
    }

    /// Valid profile with only PubSub blocks → no errors.
    #[test]
    fn validate_profile_no_hardware_channels_ok() {
        let profile = DeploymentProfile::new("test");
        // PID block only uses PubSub channels.
        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("pid".into(), serde_json::json!({})),
        ];
        assert!(validate_profile(&profile, &blocks).is_ok());
    }

    /// ADC block (block_id=0) declares a hardware channel; no binding → error.
    #[test]
    fn validate_profile_missing_hardware_binding_error() {
        let profile = DeploymentProfile::new("test");
        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
        ];
        let errors = validate_profile(&profile, &blocks).unwrap_err();
        assert!(
            errors.iter().any(|e| matches!(
                e,
                ValidationError::MissingPeripheralBinding { channel_name, block_type }
                    if channel_name == "adc0" && block_type == "adc"
            )),
            "expected MissingPeripheralBinding, got: {:?}",
            errors
        );
    }

    /// node_assignments references an MCU family not in inventory → UnknownMcu.
    #[test]
    fn validate_profile_unknown_mcu_error() {
        let mut profile = DeploymentProfile::new("test");
        profile.node_assignments.insert(0, "UnknownChip9000".into());
        let blocks: Vec<(String, serde_json::Value)> = vec![];
        let errors = validate_profile(&profile, &blocks).unwrap_err();
        assert!(
            errors.iter().any(|e| matches!(
                e,
                ValidationError::UnknownMcu { family } if family == "UnknownChip9000"
            )),
            "expected UnknownMcu, got: {:?}",
            errors
        );
    }

    /// Two bindings on the same node share pin "PA0" → PinConflict.
    #[test]
    fn validate_profile_pin_conflict_error() {
        let mut profile = DeploymentProfile::new("test");
        // Two ADC blocks (block 0 and block 1), both assigned pin "PA0" on node "Rp2040".
        profile.node_assignments.insert(0, "Rp2040".into());
        profile.node_assignments.insert(1, "Rp2040".into());

        let binding_a = make_binding(0, "adc0", "Rp2040", vec!["PA0"]);
        let binding_b = make_binding(1, "adc1", "Rp2040", vec!["PA0"]);
        profile.peripheral_assignments.push(binding_a);
        profile.peripheral_assignments.push(binding_b);

        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
            ("adc".into(), serde_json::json!({"channel_name": "adc1"})),
        ];

        let errors = validate_profile(&profile, &blocks).unwrap_err();
        assert!(
            errors.iter().any(|e| matches!(
                e,
                ValidationError::PinConflict { pin, node, .. }
                    if pin == "PA0" && node == "Rp2040"
            )),
            "expected PinConflict, got: {:?}",
            errors
        );
    }

    /// Valid profile: ADC block with a correct binding and a known MCU → Ok.
    #[test]
    fn validate_profile_valid_with_correct_bindings_ok() {
        let mut profile = DeploymentProfile::new("test");
        profile.node_assignments.insert(0, "Rp2040".into());

        // Provide a binding for block_id=0, port_name="adc0" (matches the adc block's declared channel).
        let binding = make_binding(0, "adc0", "Rp2040", vec!["GP26"]);
        profile.peripheral_assignments.push(binding);

        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
        ];

        assert!(
            validate_profile(&profile, &blocks).is_ok(),
            "expected Ok for valid profile"
        );
    }
}
