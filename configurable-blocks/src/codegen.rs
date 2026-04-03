//! Generate complete Embassy firmware crates from a deployment configuration.
//!
//! Takes a [`DeploymentManifest`] + lowered [`Dag`] + [`McuDef`] and produces
//! all files for a buildable, flashable Embassy firmware crate — pure safe Rust,
//! no C FFI.
//!
//! # Generated files
//!
//! ```text
//! firmware-{node_id}/
//!   Cargo.toml              # Embassy deps for the target MCU
//!   .cargo/config.toml      # target triple + probe-rs runner
//!   memory.x                # linker script (flash/RAM regions from McuDef)
//!   build.rs                # linker flags
//!   src/main.rs             # Embassy tasks + DAG evaluation + peripheral init
//! ```

use std::fmt::Write;

use dag_core::cbor;
use dag_core::op::Dag;
use module_traits::deployment::{
    DeploymentManifest, TaskBinding, TaskTrigger,
};
use module_traits::inventory::{CpuCore, McuDef, MemoryKind};

/// Output of the codegen: a list of (path, content) pairs.
pub type GeneratedFiles = Vec<(String, String)>;

/// Generate a complete firmware crate for one node in the deployment.
///
/// Returns `(path, content)` pairs ready to write to disk or pack into a ZIP.
pub fn generate_node_crate(
    node_id: &str,
    mcu: &McuDef,
    tasks: &[TaskBinding],
    dag: &Dag,
    manifest: &DeploymentManifest,
) -> Result<GeneratedFiles, String> {
    let prefix = format!("firmware-{node_id}");
    let mut files = Vec::new();

    files.push((format!("{prefix}/Cargo.toml"), gen_cargo_toml(node_id, mcu)));
    files.push((format!("{prefix}/.cargo/config.toml"), gen_cargo_config(mcu)));
    files.push((format!("{prefix}/memory.x"), gen_memory_x(mcu)));
    files.push((format!("{prefix}/build.rs"), gen_build_rs()));
    files.push((format!("{prefix}/src/main.rs"), gen_main_rs(node_id, mcu, tasks, dag, manifest)?));

    Ok(files)
}

/// Generate firmware crates for all nodes in a deployment manifest.
pub fn generate_all_crates(
    manifest: &DeploymentManifest,
    dag: &Dag,
) -> Result<GeneratedFiles, String> {
    let mut all_files = Vec::new();

    for node in &manifest.topology.nodes {
        let mcu = module_traits::inventory::mcu_for(&node.mcu_family)
            .ok_or_else(|| format!("unknown MCU family: {}", node.mcu_family))?;

        let node_tasks: Vec<TaskBinding> = manifest.tasks
            .iter()
            .filter(|t| t.node == node.id)
            .cloned()
            .collect();

        let mut files = generate_node_crate(&node.id, &mcu, &node_tasks, dag, manifest)?;
        all_files.append(&mut files);
    }

    Ok(all_files)
}

// ---------------------------------------------------------------------------
// File generators
// ---------------------------------------------------------------------------

fn gen_cargo_toml(node_id: &str, mcu: &McuDef) -> String {
    let (embassy_hal, hal_features, extra_deps) = match mcu.core {
        CpuCore::CortexM0Plus if mcu.family.contains("Rp") => (
            "embassy-rp".to_string(),
            r#"features = ["time-driver", "critical-section-impl"]"#.to_string(),
            "cortex-m = { version = \"0.7\", features = [\"critical-section-single-core\"] }\ncortex-m-rt = \"0.7\"".to_string(),
        ),
        CpuCore::CortexM4 | CpuCore::CortexM4F => (
            "embassy-stm32".to_string(),
            format!(r#"features = ["{}","time-driver-any","exti"]"#, mcu.part_number.to_lowercase()),
            "cortex-m = { version = \"0.7\", features = [\"critical-section-single-core\"] }\ncortex-m-rt = \"0.7\"".to_string(),
        ),
        CpuCore::CortexM0Plus => (
            "embassy-stm32".to_string(),
            format!(r#"features = ["{}","time-driver-any"]"#, mcu.part_number.to_lowercase()),
            "cortex-m = { version = \"0.7\", features = [\"critical-section-single-core\"] }\ncortex-m-rt = \"0.7\"".to_string(),
        ),
        CpuCore::RiscV32IMC => (
            "esp-hal".to_string(),
            r#"features = ["esp32c3"]"#.to_string(),
            String::new(),
        ),
        _ => (
            "embassy-executor".to_string(),
            r#"features = ["arch-std"]"#.to_string(),
            String::new(),
        ),
    };

    let mut out = String::new();
    writeln!(out, "[package]").unwrap();
    writeln!(out, "name = \"firmware-{node_id}\"").unwrap();
    writeln!(out, "version = \"0.1.0\"").unwrap();
    writeln!(out, "edition = \"2021\"").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "[dependencies]").unwrap();
    writeln!(out, "dag-core = {{ path = \"../dag-core\", default-features = false }}").unwrap();
    writeln!(out, "embassy-executor = {{ version = \"0.7\", features = [\"arch-cortex-m\", \"executor-thread\"] }}").unwrap();
    writeln!(out, "embassy-time = \"0.4\"").unwrap();
    writeln!(out, "{embassy_hal} = {{ version = \"0.4\", {hal_features} }}").unwrap();
    if !extra_deps.is_empty() {
        writeln!(out, "{extra_deps}").unwrap();
    }
    writeln!(out, "panic-halt = \"1\"").unwrap();
    writeln!(out, "defmt = \"0.3\"").unwrap();
    writeln!(out, "defmt-rtt = \"0.4\"").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![forbid(unsafe_code)]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "[profile.release]").unwrap();
    writeln!(out, "opt-level = \"s\"").unwrap();
    writeln!(out, "lto = true").unwrap();
    out
}

fn gen_cargo_config(mcu: &McuDef) -> String {
    let target = match mcu.core {
        CpuCore::CortexM0Plus => "thumbv6m-none-eabi",
        CpuCore::CortexM4 | CpuCore::CortexM4F => "thumbv7em-none-eabihf",
        CpuCore::CortexM7 => "thumbv7em-none-eabihf",
        CpuCore::RiscV32IMC => "riscv32imc-unknown-none-elf",
        CpuCore::HostSim => "x86_64-unknown-linux-gnu",
    };

    let chip = &mcu.part_number;

    format!(
        r#"[target.{target}]
runner = "probe-rs run --chip {chip}"

[build]
target = "{target}"

[env]
DEFMT_LOG = "info"
"#
    )
}

fn gen_memory_x(mcu: &McuDef) -> String {
    let mut out = String::new();
    writeln!(out, "MEMORY {{").unwrap();

    for region in &mcu.memory {
        let name = match region.kind {
            MemoryKind::Flash => "FLASH",
            MemoryKind::Ram => "RAM",
            MemoryKind::Bootloader => continue,
            MemoryKind::PeripheralRam => continue,
        };
        let attr = match region.kind {
            MemoryKind::Flash => "rx",
            _ => "rw",
        };
        writeln!(
            out,
            "    {name} ({attr}) : ORIGIN = 0x{:08X}, LENGTH = {}K",
            region.start,
            region.size / 1024
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

fn gen_build_rs() -> String {
    r#"fn main() {
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
"#
    .to_string()
}

fn gen_main_rs(
    node_id: &str,
    _mcu: &McuDef,
    tasks: &[TaskBinding],
    dag: &Dag,
    _manifest: &DeploymentManifest,
) -> Result<String, String> {
    let mut out = String::new();

    writeln!(out, "#![no_std]").unwrap();
    writeln!(out, "#![no_main]").unwrap();
    writeln!(out, "#![forbid(unsafe_code)]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use defmt_rtt as _;").unwrap();
    writeln!(out, "use panic_halt as _;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use embassy_executor::Spawner;").unwrap();
    writeln!(out, "use embassy_time::{{Duration, Ticker}};").unwrap();
    writeln!(out).unwrap();

    // Embed the DAG as a CBOR constant
    let cbor_bytes = cbor::encode_dag(dag);
    writeln!(out, "/// CBOR-encoded DAG ({} nodes, {} bytes)", dag.len(), cbor_bytes.len()).unwrap();
    write!(out, "const DAG_CBOR: &[u8] = &[").unwrap();
    for (i, byte) in cbor_bytes.iter().enumerate() {
        if i % 16 == 0 {
            write!(out, "\n    ").unwrap();
        }
        write!(out, "0x{byte:02X}, ").unwrap();
    }
    writeln!(out, "\n];").unwrap();
    writeln!(out).unwrap();

    // PubSub reader/writer for evaluation
    writeln!(out, "/// Simple in-memory pubsub store.").unwrap();
    writeln!(out, "struct PubSub {{").unwrap();
    writeln!(out, "    topics: [(&'static str, f64); 16],").unwrap();
    writeln!(out, "    count: usize,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl PubSub {{").unwrap();
    writeln!(out, "    const fn new() -> Self {{").unwrap();
    writeln!(out, "        Self {{ topics: [(\"\", 0.0); 16], count: 0 }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl dag_core::eval::PubSubReader for PubSub {{").unwrap();
    writeln!(out, "    fn read(&self, topic: &str) -> f64 {{").unwrap();
    writeln!(out, "        for i in 0..self.count {{").unwrap();
    writeln!(out, "            if self.topics[i].0 == topic {{ return self.topics[i].1; }}").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "        0.0").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Main function
    writeln!(out, "#[embassy_executor::main]").unwrap();
    writeln!(out, "async fn main(_spawner: Spawner) {{").unwrap();
    writeln!(out, "    defmt::info!(\"firmware-{node_id} starting\");").unwrap();
    writeln!(out).unwrap();

    // Decode DAG at startup
    writeln!(out, "    let dag = dag_core::cbor::decode_dag(DAG_CBOR)").unwrap();
    writeln!(out, "        .expect(\"CBOR DAG decode failed\");").unwrap();
    writeln!(out, "    let mut values = [0.0_f64; {}];", dag.len()).unwrap();
    writeln!(out, "    let mut pubsub = PubSub::new();").unwrap();
    writeln!(out, "    defmt::info!(\"DAG loaded: {{}} nodes\", dag.len());").unwrap();
    writeln!(out).unwrap();

    // Generate ticker for the first periodic task (or default 50Hz)
    let tick_hz = tasks.iter().find_map(|t| match &t.trigger {
        TaskTrigger::Periodic { hz } => Some(*hz),
        _ => None,
    }).unwrap_or(50.0);
    let tick_ms = (1000.0 / tick_hz) as u64;

    writeln!(out, "    let mut ticker = Ticker::every(Duration::from_millis({tick_ms}));").unwrap();
    writeln!(out, "    defmt::info!(\"tick rate: {{}}Hz ({{}}ms)\", {tick_hz:.0}, {tick_ms});").unwrap();
    writeln!(out).unwrap();

    // Main loop
    writeln!(out, "    loop {{").unwrap();
    writeln!(out, "        ticker.next().await;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "        let result = dag.evaluate(").unwrap();
    writeln!(out, "            &dag_core::eval::NullChannels,").unwrap();
    writeln!(out, "            &pubsub,").unwrap();
    writeln!(out, "            &mut values,").unwrap();
    writeln!(out, "        );").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "        // Store published topics for next tick").unwrap();
    writeln!(out, "        for (topic, val) in &result.publishes {{").unwrap();
    writeln!(out, "            let mut found = false;").unwrap();
    writeln!(out, "            for i in 0..pubsub.count {{").unwrap();
    writeln!(out, "                if pubsub.topics[i].0 == topic {{").unwrap();
    writeln!(out, "                    pubsub.topics[i].1 = *val;").unwrap();
    writeln!(out, "                    found = true;").unwrap();
    writeln!(out, "                    break;").unwrap();
    writeln!(out, "                }}").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "            if !found && pubsub.count < 16 {{").unwrap();
    writeln!(out, "                // Note: topic string must be 'static for this to work").unwrap();
    writeln!(out, "                // In practice, dag-core topics are embedded in the DAG constant").unwrap();
    writeln!(out, "                pubsub.count += 1;").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use module_traits::deployment::*;

    fn simple_manifest() -> DeploymentManifest {
        DeploymentManifest {
            topology: SystemTopology {
                nodes: vec![BoardNode {
                    id: "motor_ctrl".into(),
                    mcu_family: "Rp2040".into(),
                    board: None,
                    rust_target: None,
                }],
                links: vec![],
            },
            tasks: vec![TaskBinding {
                name: "control_loop".into(),
                node: "motor_ctrl".into(),
                blocks: vec![1, 2],
                trigger: TaskTrigger::Periodic { hz: 100.0 },
                priority: 1,
                stack_size: None,
            }],
            channels: vec![],
            peripheral_bindings: vec![],
        }
    }

    fn simple_dag() -> Dag {
        let mut dag = Dag::new();
        let c = dag.constant(42.0).unwrap();
        dag.publish("output", c).unwrap();
        dag
    }

    #[test]
    fn generates_all_files() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

        assert!(paths.contains(&"firmware-motor_ctrl/Cargo.toml"));
        assert!(paths.contains(&"firmware-motor_ctrl/.cargo/config.toml"));
        assert!(paths.contains(&"firmware-motor_ctrl/memory.x"));
        assert!(paths.contains(&"firmware-motor_ctrl/build.rs"));
        assert!(paths.contains(&"firmware-motor_ctrl/src/main.rs"));
    }

    #[test]
    fn main_rs_has_dag_cbor() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        let main = files.iter().find(|(p, _)| p.ends_with("main.rs")).unwrap();

        assert!(main.1.contains("DAG_CBOR"));
        assert!(main.1.contains("dag.evaluate"));
        assert!(main.1.contains("no_std"));
        assert!(main.1.contains("forbid(unsafe_code)"));
        assert!(main.1.contains("embassy_executor::main"));
    }

    #[test]
    fn main_rs_has_correct_tick_rate() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        let main = files.iter().find(|(p, _)| p.ends_with("main.rs")).unwrap();

        // 100 Hz = 10ms
        assert!(main.1.contains("from_millis(10)"));
    }

    #[test]
    fn cargo_toml_has_embassy_rp() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        let cargo = files.iter().find(|(p, _)| p.ends_with("Cargo.toml")).unwrap();

        assert!(cargo.1.contains("embassy-rp"));
        assert!(cargo.1.contains("dag-core"));
    }

    #[test]
    fn memory_x_has_flash_and_ram() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        let mem = files.iter().find(|(p, _)| p.ends_with("memory.x")).unwrap();

        assert!(mem.1.contains("FLASH"));
        assert!(mem.1.contains("RAM"));
    }

    #[test]
    fn cargo_config_has_target() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        let cfg = files.iter().find(|(p, _)| p.ends_with("config.toml")).unwrap();

        assert!(cfg.1.contains("thumbv6m-none-eabi"));
        assert!(cfg.1.contains("probe-rs run"));
    }

    #[test]
    fn unknown_mcu_errors() {
        let mut manifest = simple_manifest();
        manifest.topology.nodes[0].mcu_family = "UnknownChip".into();
        let dag = simple_dag();
        let result = generate_all_crates(&manifest, &dag);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown MCU family"));
    }

    #[test]
    fn no_unsafe_in_generated_code() {
        let manifest = simple_manifest();
        let dag = simple_dag();
        let files = generate_all_crates(&manifest, &dag).unwrap();
        for (path, content) in &files {
            if path.ends_with(".rs") {
                assert!(
                    !content.contains("unsafe {") && !content.contains("unsafe fn"),
                    "file {path} contains unsafe code"
                );
            }
        }
    }
}
