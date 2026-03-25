//! # gs-usb-device
//!
//! A `no_std` Rust implementation of the device side of the
//! [gs_usb protocol](https://github.com/torvalds/linux/blob/master/drivers/net/can/usb/gs_usb.c),
//! built on [`embassy-usb`](https://docs.embassy.dev/embassy-usb/).
//!
//! When a microcontroller running this library is connected to a Linux host via
//! USB, the host's `gs_usb` kernel module automatically recognizes it and
//! creates a SocketCAN network interface (e.g. `can0`). No custom host-side
//! drivers are needed.
//!
//! ## Why gs_usb?
//!
//! Of the Linux kernel's USB-CAN drivers, `gs_usb` is the only practical
//! choice for a new device implementation:
//!
//! | Driver | Protocol | Practical for new HW? |
//! |---|---|---|
//! | **gs_usb.c** | Open vendor-class protocol | **Yes** — documented, open VID/PID available |
//! | ems_usb.c | Proprietary (EMS Dr. Thomas Wuensche) | No — vendor-locked |
//! | esd_usb.c | Proprietary (esd electronics) | No — vendor-locked |
//! | f81604.c | Proprietary (Fintek) | No — vendor-locked |
//! | mcba_usb.c | Proprietary (Microchip CAN BUS Analyzer) | No — vendor-locked |
//! | nct6694_canfd.c | Proprietary (Nuvoton NCT6694) | No — vendor-locked |
//! | ucan.c | Proprietary (Theobroma Systems) | No — vendor-locked |
//! | usb_8dev.c | Proprietary (8 Devices) | No — vendor-locked |
//!
//! ## Architecture
//!
//! ```text
//!  ┌──────────────────────────────┐     ┌─────────────────┐
//!  │  Linux Host                  │     │  MCU (Embassy)   │
//!  │                              │ USB │                  │
//!  │  ip link set can0 up ...     │◄───►│  gs-usb-device   │
//!  │  candump can0                │     │    ├─ class.rs   │──► CAN peripheral
//!  │  cansend can0 123#DEADBEEF   │     │    ├─ protocol.rs│
//!  │                              │     │    └─ can.rs     │
//!  │  gs_usb kernel module        │     │                  │
//!  └──────────────────────────────┘     └─────────────────┘
//! ```
//!
//! ## Protocol Summary
//!
//! - **Probe**: Host sends `BREQ_HOST_FORMAT` (byte order), reads
//!   `BREQ_DEVICE_CONFIG` and `BREQ_BT_CONST` via vendor control transfers.
//! - **Open**: Host sends `BREQ_BITTIMING` then `BREQ_MODE(START)`.
//! - **TX**: Host sends `gs_host_frame` via bulk OUT. Device echoes it back
//!   on bulk IN with the same `echo_id` after CAN transmission completes.
//! - **RX**: Device sends `gs_host_frame` on bulk IN with `echo_id = 0xFFFFFFFF`.
//! - **Close**: Host sends `BREQ_MODE(RESET)`.
//!
//! ## Usage
//!
//! ```ignore
//! use gs_usb_device::{GsUsbConfig, GsUsbState, GsUsbHandler, add_gs_usb_interface};
//! use gs_usb_device::{read_host_tx_frame, write_host_frame, frame_to_host_frame};
//!
//! // 1. Configure
//! let gs_config = GsUsbConfig {
//!     num_channels: 1,
//!     fclk_can: 80_000_000,
//!     ..Default::default()
//! };
//!
//! // 2. Create state and handler
//! static mut STATE: Option<GsUsbState> = None;
//! // ... (use StaticCell or similar for 'static lifetime)
//!
//! // 3. Add to embassy-usb builder
//! let (ep_in, ep_out) = add_gs_usb_interface(&mut builder, &mut state, &mut handler, 64);
//!
//! // 4. Run bulk endpoint tasks in #[embassy_executor::task] functions.
//! // Bulk endpoints cannot be used inside Handler (synchronous).
//! //
//! // Task A: Host → Device (TX requests)
//! #[embassy_executor::task]
//! async fn can_tx_task(
//!     mut ep_out: Endpoint<'static, USB, Out>,
//!     mut ep_in: Endpoint<'static, USB, In>,
//! ) -> ! {
//!     let mut buf = [0u8; 64];
//!     let mut tx_buf = [0u8; 64];
//!     loop {
//!         if let Some(frame) = read_host_tx_frame(&mut ep_out, &mut buf).await {
//!             let can_frame = GsCanFrame::from_host_frame(&frame);
//!             can_peripheral.transmit(&can_frame);
//!             // Echo back
//!             write_host_frame(&mut ep_in, &frame, &mut tx_buf, false).await.ok();
//!         }
//!     }
//! }
//!
//! // Task B: Device → Host (RX frames from CAN bus)
//! #[embassy_executor::task]
//! async fn can_rx_task(
//!     mut ep_in: Endpoint<'static, USB, In>,
//! ) -> ! {
//!     let mut buf = [0u8; 64];
//!     loop {
//!         let rx_frame = can_peripheral.receive().await;
//!         let host_frame = frame_to_host_frame(&rx_frame, 0, None);
//!         write_host_frame(&mut ep_in, &host_frame, &mut buf, false).await.ok();
//!     }
//! }
//! ```
//!
//! ## Host Setup
//!
//! ```bash
//! # The gs_usb module loads automatically. Then:
//! sudo ip link set can0 type can bitrate 500000
//! sudo ip link set can0 up
//! candump can0
//! cansend can0 123#DEADBEEF
//! ```

#![no_std]
#![forbid(unsafe_code)]

pub mod can;
pub mod class;
pub mod protocol;

// Re-export primary API types.
pub use can::{frame_to_host_frame, GsCanFrame};
pub use class::{
    add_gs_usb_interface, read_host_tx_frame, write_host_frame, GsUsbConfig, GsUsbHandler,
    GsUsbState, CANDLELIGHT_PID, CANDLELIGHT_VID, GS_USB_EP_SIZE, GS_USB_PID, GS_USB_VID,
};
pub use protocol::HostFrame;
