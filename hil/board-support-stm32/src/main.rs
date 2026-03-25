//! CDC NCM Ethernet adapter with WebSocket I2C bus control for STM32.
//!
//! This firmware exposes a CDC NCM Ethernet adapter within a USB composite
//! device. The CDC NCM interface enumerates as an Ethernet adapter with
//! static IPv4 address 169.254.1.61/16. A WebSocket server on port 8080
//! provides runtime-configurable simulated I2C buses via CBOR commands.
//!
//! Unlike the RP2040 variant, USB i2c-tiny-usb interfaces and CMSIS-DAP
//! are omitted because STM32F4 USB OTG FS has only 4 IN + 4 OUT endpoints.
//!
//! # Supported chips
//!
//! Enable exactly one feature: `stm32f401cc`, `stm32f411ce`, or `stm32h743vi`.

#![no_std]
#![no_main]
#![forbid(unsafe_code)]

#[cfg(not(target_arch = "arm"))]
compile_error!("board-support-stm32 must be built for ARM. Use: cargo build-stm32");

mod chip;
mod http_static;
mod ws_dispatch;
mod ws_server;

use board_config_common::network;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use hil_firmware_support::{add_cdc_ncm, create_net_stack, create_usb_builder, usb_device_config};
use {defmt_rtt as _, panic_probe as _};

hil_firmware_support::define_ncm_tasks!(chip::UsbDriver);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Program start");
    let (driver, mut led) = chip::init();

    // LED on for status
    led.set_high();

    // USB device: composite with CDC NCM only (no vendor interfaces)
    let config = usb_device_config();
    let mut builder = create_usb_builder(driver, config);

    let (ncm_runner, net_device) =
        add_cdc_ncm(&mut builder, network::DEVICE_MAC, network::HOST_MAC);

    let usb = builder.build();
    unwrap!(spawner.spawn(usb_task(usb)));
    unwrap!(spawner.spawn(usb_ncm_task(ncm_runner)));

    // Network stack
    let (stack, net_runner) = create_net_stack(
        net_device,
        network::IPV4_ADDR,
        network::IPV4_PREFIX,
        network::IPV6_SEGMENTS,
        network::IPV6_PREFIX,
    );
    unwrap!(spawner.spawn(net_task(net_runner)));
    unwrap!(spawner.spawn(dhcp_responder_task(stack)));
    unwrap!(spawner.spawn(ws_server::ws_server_task(stack)));

    info!("CDC NCM + WebSocket I2C device ready");

    // Main loop — LED heartbeat
    loop {
        led.toggle();
        embassy_time::Timer::after_millis(500).await;
    }
}
