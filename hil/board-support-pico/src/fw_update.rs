//! Board-specific DFU flash writer for RP2040.
//!
//! Wraps the embassy-boot-rp `BlockingFirmwareUpdater` behind the
//! [`DfuFlashWriter`](hil_firmware_support::fw_update::DfuFlashWriter) trait
//! so the generic firmware update protocol can write to the RP2040's
//! DFU flash partition.

use core::cell::RefCell;

use embassy_boot_rp::{AlignedBuffer, BlockingFirmwareUpdater, FirmwareUpdaterConfig};
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use hil_firmware_support::fw_update::DfuFlashWriter;

/// Total flash size on the RP2040 board (2 MiB).
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

/// Type alias for the statically allocated flash blocking mutex.
///
/// Uses `blocking_mutex::Mutex` (not the async one) because
/// `FirmwareUpdaterConfig::from_linkerfile_blocking` requires it.
pub type FlashMutex =
    BlockingMutex<NoopRawMutex, RefCell<Flash<'static, FLASH, Blocking, FLASH_SIZE>>>;

/// DFU flash writer for the RP2040, backed by embassy-boot-rp.
///
/// Holds a reference to the shared flash blocking mutex allocated in `main()`.
/// All operations use blocking flash access since the embassy-usb
/// `Handler` trait methods are synchronous.
pub struct PicoDfuWriter {
    flash: &'static FlashMutex,
}

impl PicoDfuWriter {
    /// Creates a new writer backed by the given flash mutex.
    pub fn new(flash: &'static FlashMutex) -> Self {
        Self { flash }
    }
}

impl DfuFlashWriter for PicoDfuWriter {
    fn erase_dfu(&mut self) -> Result<(), ()> {
        let mut aligned = AlignedBuffer([0u8; 4]);
        let config = FirmwareUpdaterConfig::from_linkerfile_blocking(self.flash, self.flash);
        let mut updater = BlockingFirmwareUpdater::new(config, aligned.as_mut());
        let _ = updater.prepare_update().map_err(|_| ())?;
        Ok(())
    }

    fn write_dfu(&mut self, offset: u32, data: &[u8]) -> Result<(), ()> {
        let mut aligned = AlignedBuffer([0u8; 4]);
        let config = FirmwareUpdaterConfig::from_linkerfile_blocking(self.flash, self.flash);
        let mut updater = BlockingFirmwareUpdater::new(config, aligned.as_mut());
        updater
            .write_firmware(offset as usize, data)
            .map_err(|_| ())
    }

    fn read_dfu(&mut self, _offset: u32, _buf: &mut [u8]) -> Result<(), ()> {
        // Reading from DFU partition is not directly supported by
        // BlockingFirmwareUpdater. CRC verification is done during write.
        Err(())
    }

    fn mark_updated(&mut self) -> Result<(), ()> {
        let mut aligned = AlignedBuffer([0u8; 4]);
        let config = FirmwareUpdaterConfig::from_linkerfile_blocking(self.flash, self.flash);
        let mut updater = BlockingFirmwareUpdater::new(config, aligned.as_mut());
        updater.mark_updated().map_err(|_| ())
    }

    fn mark_booted(&mut self) -> Result<(), ()> {
        let mut aligned = AlignedBuffer([0u8; 4]);
        let config = FirmwareUpdaterConfig::from_linkerfile_blocking(self.flash, self.flash);
        let mut updater = BlockingFirmwareUpdater::new(config, aligned.as_mut());
        updater.mark_booted().map_err(|_| ())
    }

    fn system_reset(&mut self) -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}

/// Marks the current firmware as booted at startup.
///
/// Called once from `main()` to prevent the bootloader from rolling
/// back to the previous firmware on the next power cycle.
pub fn mark_booted(flash: &'static FlashMutex) {
    let mut aligned = AlignedBuffer([0u8; 4]);
    let config = FirmwareUpdaterConfig::from_linkerfile_blocking(flash, flash);
    let mut updater = BlockingFirmwareUpdater::new(config, aligned.as_mut());
    if updater.mark_booted().is_err() {
        defmt::warn!("Failed to mark firmware as booted");
    } else {
        defmt::info!("Firmware marked as booted");
    }
}
