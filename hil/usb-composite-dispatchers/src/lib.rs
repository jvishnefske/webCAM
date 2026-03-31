#![no_std]

pub mod i2c_tiny_usb;

#[cfg(test)]
mod test_logger {
    #[defmt::global_logger]
    struct TestLogger;

    // SAFETY: Test-only no-op logger; single-threaded test runner.
    unsafe impl defmt::Logger for TestLogger {
        fn acquire() {}
        unsafe fn flush() {}
        unsafe fn release() {}
        unsafe fn write(_bytes: &[u8]) {}
    }

    defmt::timestamp!("{=u32}", 0);
}
