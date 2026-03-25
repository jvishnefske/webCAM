//! CMSIS-DAP v2 USB bulk endpoint task.
//!
//! Runs a loop reading DAP commands from the bulk OUT endpoint,
//! processing them through a [`DapProcessor`], and writing responses
//! to the bulk IN endpoint.

use dap_dispatch::protocol::DapProcessor;
use dap_dispatch::usb_class;
use embassy_usb::driver::{EndpointIn, EndpointOut};

/// Runs the CMSIS-DAP v2 bulk endpoint loop.
///
/// Reads raw DAP commands from `ep_out`, passes them through
/// `dap.process_command()`, and writes the response to `ep_in`.
/// Runs forever until the USB device is disconnected.
pub async fn dap_bulk_task<I: EndpointIn, O: EndpointOut, P: DapProcessor>(
    ep_in: &mut I,
    ep_out: &mut O,
    dap: &mut P,
) {
    let mut cmd_buf = [0u8; 64];
    let mut resp_buf = [0u8; 64];

    loop {
        match usb_class::read_dap_command(ep_out, &mut cmd_buf).await {
            Ok(n) if n > 0 => {
                let resp_len = dap.process_command(&cmd_buf[..n], &mut resp_buf);
                if resp_len > 0 {
                    if usb_class::write_dap_response(ep_in, &resp_buf[..resp_len])
                        .await
                        .is_err()
                    {
                        // USB disconnected, wait for reconnect
                        continue;
                    }
                }
            }
            _ => {
                // USB disconnected or empty read, wait for reconnect
                continue;
            }
        }
    }
}
