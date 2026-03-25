//! USB-to-I2C composite adapter with CDC NCM Ethernet and CMSIS-DAP.
//!
//! This firmware exposes ten simulated I2C buses as independent i2c-tiny-usb
//! interfaces, a CDC NCM Ethernet adapter, and a CMSIS-DAP v2 debug probe
//! within a single composite USB device. The Linux `i2c-tiny-usb` kernel
//! driver probes each vendor interface, creating ten `/dev/i2c-X` entries.
//! The CDC NCM interface enumerates as an Ethernet adapter with static IPv4
//! address 169.254.1.61/16. The CMSIS-DAP v2 interface appears as a debug
//! probe recognized by probe-rs and OpenOCD.

#![no_std]
#![no_main]

#[cfg(not(target_arch = "arm"))]
compile_error!("board-support-pico must be built for ARM. Use: cargo build-pico");

mod dap_pins;
mod dap_task;
mod fw_update;
mod http_static;
mod ws_server;

use board_config_common::network;
use core::cell::RefCell;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{FLASH, USB};
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use hil_firmware_support::{
    add_cdc_ncm, allocate_vendor_interface, create_net_stack, create_usb_builder, usb_device_config,
};
use static_cell::StaticCell;
use usb_composite_dispatchers::i2c_tiny_usb::MultiI2cHandlerBuilder;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

hil_firmware_support::define_ncm_tasks!(Driver<'static, USB>);

/// Concrete bulk IN endpoint type for the RP2040 USB driver.
type DapEpIn = <Driver<'static, USB> as embassy_usb::driver::Driver<'static>>::EndpointIn;

/// Concrete bulk OUT endpoint type for the RP2040 USB driver.
type DapEpOut = <Driver<'static, USB> as embassy_usb::driver::Driver<'static>>::EndpointOut;

/// Background task that runs the CMSIS-DAP v2 bulk endpoint loop.
#[embassy_executor::task]
async fn dap_usb_task(
    mut ep_in: DapEpIn,
    mut ep_out: DapEpOut,
    dap: &'static mut dap_pins::PicoDapProcessor,
) {
    dap_task::dap_bulk_task(&mut ep_in, &mut ep_out, dap).await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Program start");
    let p = embassy_rp::init(Default::default());

    // LED for status indication
    let mut led = Output::new(p.PIN_25, Level::Low);
    led.set_high();

    // Flash peripheral for A/B firmware updates
    static FLASH_CELL: StaticCell<
        BlockingMutex<
            NoopRawMutex,
            RefCell<Flash<'static, FLASH, Blocking, { fw_update::FLASH_SIZE }>>,
        >,
    > = StaticCell::new();
    let flash = Flash::new_blocking(p.FLASH);
    let flash_mutex = FLASH_CELL.init(BlockingMutex::new(RefCell::new(flash)));

    // Mark current firmware as booted to prevent rollback
    fw_update::mark_booted(flash_mutex);

    // Build 10 simulated I2C buses from board-config-common topologies
    let i2c0 = board_config_common::i2c0::build();
    let i2c1 = board_config_common::i2c1::build();
    let i2c2 = board_config_common::i2c2::build();
    let i2c3 = board_config_common::i2c3::build();
    let i2c4 = board_config_common::i2c4::build();
    let i2c5 = board_config_common::i2c5::build();
    let i2c6 = board_config_common::i2c6::build();
    let i2c7 = board_config_common::i2c7::build();
    let i2c8 = board_config_common::i2c8::build();
    let i2c9 = board_config_common::i2c9::build();

    // USB device: composite with vendor interfaces + CDC NCM
    let driver = Driver::new(p.USB, Irqs);
    let config = usb_device_config();
    let mut builder = create_usb_builder(driver, config);

    // Allocate 10 vendor interfaces for i2c-tiny-usb
    let if0 = allocate_vendor_interface(&mut builder);
    let if1 = allocate_vendor_interface(&mut builder);
    let if2 = allocate_vendor_interface(&mut builder);
    let if3 = allocate_vendor_interface(&mut builder);
    let if4 = allocate_vendor_interface(&mut builder);
    let if5 = allocate_vendor_interface(&mut builder);
    let if6 = allocate_vendor_interface(&mut builder);
    let if7 = allocate_vendor_interface(&mut builder);
    let if8 = allocate_vendor_interface(&mut builder);
    let if9 = allocate_vendor_interface(&mut builder);

    let (ncm_runner, net_device) =
        add_cdc_ncm(&mut builder, network::DEVICE_MAC, network::HOST_MAC);

    // Board-specific: multi-bus i2c-tiny-usb handler
    static HANDLER: StaticCell<
        usb_composite_dispatchers::i2c_tiny_usb::MultiI2cHandler<(
            board_config_common::i2c9::Bus,
            (
                board_config_common::i2c8::Bus,
                (
                    board_config_common::i2c7::Bus,
                    (
                        board_config_common::i2c6::Bus,
                        (
                            board_config_common::i2c5::Bus,
                            (
                                board_config_common::i2c4::Bus,
                                (
                                    board_config_common::i2c3::Bus,
                                    (
                                        board_config_common::i2c2::Bus,
                                        (
                                            board_config_common::i2c1::Bus,
                                            (board_config_common::i2c0::Bus, ()),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )>,
    > = StaticCell::new();
    let handler = HANDLER.init(
        MultiI2cHandlerBuilder::new()
            .with_bus(i2c0, if0)
            .with_bus(i2c1, if1)
            .with_bus(i2c2, if2)
            .with_bus(i2c3, if3)
            .with_bus(i2c4, if4)
            .with_bus(i2c5, if5)
            .with_bus(i2c6, if6)
            .with_bus(i2c7, if7)
            .with_bus(i2c8, if8)
            .with_bus(i2c9, if9)
            .build(),
    );
    builder.handler(handler);

    // CMSIS-DAP v2 debug probe interface (bulk endpoints)
    let (dap_ep_in, dap_ep_out) =
        dap_dispatch::usb_class::add_cmsis_dap_v2_interface(&mut builder, 64);

    let usb = builder.build();
    unwrap!(spawner.spawn(usb_task(usb)));
    unwrap!(spawner.spawn(usb_ncm_task(ncm_runner)));

    // DAP processor (static allocation for 'static task lifetime)
    static DAP_PROCESSOR: StaticCell<dap_pins::PicoDapProcessor> = StaticCell::new();
    let dap = DAP_PROCESSOR.init(dap_pins::PicoDapProcessor::new());
    unwrap!(spawner.spawn(dap_usb_task(dap_ep_in, dap_ep_out, dap)));

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
    unwrap!(spawner.spawn(ws_server::ws_server_task(stack, flash_mutex)));

    info!("10-bus i2c-tiny-usb + CDC NCM + WebSocket + CMSIS-DAP device ready");

    // Main loop - LED heartbeat
    loop {
        led.toggle();
        embassy_time::Timer::after_millis(500).await;
    }
}
