//! Chip-specific peripheral initialization for STM32 targets.
//!
//! Each supported chip provides a [`UsbDriver`] type alias and an [`init`]
//! function that configures clocks, creates the USB OTG driver, and returns
//! an LED output pin. The correct variant is selected at compile time via
//! Cargo features (`stm32f401cc`, `stm32f411ce`, `stm32h743vi`).
//!
//! # Clock Configurations
//!
//! All variants assume a 25 MHz HSE crystal (standard on BlackPill and
//! Nucleo boards). The PLL is configured to produce a 48 MHz clock for
//! USB OTG alongside the maximum system clock for each chip.

#![forbid(unsafe_code)]

use embassy_stm32::gpio::{Level, Output, Speed};
use static_cell::StaticCell;

// ---------------------------------------------------------------------------
// STM32F4 family: USB OTG FS on PA11/PA12
// ---------------------------------------------------------------------------

#[cfg(any(feature = "stm32f401cc", feature = "stm32f411ce"))]
/// USB OTG FS driver type for STM32F4 chips.
pub type UsbDriver = embassy_stm32::usb::Driver<'static, embassy_stm32::peripherals::USB_OTG_FS>;

#[cfg(any(feature = "stm32f401cc", feature = "stm32f411ce"))]
embassy_stm32::bind_interrupts!(struct Irqs {
    OTG_FS => embassy_stm32::usb::InterruptHandler<embassy_stm32::peripherals::USB_OTG_FS>;
});

// ---------------------------------------------------------------------------
// STM32F401CC — Cortex-M4F, 84 MHz, 256 KB Flash, 64 KB SRAM
// ---------------------------------------------------------------------------

#[cfg(feature = "stm32f401cc")]
/// Initializes the STM32F401CC: 84 MHz from 25 MHz HSE, USB OTG FS, LED on PC13.
///
/// PLL: 25 MHz / 25 × 336 / 4 = 84 MHz SYSCLK, / 7 = 48 MHz USB.
pub fn init() -> (UsbDriver, Output<'static>) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: embassy_stm32::time::Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV25,
            mul: PllMul::MUL336,
            divp: Some(PllPDiv::DIV4),
            divq: Some(PllQDiv::DIV7),
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let p = embassy_stm32::init(config);

    static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);
    let driver =
        embassy_stm32::usb::Driver::new_fs(
            p.USB_OTG_FS,
            Irqs,
            p.PA12,
            p.PA11,
            ep_out_buffer,
            embassy_stm32::usb::Config::default(),
        );
    let led = Output::new(p.PC13, Level::Low, Speed::Low);

    (driver, led)
}

// ---------------------------------------------------------------------------
// STM32F411CE — Cortex-M4F, 96 MHz, 512 KB Flash, 128 KB SRAM
// ---------------------------------------------------------------------------

#[cfg(feature = "stm32f411ce")]
/// Initializes the STM32F411CE: 96 MHz from 25 MHz HSE, USB OTG FS, LED on PC13.
///
/// PLL: 25 MHz / 25 × 192 / 2 = 96 MHz SYSCLK, / 4 = 48 MHz USB.
/// Runs at 96 MHz instead of 100 MHz to produce a clean 48 MHz USB clock.
pub fn init() -> (UsbDriver, Output<'static>) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: embassy_stm32::time::Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV25,
            mul: PllMul::MUL192,
            divp: Some(PllPDiv::DIV2),
            divq: Some(PllQDiv::DIV4),
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let p = embassy_stm32::init(config);

    static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);
    let driver =
        embassy_stm32::usb::Driver::new_fs(
            p.USB_OTG_FS,
            Irqs,
            p.PA12,
            p.PA11,
            ep_out_buffer,
            embassy_stm32::usb::Config::default(),
        );
    let led = Output::new(p.PC13, Level::Low, Speed::Low);

    (driver, led)
}

// ---------------------------------------------------------------------------
// STM32H743VI — Cortex-M7, 480 MHz, 2 MB Flash, 1 MB SRAM
// ---------------------------------------------------------------------------

#[cfg(feature = "stm32h743vi")]
/// USB OTG FS driver type for STM32H7.
pub type UsbDriver = embassy_stm32::usb::Driver<'static, embassy_stm32::peripherals::USB_OTG_FS>;

#[cfg(feature = "stm32h743vi")]
embassy_stm32::bind_interrupts!(struct Irqs {
    OTG_FS => embassy_stm32::usb::InterruptHandler<embassy_stm32::peripherals::USB_OTG_FS>;
});

#[cfg(feature = "stm32h743vi")]
/// Initializes the STM32H743VI: 480 MHz from 25 MHz HSE, USB OTG FS, LED on PB0.
///
/// PLL1: 25 MHz / 5 × 192 / 2 = 480 MHz SYSCLK.
/// PLL3: 25 MHz / 5 × 48 / 5 = 48 MHz USB clock.
pub fn init() -> (UsbDriver, Output<'static>) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: embassy_stm32::time::Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL192,
            divp: Some(PllDiv::DIV2),
            divq: None,
            divr: None,
        });
        // PLL3 provides 48 MHz for USB
        config.rcc.pll3 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL48,
            divp: None,
            divq: Some(PllDiv::DIV5),
            divr: None,
        });
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let p = embassy_stm32::init(config);

    static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);
    let driver = embassy_stm32::usb::Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        ep_out_buffer,
        embassy_stm32::usb::Config::default(),
    );
    let led = Output::new(p.PB0, Level::Low, Speed::Low);

    (driver, led)
}
