//! Deployment profile types for configurable block sets.
//!
//! A single block set may have multiple [`DeploymentProfile`]s — each profile
//! remaps channels differently, assigns blocks to nodes, and binds hardware
//! peripherals. This enables the same logical control flow to be deployed to
//! different hardware configurations without modifying the block definitions.

use std::collections::HashMap;
use std::fmt;

use module_traits::deployment::{
    BoardNode, ChannelBinding, DeploymentManifest, PeripheralBinding, SystemTopology, TaskBinding,
    TaskTrigger,
};
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

/// A board entry in a deployment profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoardEntry {
    /// Unique node identifier (e.g. "motor_ctrl", "sensor_hub").
    pub node_id: String,
    /// MCU family (must match [`module_traits::inventory::mcu_for`]).
    pub mcu_family: String,
}

/// Describes one complete deployment of a configurable block set.
///
/// A deployment profile captures everything needed to take an abstract set of
/// configurable blocks and place them onto specific hardware:
///
/// - **boards** — MCU boards in the deployment (node_id + MCU family)
/// - **channel_map** — remaps logical channel names to deployment-specific ones
/// - **node_assignments** — maps block IDs to the node (MCU/board) they run on
/// - **peripheral_assignments** — binds block ports to hardware peripherals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentProfile {
    /// Human-readable name for this deployment (e.g. "robot_arm_v1", "bench_test").
    pub name: String,
    /// Boards in this deployment. Empty means single-board / legacy mode.
    #[serde(default)]
    pub boards: Vec<BoardEntry>,
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
            boards: Vec::new(),
            channel_map: ChannelMap::new(),
            node_assignments: HashMap::new(),
            peripheral_assignments: Vec::new(),
        }
    }

    /// Add a board to this deployment profile.
    ///
    /// The `node_id` must be unique within the profile and `mcu_family` must
    /// be a recognised family (e.g. "Rp2040", "Stm32f4").
    pub fn add_board(&mut self, node_id: &str, mcu_family: &str) {
        // Avoid duplicates
        if !self.boards.iter().any(|b| b.node_id == node_id) {
            self.boards.push(BoardEntry {
                node_id: node_id.to_string(),
                mcu_family: mcu_family.to_string(),
            });
        }
    }

    /// Assign a block to a specific board node.
    pub fn assign_block(&mut self, block_id: u32, node_id: &str) {
        self.node_assignments
            .insert(block_id, node_id.to_string());
    }

    /// Convert this profile into a [`DeploymentManifest`] suitable for codegen.
    ///
    /// - Creates a [`BoardNode`] for each board entry.
    /// - Creates a single [`TaskBinding`] per node, grouping all assigned blocks.
    /// - Copies `peripheral_assignments` as-is.
    /// - Creates placeholder [`ChannelBinding`] entries as `Simulated`.
    ///
    /// If no boards have been added, falls back to inferring boards from
    /// unique node values in `node_assignments` and `peripheral_assignments`.
    pub fn to_manifest(&self, tick_hz: f64) -> DeploymentManifest {
        // Resolve boards: explicit boards or inferred from assignments.
        let boards: Vec<BoardEntry> = if self.boards.is_empty() {
            // Infer from node_assignments + peripheral_assignments
            let mut seen: HashMap<String, String> = HashMap::new();
            for node_id in self.node_assignments.values() {
                seen.entry(node_id.clone())
                    .or_insert_with(|| node_id.clone());
            }
            for pb in &self.peripheral_assignments {
                seen.entry(pb.node.clone())
                    .or_insert_with(|| pb.node.clone());
            }
            seen.into_keys()
                .map(|node_id| BoardEntry {
                    node_id: node_id.clone(),
                    // When boards are not explicitly configured, default to Host
                    // simulation. Users should call add_board() for real targets.
                    mcu_family: "Host".to_string(),
                })
                .collect()
        } else {
            self.boards.clone()
        };

        let nodes: Vec<BoardNode> = boards
            .iter()
            .map(|b| {
                let mcu = module_traits::inventory::mcu_for(&b.mcu_family);
                BoardNode {
                    id: b.node_id.clone(),
                    mcu_family: b.mcu_family.clone(),
                    board: None,
                    rust_target: mcu.map(|m| {
                        match m.core {
                            module_traits::inventory::CpuCore::CortexM0Plus => {
                                "thumbv6m-none-eabi".to_string()
                            }
                            module_traits::inventory::CpuCore::CortexM4
                            | module_traits::inventory::CpuCore::CortexM4F => {
                                "thumbv7em-none-eabihf".to_string()
                            }
                            module_traits::inventory::CpuCore::CortexM7 => {
                                "thumbv7em-none-eabihf".to_string()
                            }
                            module_traits::inventory::CpuCore::RiscV32IMC => {
                                "riscv32imc-unknown-none-elf".to_string()
                            }
                            module_traits::inventory::CpuCore::HostSim => {
                                "x86_64-unknown-linux-gnu".to_string()
                            }
                        }
                    }),
                }
            })
            .collect();

        // One task per node, grouping all assigned blocks.
        let tasks: Vec<TaskBinding> = boards
            .iter()
            .map(|b| {
                let block_ids: Vec<u32> = self
                    .node_assignments
                    .iter()
                    .filter(|(_, n)| *n == &b.node_id)
                    .map(|(id, _)| *id)
                    .collect();
                TaskBinding {
                    name: format!("{}_task", b.node_id),
                    node: b.node_id.clone(),
                    blocks: block_ids,
                    trigger: TaskTrigger::Periodic { hz: tick_hz },
                    priority: 1,
                    stack_size: None,
                }
            })
            .collect();

        // Placeholder channel bindings (Simulated transport).
        let channels: Vec<ChannelBinding> = Vec::new();

        DeploymentManifest {
            topology: SystemTopology {
                nodes,
                links: Vec::new(),
            },
            tasks,
            channels,
            peripheral_bindings: self.peripheral_assignments.clone(),
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
    /// A block assigned to a board has a hardware channel but no
    /// [`PeripheralBinding`] scoped to that specific board/node.
    MissingBoardBinding {
        block_id: u32,
        channel_name: String,
        block_type: String,
        node_id: String,
    },
    /// A block references a node_id that is not listed in `boards`.
    UnknownBoard {
        block_id: u32,
        node_id: String,
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
            ValidationError::MissingBoardBinding {
                block_id,
                channel_name,
                block_type,
                node_id,
            } => write!(
                f,
                "block {}('{}') has hardware channel '{}' but no PeripheralBinding for node '{}'",
                block_id, block_type, channel_name, node_id
            ),
            ValidationError::UnknownBoard {
                block_id,
                node_id,
            } => write!(
                f,
                "block {} is assigned to node '{}' which is not in the boards list",
                block_id, node_id
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
    // Look up MCU family via profile.boards (node_id → mcu_family), not the
    // raw node_id string which is a logical name like "motor_ctrl".
    for (_, node_id) in &profile.node_assignments {
        if let Some(board) = profile.boards.iter().find(|b| b.node_id == *node_id) {
            if module_traits::inventory::mcu_for(&board.mcu_family).is_none() {
                errors.push(ValidationError::UnknownMcu {
                    family: board.mcu_family.clone(),
                });
            }
        }
        // If no board entry exists, validate_multi_board covers that case.
        // For legacy single-board profiles without boards, skip this check.
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

/// Validate multi-board channel alignment in a [`DeploymentProfile`].
///
/// Checks that:
/// 1. **UnknownBoard** — every block's assigned node must appear in `profile.boards`.
/// 2. **MissingBoardBinding** — every hardware-channel block assigned to a board
///    must have a [`PeripheralBinding`] scoped to that specific node.
/// 3. **UnknownMcu** — every board's `mcu_family` must be a recognised MCU family.
///
/// This extends [`validate_profile`] with board-aware checks. If the profile has
/// no explicit boards, validation passes (legacy single-board mode).
pub fn validate_multi_board(
    profile: &DeploymentProfile,
    blocks: &[(String, serde_json::Value)],
) -> Result<(), Vec<ValidationError>> {
    // If no explicit boards, nothing to check (legacy single-board mode).
    if profile.boards.is_empty() {
        return Ok(());
    }

    let mut errors: Vec<ValidationError> = Vec::new();

    let board_ids: Vec<&str> = profile.boards.iter().map(|b| b.node_id.as_str()).collect();

    // 1. Every board's MCU family must be known.
    for board in &profile.boards {
        if module_traits::inventory::mcu_for(&board.mcu_family).is_none() {
            errors.push(ValidationError::UnknownMcu {
                family: board.mcu_family.clone(),
            });
        }
    }

    // 2. Every assigned block must reference a known board.
    for (block_id, node_id) in &profile.node_assignments {
        if !board_ids.contains(&node_id.as_str()) {
            errors.push(ValidationError::UnknownBoard {
                block_id: *block_id,
                node_id: node_id.clone(),
            });
        }
    }

    // 3. Every hardware-channel block on a board must have a binding for that board.
    for (block_idx, (block_type, config)) in blocks.iter().enumerate() {
        let block_id = block_idx as u32;

        // Only check blocks that are assigned to a board.
        let node_id = match profile.node_assignments.get(&block_id) {
            Some(n) => n,
            None => continue,
        };

        let mut block_instance = match crate::registry::create_block(block_type) {
            Some(b) => b,
            None => continue,
        };
        block_instance.apply_config(config);

        for channel in block_instance.declared_channels() {
            if channel.kind != crate::schema::ChannelKind::Hardware {
                continue;
            }

            // Check that a binding exists for this block+port+node combination.
            let has_binding = profile.peripheral_assignments.iter().any(|pb| {
                pb.block_id == block_id
                    && pb.port_name == channel.name
                    && pb.node == *node_id
            });

            if !has_binding {
                errors.push(ValidationError::MissingBoardBinding {
                    block_id,
                    channel_name: channel.name.clone(),
                    block_type: block_type.clone(),
                    node_id: node_id.clone(),
                });
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
        // Node assigned with a board that has an unknown MCU family.
        profile.add_board("my_node", "UnknownChip9000");
        profile.node_assignments.insert(0, "my_node".into());
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
        profile.add_board("motor_ctrl", "Rp2040");
        // Two ADC blocks (block 0 and block 1), both assigned pin "PA0" on node "motor_ctrl".
        profile.node_assignments.insert(0, "motor_ctrl".into());
        profile.node_assignments.insert(1, "motor_ctrl".into());

        let binding_a = make_binding(0, "adc0", "motor_ctrl", vec!["PA0"]);
        let binding_b = make_binding(1, "adc1", "motor_ctrl", vec!["PA0"]);
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
                    if pin == "PA0" && node == "motor_ctrl"
            )),
            "expected PinConflict, got: {:?}",
            errors
        );
    }

    /// Valid profile: ADC block with a correct binding and a known MCU → Ok.
    #[test]
    fn validate_profile_valid_with_correct_bindings_ok() {
        let mut profile = DeploymentProfile::new("test");
        profile.add_board("motor_ctrl", "Rp2040");
        profile.node_assignments.insert(0, "motor_ctrl".into());

        // Provide a binding for block_id=0, port_name="adc0" (matches the adc block's declared channel).
        let binding = make_binding(0, "adc0", "motor_ctrl", vec!["GP26"]);
        profile.peripheral_assignments.push(binding);

        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
        ];

        assert!(
            validate_profile(&profile, &blocks).is_ok(),
            "expected Ok for valid profile"
        );
    }

    // ── Multi-board tests ──────────────────────────────────────────────

    #[test]
    fn add_board_and_assign_block() {
        let mut profile = DeploymentProfile::new("multi");
        profile.add_board("board_a", "Rp2040");
        profile.add_board("board_b", "Stm32f4");
        profile.assign_block(0, "board_a");
        profile.assign_block(1, "board_b");

        assert_eq!(profile.boards.len(), 2);
        assert_eq!(
            profile.node_assignments.get(&0).map(String::as_str),
            Some("board_a")
        );
        assert_eq!(
            profile.node_assignments.get(&1).map(String::as_str),
            Some("board_b")
        );
    }

    #[test]
    fn add_board_deduplicates() {
        let mut profile = DeploymentProfile::new("dedup");
        profile.add_board("board_a", "Rp2040");
        profile.add_board("board_a", "Stm32f4"); // same node_id, should be ignored
        assert_eq!(profile.boards.len(), 1);
        assert_eq!(profile.boards[0].mcu_family, "Rp2040");
    }

    #[test]
    fn to_manifest_produces_correct_topology() {
        let mut profile = DeploymentProfile::new("two_boards");
        profile.add_board("motor_ctrl", "Rp2040");
        profile.add_board("sensor_hub", "Stm32f4");
        profile.assign_block(0, "motor_ctrl");
        profile.assign_block(1, "motor_ctrl");
        profile.assign_block(2, "sensor_hub");

        let manifest = profile.to_manifest(100.0);

        assert_eq!(manifest.topology.nodes.len(), 2);
        assert!(manifest.topology.nodes.iter().any(|n| n.id == "motor_ctrl" && n.mcu_family == "Rp2040"));
        assert!(manifest.topology.nodes.iter().any(|n| n.id == "sensor_hub" && n.mcu_family == "Stm32f4"));
        assert_eq!(manifest.tasks.len(), 2);

        let motor_task = manifest.tasks.iter().find(|t| t.node == "motor_ctrl").unwrap();
        assert!(motor_task.blocks.contains(&0));
        assert!(motor_task.blocks.contains(&1));

        let sensor_task = manifest.tasks.iter().find(|t| t.node == "sensor_hub").unwrap();
        assert_eq!(sensor_task.blocks, vec![2]);
    }

    #[test]
    fn to_manifest_uses_tick_hz() {
        let mut profile = DeploymentProfile::new("tick_test");
        profile.add_board("node0", "Host");
        profile.assign_block(0, "node0");

        let manifest = profile.to_manifest(200.0);
        let task = &manifest.tasks[0];
        match &task.trigger {
            module_traits::deployment::TaskTrigger::Periodic { hz } => {
                assert!((hz - 200.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Periodic trigger"),
        }
    }

    #[test]
    fn to_manifest_copies_peripheral_bindings() {
        let mut profile = DeploymentProfile::new("bindings_test");
        profile.add_board("board_a", "Rp2040");
        profile.assign_block(0, "board_a");
        let binding = make_binding(0, "adc0", "board_a", vec!["GP26"]);
        profile.peripheral_assignments.push(binding);

        let manifest = profile.to_manifest(50.0);
        assert_eq!(manifest.peripheral_bindings.len(), 1);
        assert_eq!(manifest.peripheral_bindings[0].node, "board_a");
    }

    #[test]
    fn serde_backward_compat_no_boards_field() {
        // Simulate a legacy profile JSON without "boards" field.
        let json = r#"{"name":"legacy","channel_map":{},"node_assignments":{},"peripheral_assignments":[]}"#;
        let profile: DeploymentProfile = serde_json::from_str(json).expect("deserialize legacy");
        assert!(profile.boards.is_empty());
        assert_eq!(profile.name, "legacy");
    }

    #[test]
    fn serde_roundtrip_with_boards() {
        let mut profile = DeploymentProfile::new("roundtrip");
        profile.add_board("node_a", "Rp2040");
        profile.add_board("node_b", "Stm32f4");
        profile.assign_block(0, "node_a");

        let json = serde_json::to_string(&profile).expect("serialize");
        let decoded: DeploymentProfile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.boards.len(), 2);
        assert_eq!(decoded.boards[0].node_id, "node_a");
        assert_eq!(decoded.boards[1].mcu_family, "Stm32f4");
    }

    // ── validate_multi_board tests ─────────────────────────────────────

    #[test]
    fn validate_multi_board_empty_boards_passes() {
        let profile = DeploymentProfile::new("empty");
        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
        ];
        // No boards → legacy mode → always passes.
        assert!(validate_multi_board(&profile, &blocks).is_ok());
    }

    #[test]
    fn validate_multi_board_unknown_mcu_error() {
        let mut profile = DeploymentProfile::new("bad_mcu");
        profile.add_board("node0", "FakeChip");
        let blocks: Vec<(String, serde_json::Value)> = vec![];
        let errors = validate_multi_board(&profile, &blocks).unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::UnknownMcu { family } if family == "FakeChip"
        )));
    }

    #[test]
    fn validate_multi_board_unknown_board_error() {
        let mut profile = DeploymentProfile::new("bad_board");
        profile.add_board("board_a", "Rp2040");
        profile.assign_block(0, "nonexistent");
        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("pid".into(), serde_json::json!({})),
        ];
        let errors = validate_multi_board(&profile, &blocks).unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::UnknownBoard { block_id: 0, ref node_id } if node_id == "nonexistent"
        )));
    }

    #[test]
    fn validate_multi_board_missing_binding_for_board() {
        let mut profile = DeploymentProfile::new("missing_binding");
        profile.add_board("board_a", "Rp2040");
        profile.add_board("board_b", "Stm32f4");
        // Assign ADC block to board_b but only provide binding for board_a.
        profile.assign_block(0, "board_b");
        let binding = make_binding(0, "adc0", "board_a", vec!["GP26"]);
        profile.peripheral_assignments.push(binding);

        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
        ];
        let errors = validate_multi_board(&profile, &blocks).unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::MissingBoardBinding {
                block_id: 0,
                ref channel_name,
                ref node_id,
                ..
            } if channel_name == "adc0" && node_id == "board_b"
        )));
    }

    #[test]
    fn validate_multi_board_valid_two_boards() {
        let mut profile = DeploymentProfile::new("valid_multi");
        profile.add_board("board_a", "Rp2040");
        profile.add_board("board_b", "Stm32f4");
        profile.assign_block(0, "board_a");
        profile.assign_block(1, "board_b");
        // Binding for block 0 on board_a
        profile.peripheral_assignments.push(make_binding(0, "adc0", "board_a", vec!["GP26"]));
        // Binding for block 1 on board_b
        profile.peripheral_assignments.push(make_binding(1, "adc1", "board_b", vec!["PA0"]));

        let blocks: Vec<(String, serde_json::Value)> = vec![
            ("adc".into(), serde_json::json!({"channel_name": "adc0"})),
            ("adc".into(), serde_json::json!({"channel_name": "adc1"})),
        ];
        assert!(
            validate_multi_board(&profile, &blocks).is_ok(),
            "expected Ok for valid multi-board profile"
        );
    }
}
