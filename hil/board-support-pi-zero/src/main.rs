//! Raspberry Pi Zero Linux HIL node.
//!
//! Exposes real Linux I2C buses and simulated [`RuntimeBus`] instances
//! over the same WebSocket/CBOR protocol used by the Pico firmware,
//! plus hil-backplane UDP discovery and optional CMSIS-DAP SWD debug
//! probe support.
//!
//! ```text
//! board-support-pi-zero --i2c-bus /dev/i2c-1 --sim-buses 4 --port 8080
//! board-support-pi-zero --enable-dap --swclk-gpio 24 --swdio-gpio 25
//! ```

#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use clap::Parser;
use dap_dispatch::protocol::DapProcessor;

mod combined_buses;
mod dap_processor;
mod linux_i2c;
mod ws_server;

use combined_buses::CombinedBusSet;
use linux_i2c::LinuxI2cBus;

/// Raspberry Pi Zero HIL node with real and simulated I2C buses.
#[derive(Parser)]
#[command(name = "board-support-pi-zero")]
struct Cli {
    /// Real I2C bus device paths (e.g. /dev/i2c-1). Repeatable.
    #[arg(long = "i2c-bus", value_name = "PATH")]
    i2c_buses: Vec<String>,

    /// Number of simulated I2C buses (max 32).
    #[arg(long, default_value = "10")]
    sim_buses: u8,

    /// WebSocket server port.
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Auto-detect /dev/i2c-* devices instead of specifying paths.
    #[arg(long)]
    auto_detect: bool,

    /// Backplane node ID.
    #[arg(long, default_value = "100")]
    node_id: u32,

    /// Enable CMSIS-DAP SWD debug probe.
    #[arg(long)]
    enable_dap: bool,

    /// GPIO chip device path for SWD pins.
    #[arg(long, default_value = "/dev/gpiochip0")]
    gpio_chip: String,

    /// GPIO line offset for SWCLK pin.
    #[arg(long, default_value = "24")]
    swclk_gpio: u32,

    /// GPIO line offset for SWDIO pin.
    #[arg(long, default_value = "25")]
    swdio_gpio: u32,

    /// GPIO line offset for nRESET pin.
    #[arg(long, default_value = "18")]
    nreset_gpio: u32,
}

/// Scans `/dev` for `i2c-*` device nodes, returning sorted paths.
fn detect_i2c_devices() -> Vec<String> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir("/dev").into_iter().flatten() {
        if let Ok(entry) = entry {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("i2c-") {
                paths.push(format!("/dev/{name}"));
            }
        }
    }
    paths.sort();
    paths
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    let i2c_paths = if cli.auto_detect && cli.i2c_buses.is_empty() {
        detect_i2c_devices()
    } else {
        cli.i2c_buses
    };

    let mut linux_buses = Vec::new();
    for path in &i2c_paths {
        match LinuxI2cBus::open(path) {
            Ok(bus) => {
                log::info!("Opened real I2C bus: {path}");
                linux_buses.push(bus);
            }
            Err(e) => {
                log::error!("{e}");
                return Err(e.into());
            }
        }
    }

    let dap: Option<Arc<Mutex<Box<dyn DapProcessor + Send>>>> = if cli.enable_dap {
        log::info!(
            "DAP enabled (gpio_chip={}, swclk={}, swdio={}, nreset={})",
            cli.gpio_chip,
            cli.swclk_gpio,
            cli.swdio_gpio,
            cli.nreset_gpio,
        );
        let processor = dap_processor::create_dap_processor();
        Some(Arc::new(Mutex::new(Box::new(processor))))
    } else {
        log::info!("DAP disabled (use --enable-dap to enable)");
        None
    };

    log::info!(
        "Starting with {} real + {} simulated I2C buses on port {}",
        linux_buses.len(),
        cli.sim_buses,
        cli.port,
    );

    let buses = Arc::new(Mutex::new(CombinedBusSet::new(linux_buses, cli.sim_buses)));

    let ws_buses = buses.clone();
    let ws_dap = dap.clone();
    let ws_port = cli.port;
    let ws_handle = tokio::spawn(async move {
        if let Err(e) = ws_server::run(ws_buses, ws_dap, ws_port).await {
            log::error!("WebSocket server error: {e}");
        }
    });

    let announce_handle = tokio::spawn(backplane_announce(cli.node_id, cli.port));

    tokio::signal::ctrl_c().await?;
    log::info!("Shutting down...");

    ws_handle.abort();
    announce_handle.abort();

    Ok(())
}

/// Periodically broadcasts a [`NodeAnnounce`] on the backplane multicast group.
async fn backplane_announce(node_id: u32, ws_port: u16) {
    use hil_backplane::discovery::NodeAnnounce;
    use hil_backplane::node::Node;
    use hil_backplane::node_id::NodeId;
    use hil_backplane::transport::udp::UdpTransport;

    let transport = match UdpTransport::with_defaults() {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to create backplane transport: {e}");
            return;
        }
    };

    let mut node = Node::new(NodeId::new(node_id), transport);

    let mut name = heapless::String::<64>::new();
    if core::fmt::Write::write_fmt(&mut name, format_args!("pi-zero:{ws_port}")).is_err() {
        log::error!("Node name too long");
        return;
    }

    let announce = NodeAnnounce {
        node_id: NodeId::new(node_id),
        name,
        publishes: heapless::Vec::new(),
        serves: heapless::Vec::new(),
    };

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        interval.tick().await;
        if let Err(e) = node.publish(&announce) {
            log::warn!("Backplane announce failed: {e}");
        }
    }
}
