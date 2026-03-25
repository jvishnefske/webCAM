//! CMSIS-DAP v2 USB vendor class for embassy-usb.
//!
//! Provides helpers to add a CMSIS-DAP v2 bulk interface to an embassy-usb
//! composite device and async functions to read/write DAP commands on the
//! bulk endpoints.
//!
//! # USB Descriptor Layout
//!
//! CMSIS-DAP v2 uses a vendor-specific USB class with bulk endpoints:
//! - `bInterfaceClass = 0xFF` (vendor-specific)
//! - `bInterfaceSubClass = 0x00`
//! - `bInterfaceProtocol = 0x00`
//! - One Bulk IN endpoint (device → host: DAP responses)
//! - One Bulk OUT endpoint (host → device: DAP commands)
//! - Interface string descriptor: "CMSIS-DAP v2"
//!
//! probe-rs identifies the interface by searching for "CMSIS-DAP" in the
//! interface string descriptor.

use embassy_usb::driver::{Driver, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::Builder;

/// Adds a CMSIS-DAP v2 vendor interface with bulk endpoints to the USB builder.
///
/// Creates a vendor-specific function (class 0xFF, subclass 0x00, protocol 0x00)
/// with one bulk IN and one bulk OUT endpoint of the specified size.
///
/// Returns `(EndpointIn, EndpointOut)` for use in an async DAP processing task.
///
/// # Panics
///
/// Panics if the builder cannot allocate the endpoints (e.g., descriptor
/// buffer overflow). This is a startup-time configuration error.
pub fn add_cmsis_dap_v2_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    ep_size: u16,
) -> (D::EndpointIn, D::EndpointOut) {
    let mut func = builder.function(0xFF, 0x00, 0x00);
    let mut iface = func.interface();
    let mut alt = iface.alt_setting(0xFF, 0x00, 0x00, None);
    let ep_in = alt.endpoint_bulk_in(ep_size);
    let ep_out = alt.endpoint_bulk_out(ep_size);
    (ep_in, ep_out)
}

/// Reads a single DAP command from the bulk OUT endpoint.
///
/// Returns the number of bytes read into `buf`.
///
/// # Errors
///
/// Returns [`EndpointError`] if the USB endpoint is disabled or the
/// device is not configured.
pub async fn read_dap_command<E: EndpointOut>(
    ep_out: &mut E,
    buf: &mut [u8],
) -> Result<usize, EndpointError> {
    ep_out.read(buf).await
}

/// Writes a DAP response to the bulk IN endpoint.
///
/// Sends `data` to the host. The caller must ensure `data.len()` does
/// not exceed the endpoint's max packet size.
///
/// # Errors
///
/// Returns [`EndpointError`] if the USB endpoint is disabled or the
/// device is not configured.
pub async fn write_dap_response<E: EndpointIn>(
    ep_in: &mut E,
    data: &[u8],
) -> Result<(), EndpointError> {
    if !data.is_empty() {
        ep_in.write(data).await?;
    }
    Ok(())
}
