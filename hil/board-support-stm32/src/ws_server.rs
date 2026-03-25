//! Embassy task wrapper for the WebSocket server.
//!
//! Delegates to [`hil_firmware_support::ws_server::run`] with the
//! board-specific [`StmBusSet`](crate::ws_dispatch::StmBusSet),
//! static assets, and a stub firmware writer (OTA deferred on STM32).

use hil_firmware_support::fw_update::StubDfuWriter;

/// Embassy task that runs the HTTP and WebSocket server.
///
/// Creates a runtime-configurable set of I2C buses and delegates to the
/// generic server loop in `hil_firmware_support`. OTA firmware update
/// requests are rejected by the stub writer.
#[embassy_executor::task]
pub async fn ws_server_task(stack: embassy_net::Stack<'static>) -> ! {
    let mut buses = crate::ws_dispatch::StmBusSet::new();
    let assets = crate::http_static::assets();
    let mut fw_writer = StubDfuWriter::new();
    hil_firmware_support::ws_server::run(stack, &mut buses, &assets, &mut fw_writer).await
}
