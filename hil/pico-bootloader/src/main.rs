//! A/B partition bootloader for RP2040.
//!
//! Uses embassy-boot-rp to manage active and DFU flash partitions.
//! On boot, checks if a new firmware image has been staged in the DFU
//! partition and swaps it into the active partition before jumping to
//! the application.
//!
//! This is the **only** crate in the project that uses `unsafe` — the
//! `BootLoader::load()` call requires it to transfer control to the
//! application entry point.

#![no_std]
#![no_main]

#[cfg(not(target_arch = "arm"))]
compile_error!("pico-bootloader must be built for ARM. Use: cargo build-bootloader");

use core::cell::RefCell;

use defmt_rtt as _;

use cortex_m_rt::entry;
use embassy_boot_rp::*;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex;

/// Total flash size on the RP2040 board (2 MiB).
const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    let flash = WatchdogFlash::<FLASH_SIZE>::start(
        p.FLASH,
        p.WATCHDOG,
        embassy_time::Duration::from_secs(8),
    );
    let flash = Mutex::<NoopRawMutex, _>::new(RefCell::new(flash));

    let config = BootLoaderConfig::from_linkerfile_blocking(&flash, &flash, &flash);
    let bl: BootLoader<4096> = BootLoader::prepare(config);

    // SAFETY: load() transfers control to the application entry point.
    // This is the only unsafe in the entire project.
    unsafe {
        bl.load(embassy_rp::flash::FLASH_BASE as u32);
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    cortex_m::asm::udf()
}
