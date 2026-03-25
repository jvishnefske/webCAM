//! USB-to-I2C composite adapter for Raspberry Pi Pico 2 (RP2350A).
//!
//! Exposes 10 runtime-configurable I2C buses as i2c-tiny-usb interfaces,
//! a CDC NCM Ethernet adapter, and a CMSIS-DAP v2 debug probe.
//!
//! Key difference from board-support-pico (RP2040): the USB i2c-tiny-usb
//! buses and the WebSocket runtime buses share the same RuntimeBus instances.
//! Devices added via WebSocket immediately appear on `/dev/i2c-*`.

#![no_std]
#![no_main]

#[cfg(not(target_arch = "arm"))]
compile_error!("board-support-pico2 must be built for ARM. Use: cargo build-pico2");

// RP2350 IMAGE_DEF block — required for the boot ROM to recognize this as a valid image.
// Must include VECTOR_TABLE item pointing to 0x10000100 (start of FLASH region).
#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: embassy_rp::block::ImageDefVt =
    embassy_rp::block::ImageDefVt::secure_exe_vt(0x10000100);

mod dap_pins;
mod dap_task;
mod http_static;
mod shared_buses;
mod ws_server;

use board_config_common::network;
use core::cell::RefCell;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use hil_firmware_support::{
    add_cdc_ncm, allocate_vendor_interface, create_net_stack, create_usb_builder,
    usb_device_config,
};
use shared_buses::{SharedBusMutex, SharedState, UsbI2cHandler};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

hil_firmware_support::define_ncm_tasks!(Driver<'static, USB>);

type DapEpIn = <Driver<'static, USB> as embassy_usb::driver::Driver<'static>>::EndpointIn;
type DapEpOut = <Driver<'static, USB> as embassy_usb::driver::Driver<'static>>::EndpointOut;

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
    info!("Pico 2 (RP2350A) program start");
    let p = embassy_rp::init(Default::default());

    // LED for status indication (GPIO 25 on Pico 2)
    let mut led = Output::new(p.PIN_25, Level::Low);
    led.set_high();

    // Shared runtime I2C buses: accessible from both USB and WebSocket
    static SHARED: StaticCell<SharedBusMutex> = StaticCell::new();
    let shared: &'static SharedBusMutex =
        SHARED.init(BlockingMutex::new(RefCell::new(SharedState::new())));

    // Pre-configure: 10 active buses (empty, ready for WebSocket device adds)
    shared.lock(|inner| {
        inner.borrow_mut().bus_count = 10;
    });

    // USB device: composite with vendor interfaces + CDC NCM
    let driver = Driver::new(p.USB, Irqs);
    let config = usb_device_config();
    let mut builder = create_usb_builder(driver, config);

    // Allocate 10 vendor interfaces for i2c-tiny-usb
    let if_nums: [_; 10] = core::array::from_fn(|_| allocate_vendor_interface(&mut builder));

    // Register interface numbers in shared state
    shared.lock(|inner| {
        let mut state = inner.borrow_mut();
        for (i, if_num) in if_nums.iter().enumerate() {
            state.set_interface(i, if_num.0);
        }
    });

    let (ncm_runner, net_device) =
        add_cdc_ncm(&mut builder, network::DEVICE_MAC, network::HOST_MAC);

    // USB handler backed by shared buses
    static USB_HANDLER: StaticCell<UsbI2cHandler> = StaticCell::new();
    let handler = USB_HANDLER.init(UsbI2cHandler::new(shared));
    builder.handler(handler);

    // CMSIS-DAP v2 debug probe interface
    let (dap_ep_in, dap_ep_out) =
        dap_dispatch::usb_class::add_cmsis_dap_v2_interface(&mut builder, 64);

    let usb = builder.build();
    unwrap!(spawner.spawn(usb_task(usb)));
    unwrap!(spawner.spawn(usb_ncm_task(ncm_runner)));

    // DAP processor
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
    unwrap!(spawner.spawn(ws_server::ws_server_task(stack, shared)));

    info!("10-bus shared i2c-tiny-usb + CDC NCM + WebSocket + CMSIS-DAP ready");
    info!("Devices added via WebSocket will appear on /dev/i2c-*");

    // Main loop - LED heartbeat
    loop {
        led.toggle();
        embassy_time::Timer::after_millis(500).await;
    }
}
