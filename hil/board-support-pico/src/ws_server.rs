//! Embassy task wrapper for the WebSocket server.
//!
//! Delegates to [`hil_firmware_support::ws_server::run`] with the
//! board-specific [`RuntimeBusSet`](hil_firmware_support::runtime_buses::RuntimeBusSet),
//! static assets, and DFU flash writer.

use crate::fw_update::{FlashMutex, PicoDfuWriter};

/// Embassy task that runs the HTTP and WebSocket server.
///
/// Creates a runtime-configurable set of I2C buses (independent from the
/// USB i2c-tiny-usb handler) and delegates to the generic server loop
/// in `hil_firmware_support`. The flash mutex is used to create a
/// [`PicoDfuWriter`] for firmware updates over WebSocket.
#[embassy_executor::task]
pub async fn ws_server_task(stack: embassy_net::Stack<'static>, flash: &'static FlashMutex) -> ! {
    let mut buses = hil_firmware_support::runtime_buses::RuntimeBusSet::<10, 8>::new();
    let assets = crate::http_static::assets();
    let mut fw_writer = PicoDfuWriter::new(flash);
    hil_firmware_support::ws_server::run(stack, &mut buses, &assets, &mut fw_writer).await
}
