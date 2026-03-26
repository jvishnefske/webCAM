//! WebSocket server task for Pico 2.
//!
//! Uses the shared bus mutex so devices added via WebSocket are
//! immediately visible on the USB i2c-tiny-usb interfaces.

use crate::shared_buses::{SharedBusMutex, WsBusAccess};

/// Embassy task that runs the HTTP and WebSocket server with DAG API.
///
/// Serves the DAG editor frontend, handles POST /api/dag for CBOR uploads,
/// and runs the existing I2C WebSocket dispatch.
#[embassy_executor::task]
pub async fn ws_server_task(
    stack: embassy_net::Stack<'static>,
    shared: &'static SharedBusMutex,
    dag: &'static mut crate::dag_handler::DagApiHandler,
) -> ! {
    let mut buses = WsBusAccess::new(shared);
    let assets = crate::http_static::assets();
    let mut fw_writer = NullDfuWriter;
    hil_firmware_support::ws_server::run_with_api(stack, &mut buses, &assets, &mut fw_writer, dag)
        .await
}

/// Stub DFU writer — Pico 2 is flashed via probe-rs, not OTA.
struct NullDfuWriter;

impl hil_firmware_support::fw_update::DfuFlashWriter for NullDfuWriter {
    fn erase_dfu(&mut self) -> Result<(), ()> {
        Err(())
    }
    fn write_dfu(&mut self, _offset: u32, _data: &[u8]) -> Result<(), ()> {
        Err(())
    }
    fn read_dfu(&mut self, _offset: u32, _buf: &mut [u8]) -> Result<(), ()> {
        Err(())
    }
    fn mark_updated(&mut self) -> Result<(), ()> {
        Err(())
    }
    fn mark_booted(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn system_reset(&mut self) -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}
