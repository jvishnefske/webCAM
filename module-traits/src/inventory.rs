//! MCU resource inventory — the "CubeMX" equivalent.
//!
//! [`McuDef`] captures a complete digital twin of a microcontroller:
//! CPU core, clock tree, memory map, peripheral instances with pin muxing,
//! DMA channels, and interrupt table.
//!
//! Pre-built definitions are provided via [`mcu_for`] for the supported
//! target families (RP2040, STM32F401, ESP32-C3, STM32G0B1, Host).

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level MCU definition
// ---------------------------------------------------------------------------

/// Complete MCU resource inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McuDef {
    /// Target family key (matches codegen `TargetFamily` variants).
    pub family: String,
    /// Manufacturer part number (e.g., "STM32F401CCU6").
    pub part_number: String,
    /// Human-readable name.
    pub display_name: String,
    /// CPU core type.
    pub core: CpuCore,
    /// Clock tree configuration.
    pub clock: ClockTree,
    /// Memory regions (flash, RAM, boot).
    pub memory: Vec<MemoryRegion>,
    /// All peripheral instances on the chip.
    pub peripherals: Vec<PeripheralInst>,
    /// GPIO pin definitions with alternate-function mux.
    pub pins: Vec<PinDef>,
    /// DMA channel / stream definitions.
    pub dma_channels: Vec<DmaChannel>,
    /// Interrupt table.
    pub interrupts: Vec<InterruptDef>,
}

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuCore {
    CortexM0Plus,
    CortexM4,
    CortexM4F,
    CortexM7,
    RiscV32IMC,
    /// Simulated — not a real core.
    HostSim,
}

// ---------------------------------------------------------------------------
// Clock tree
// ---------------------------------------------------------------------------

/// Clock tree configuration for RCC / PLL setup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockTree {
    /// Internal RC oscillator frequency (MHz).
    pub hsi_mhz: f32,
    /// External crystal frequency (MHz), if present.
    pub hse_mhz: Option<f32>,
    /// PLL configuration.
    pub pll: Option<PllConfig>,
    /// Resulting system clock (MHz).
    pub sys_clk_mhz: f32,
    /// AHB bus divider (1, 2, 4, …).
    pub ahb_div: u16,
    /// APB1 (low-speed) bus divider.
    pub apb1_div: u16,
    /// APB2 (high-speed) bus divider — not all chips have this.
    pub apb2_div: Option<u16>,
}

/// PLL configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PllConfig {
    pub source: ClockSource,
    /// Input divider (M).
    pub m: u16,
    /// VCO multiplier (N).
    pub n: u16,
    /// System clock divider (P).
    pub p: u16,
    /// USB / SDIO divider (Q) — optional.
    pub q: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockSource {
    Hsi,
    Hse,
    /// Internal 48 MHz oscillator (RP2040, ESP32-C3, etc.).
    InternalPll,
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// A contiguous memory region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRegion {
    pub name: String,
    pub kind: MemoryKind,
    /// Base address.
    pub start: u32,
    /// Size in bytes.
    pub size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryKind {
    Flash,
    Ram,
    Bootloader,
    PeripheralRam,
}

// ---------------------------------------------------------------------------
// Peripheral instances
// ---------------------------------------------------------------------------

/// A peripheral instance on the MCU (e.g., USART1, SPI2, TIM3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeripheralInst {
    /// Instance name (e.g., "USART1", "SPI2", "TIM1").
    pub name: String,
    /// Peripheral type.
    pub kind: PeripheralKind,
    /// Bus this peripheral sits on.
    pub bus: BusConnection,
    /// Primary IRQ name (matches Embassy IRQ binding).
    pub irq: Option<String>,
    /// DMA request number for this peripheral (if DMA-capable).
    pub dma_request: Option<u8>,
    /// Available signal↔pin mappings for this peripheral.
    pub signals: Vec<SignalPin>,
}

/// Peripheral type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeripheralKind {
    Uart,
    Spi,
    I2c,
    Adc,
    Dac,
    Timer,
    Can,
    Usb,
    Pio,
    Ledc,
    Gpio,
}

/// Bus that a peripheral is connected to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BusConnection {
    Ahb,
    Apb1,
    Apb2,
    Ioport,
    /// RP2040 / ESP32 don't have traditional bus hierarchy.
    SysBus,
}

/// A signal↔pin option for a peripheral instance.
///
/// For example, USART1 TX can be mapped to PA9 (AF7) or PB6 (AF7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalPin {
    /// Signal name (e.g., "TX", "RX", "SCK", "MOSI", "CH1").
    pub signal: String,
    /// Pin name (e.g., "PA9").
    pub pin: String,
    /// Alternate function number (STM32: AF0–AF15), or `None` for
    /// chips where the mux works differently (RP2040, ESP32).
    pub af: Option<u8>,
}

// ---------------------------------------------------------------------------
// GPIO pin definitions
// ---------------------------------------------------------------------------

/// A GPIO pin with its alternate-function multiplexer options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinDef {
    /// Pin name (e.g., "PA0", "GP0", "GPIO0").
    pub name: String,
    /// Port letter (A–D) or group index.
    pub port: String,
    /// Pin number within the port.
    pub number: u8,
    /// Alternate-function entries available on this pin.
    pub alt_functions: Vec<AltFunction>,
    /// ADC channel this pin connects to (if any).
    pub adc_channel: Option<u8>,
    /// Whether this pin is 5V-tolerant.
    pub five_v_tolerant: bool,
}

/// One alternate-function mapping on a pin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltFunction {
    /// AF number (0–15 for STM32, functional for others).
    pub af: u8,
    /// Peripheral instance (e.g., "USART1").
    pub peripheral: String,
    /// Signal within the peripheral (e.g., "TX").
    pub signal: String,
}

// ---------------------------------------------------------------------------
// DMA
// ---------------------------------------------------------------------------

/// A DMA channel or stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmaChannel {
    /// Controller (e.g., "DMA1", "DMA2").
    pub controller: String,
    /// Stream or channel number.
    pub channel: u8,
    /// Which peripheral this channel can service.
    pub peripheral: String,
    /// Transfer direction.
    pub direction: DmaDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DmaDirection {
    PeripheralToMemory,
    MemoryToPeripheral,
    MemoryToMemory,
}

// ---------------------------------------------------------------------------
// Interrupts
// ---------------------------------------------------------------------------

/// An interrupt vector entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptDef {
    /// IRQ name (e.g., "USART1", "TIM2", "EXTI0").
    pub name: String,
    /// Vector number.
    pub irq_number: u16,
    /// Default priority (lower = higher priority).
    pub default_priority: u8,
}

// ---------------------------------------------------------------------------
// Pre-built MCU definitions
// ---------------------------------------------------------------------------

/// Look up a pre-built MCU definition by target family name.
pub fn mcu_for(family: &str) -> Option<McuDef> {
    match family {
        "Host" => Some(host_mcu()),
        "Rp2040" => Some(rp2040_mcu()),
        "Stm32f4" => Some(stm32f401_mcu()),
        "Esp32c3" => Some(esp32c3_mcu()),
        "Stm32g0b1" => Some(stm32g0b1_mcu()),
        _ => None,
    }
}

/// All supported MCU family names.
pub fn supported_families() -> Vec<&'static str> {
    alloc::vec!["Host", "Rp2040", "Stm32f4", "Esp32c3", "Stm32g0b1"]
}

// -- Host (simulation) ------------------------------------------------------

fn host_mcu() -> McuDef {
    McuDef {
        family: s("Host"),
        part_number: s("host-sim"),
        display_name: s("Host Simulation"),
        core: CpuCore::HostSim,
        clock: ClockTree {
            hsi_mhz: 0.0,
            hse_mhz: None,
            pll: None,
            sys_clk_mhz: 0.0,
            ahb_div: 1,
            apb1_div: 1,
            apb2_div: None,
        },
        memory: alloc::vec![],
        peripherals: alloc::vec![
            periph("ADC", PeripheralKind::Adc, BusConnection::SysBus, None, &[]),
            periph("PWM", PeripheralKind::Timer, BusConnection::SysBus, None, &[]),
        ],
        pins: (0..32)
            .map(|i| PinDef {
                name: alloc::format!("SIM{i}"),
                port: s("SIM"),
                number: i,
                alt_functions: alloc::vec![],
                adc_channel: if i < 16 { Some(i) } else { None },
                five_v_tolerant: false,
            })
            .collect(),
        dma_channels: alloc::vec![],
        interrupts: alloc::vec![],
    }
}

// -- RP2040 -----------------------------------------------------------------

fn rp2040_mcu() -> McuDef {
    let mut peripherals = Vec::new();
    let mut pins: Vec<PinDef> = Vec::new();

    // UART0
    peripherals.push(periph("UART0", PeripheralKind::Uart, BusConnection::SysBus,
        Some("UART0_IRQ"), &[
            ("TX", "GP0", 2), ("RX", "GP1", 2),
            ("TX", "GP12", 2), ("RX", "GP13", 2),
            ("TX", "GP16", 2), ("RX", "GP17", 2),
        ]));
    // UART1
    peripherals.push(periph("UART1", PeripheralKind::Uart, BusConnection::SysBus,
        Some("UART1_IRQ"), &[
            ("TX", "GP4", 2), ("RX", "GP5", 2),
            ("TX", "GP8", 2), ("RX", "GP9", 2),
        ]));
    // SPI0
    peripherals.push(periph("SPI0", PeripheralKind::Spi, BusConnection::SysBus,
        Some("SPI0_IRQ"), &[
            ("SCK", "GP2", 1), ("MOSI", "GP3", 1), ("MISO", "GP4", 1), ("CS", "GP5", 1),
            ("SCK", "GP6", 1), ("MOSI", "GP7", 1), ("MISO", "GP0", 1), ("CS", "GP1", 1),
            ("SCK", "GP18", 1), ("MOSI", "GP19", 1), ("MISO", "GP16", 1), ("CS", "GP17", 1),
        ]));
    // SPI1
    peripherals.push(periph("SPI1", PeripheralKind::Spi, BusConnection::SysBus,
        Some("SPI1_IRQ"), &[
            ("SCK", "GP10", 1), ("MOSI", "GP11", 1), ("MISO", "GP12", 1), ("CS", "GP13", 1),
            ("SCK", "GP14", 1), ("MOSI", "GP15", 1), ("MISO", "GP8", 1), ("CS", "GP9", 1),
        ]));
    // I2C0
    peripherals.push(periph("I2C0", PeripheralKind::I2c, BusConnection::SysBus,
        Some("I2C0_IRQ"), &[
            ("SDA", "GP0", 3), ("SCL", "GP1", 3),
            ("SDA", "GP4", 3), ("SCL", "GP5", 3),
            ("SDA", "GP8", 3), ("SCL", "GP9", 3),
            ("SDA", "GP12", 3), ("SCL", "GP13", 3),
            ("SDA", "GP16", 3), ("SCL", "GP17", 3),
            ("SDA", "GP20", 3), ("SCL", "GP21", 3),
        ]));
    // I2C1
    peripherals.push(periph("I2C1", PeripheralKind::I2c, BusConnection::SysBus,
        Some("I2C1_IRQ"), &[
            ("SDA", "GP2", 3), ("SCL", "GP3", 3),
            ("SDA", "GP6", 3), ("SCL", "GP7", 3),
            ("SDA", "GP10", 3), ("SCL", "GP11", 3),
            ("SDA", "GP14", 3), ("SCL", "GP15", 3),
            ("SDA", "GP18", 3), ("SCL", "GP19", 3),
            ("SDA", "GP26", 3), ("SCL", "GP27", 3),
        ]));
    // ADC
    peripherals.push(periph("ADC", PeripheralKind::Adc, BusConnection::SysBus,
        Some("ADC_IRQ_FIFO"), &[
            ("CH0", "GP26", 0), ("CH1", "GP27", 0),
            ("CH2", "GP28", 0), ("CH3", "GP29", 0),
        ]));
    // PWM slices 0–7
    for slice in 0..8u8 {
        let a_pin = slice * 2;
        let b_pin = slice * 2 + 1;
        peripherals.push(periph(
            &alloc::format!("PWM_SLICE{slice}"),
            PeripheralKind::Timer,
            BusConnection::SysBus,
            Some("PWM_IRQ_WRAP"),
            &[
                ("A", &alloc::format!("GP{a_pin}"), 4),
                ("B", &alloc::format!("GP{b_pin}"), 4),
            ],
        ));
    }
    // PIO
    peripherals.push(periph("PIO0", PeripheralKind::Pio, BusConnection::SysBus,
        Some("PIO0_IRQ_0"), &[]));
    peripherals.push(periph("PIO1", PeripheralKind::Pio, BusConnection::SysBus,
        Some("PIO1_IRQ_0"), &[]));
    // USB
    peripherals.push(periph("USB", PeripheralKind::Usb, BusConnection::SysBus,
        Some("USBCTRL_IRQ"), &[]));

    // GPIO pins GP0–GP29
    for i in 0u8..30 {
        let adc = match i {
            26 => Some(0),
            27 => Some(1),
            28 => Some(2),
            29 => Some(3),
            _ => None,
        };
        // Collect AF entries for this pin from peripherals
        let mut afs = Vec::new();
        for p in &peripherals {
            for sp in &p.signals {
                if sp.pin == alloc::format!("GP{i}") {
                    if let Some(af) = sp.af {
                        afs.push(AltFunction {
                            af,
                            peripheral: p.name.clone(),
                            signal: sp.signal.clone(),
                        });
                    }
                }
            }
        }
        pins.push(PinDef {
            name: alloc::format!("GP{i}"),
            port: s("GPIO"),
            number: i,
            alt_functions: afs,
            adc_channel: adc,
            five_v_tolerant: false,
        });
    }

    McuDef {
        family: s("Rp2040"),
        part_number: s("RP2040"),
        display_name: s("Raspberry Pi Pico (RP2040)"),
        core: CpuCore::CortexM0Plus,
        clock: ClockTree {
            hsi_mhz: 12.0,
            hse_mhz: Some(12.0),
            pll: Some(PllConfig {
                source: ClockSource::Hse,
                m: 1, n: 133, p: 1, q: None,
            }),
            sys_clk_mhz: 133.0,
            ahb_div: 1,
            apb1_div: 1,
            apb2_div: None,
        },
        memory: alloc::vec![
            MemoryRegion { name: s("BOOT2"), kind: MemoryKind::Bootloader, start: 0x1000_0000, size: 256 },
            MemoryRegion { name: s("FLASH"), kind: MemoryKind::Flash, start: 0x1000_0100, size: 2 * 1024 * 1024 - 256 },
            MemoryRegion { name: s("RAM"), kind: MemoryKind::Ram, start: 0x2000_0000, size: 264 * 1024 },
        ],
        peripherals,
        pins,
        dma_channels: (0..12).map(|i| DmaChannel {
            controller: s("DMA"),
            channel: i,
            peripheral: s("*"),
            direction: DmaDirection::PeripheralToMemory,
        }).collect(),
        interrupts: alloc::vec![
            irq("UART0_IRQ", 20, 3),
            irq("UART1_IRQ", 21, 3),
            irq("SPI0_IRQ", 18, 3),
            irq("SPI1_IRQ", 19, 3),
            irq("I2C0_IRQ", 23, 3),
            irq("I2C1_IRQ", 24, 3),
            irq("ADC_IRQ_FIFO", 22, 3),
            irq("PWM_IRQ_WRAP", 4, 3),
            irq("PIO0_IRQ_0", 7, 3),
            irq("PIO1_IRQ_0", 9, 3),
            irq("USBCTRL_IRQ", 5, 3),
        ],
    }
}

// -- STM32F401 --------------------------------------------------------------

#[allow(clippy::vec_init_then_push)]
fn stm32f401_mcu() -> McuDef {
    let mut peripherals = Vec::new();

    // USART1
    peripherals.push(periph("USART1", PeripheralKind::Uart, BusConnection::Apb2,
        Some("USART1"), &[
            ("TX", "PA9", 7), ("RX", "PA10", 7),
            ("TX", "PB6", 7), ("RX", "PB7", 7),
        ]));
    // USART2
    peripherals.push(periph("USART2", PeripheralKind::Uart, BusConnection::Apb1,
        Some("USART2"), &[
            ("TX", "PA2", 7), ("RX", "PA3", 7),
        ]));
    // USART6
    peripherals.push(periph("USART6", PeripheralKind::Uart, BusConnection::Apb2,
        Some("USART6"), &[
            ("TX", "PA11", 8), ("RX", "PA12", 8),
        ]));
    // SPI1
    peripherals.push(periph("SPI1", PeripheralKind::Spi, BusConnection::Apb2,
        Some("SPI1"), &[
            ("SCK", "PA5", 5), ("MOSI", "PA7", 5), ("MISO", "PA6", 5),
            ("SCK", "PB3", 5), ("MOSI", "PB5", 5), ("MISO", "PB4", 5),
        ]));
    // SPI2
    peripherals.push(periph("SPI2", PeripheralKind::Spi, BusConnection::Apb1,
        Some("SPI2"), &[
            ("SCK", "PB13", 5), ("MOSI", "PB15", 5), ("MISO", "PB14", 5),
        ]));
    // SPI3
    peripherals.push(periph("SPI3", PeripheralKind::Spi, BusConnection::Apb1,
        Some("SPI3"), &[
            ("SCK", "PB3", 6), ("MOSI", "PB5", 6), ("MISO", "PB4", 6),
            ("SCK", "PC10", 6), ("MOSI", "PC12", 6), ("MISO", "PC11", 6),
        ]));
    // I2C1
    peripherals.push(periph("I2C1", PeripheralKind::I2c, BusConnection::Apb1,
        Some("I2C1_EV"), &[
            ("SDA", "PB7", 4), ("SCL", "PB6", 4),
            ("SDA", "PB9", 4), ("SCL", "PB8", 4),
        ]));
    // I2C2
    peripherals.push(periph("I2C2", PeripheralKind::I2c, BusConnection::Apb1,
        Some("I2C2_EV"), &[
            ("SDA", "PB3", 9), ("SCL", "PB10", 4),
        ]));
    // I2C3
    peripherals.push(periph("I2C3", PeripheralKind::I2c, BusConnection::Apb1,
        Some("I2C3_EV"), &[
            ("SDA", "PB4", 9), ("SCL", "PA8", 4),
        ]));
    // ADC1
    peripherals.push(periph("ADC1", PeripheralKind::Adc, BusConnection::Apb2,
        Some("ADC"), &[
            ("IN0", "PA0", 0), ("IN1", "PA1", 0),
            ("IN4", "PA4", 0), ("IN5", "PA5", 0),
            ("IN6", "PA6", 0), ("IN7", "PA7", 0),
            ("IN8", "PB0", 0), ("IN9", "PB1", 0),
        ]));
    // TIM1 (advanced)
    peripherals.push(periph("TIM1", PeripheralKind::Timer, BusConnection::Apb2,
        Some("TIM1_UP_TIM10"), &[
            ("CH1", "PA8", 1), ("CH2", "PA9", 1),
            ("CH3", "PA10", 1), ("CH4", "PA11", 1),
        ]));
    // TIM2
    peripherals.push(periph("TIM2", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM2"), &[
            ("CH1", "PA0", 1), ("CH2", "PA1", 1),
            ("CH3", "PA2", 1), ("CH4", "PA3", 1),
        ]));
    // TIM3
    peripherals.push(periph("TIM3", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM3"), &[
            ("CH1", "PA6", 2), ("CH2", "PA7", 2),
            ("CH3", "PB0", 2), ("CH4", "PB1", 2),
        ]));
    // TIM4
    peripherals.push(periph("TIM4", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM4"), &[
            ("CH1", "PB6", 2), ("CH2", "PB7", 2),
            ("CH3", "PB8", 2), ("CH4", "PB9", 2),
        ]));
    // TIM5
    peripherals.push(periph("TIM5", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM5"), &[
            ("CH1", "PA0", 2), ("CH2", "PA1", 2),
            ("CH3", "PA2", 2), ("CH4", "PA3", 2),
        ]));
    // USB OTG FS
    peripherals.push(periph("USB_OTG_FS", PeripheralKind::Usb, BusConnection::Ahb,
        Some("OTG_FS"), &[
            ("DM", "PA11", 10), ("DP", "PA12", 10),
        ]));

    // GPIO pins
    let mut pins = Vec::new();
    for (port, count) in [('A', 16u8), ('B', 16), ('C', 14)] {
        for n in 0..count {
            let name = alloc::format!("P{port}{n}");
            let adc_ch = match (port, n) {
                ('A', 0) => Some(0), ('A', 1) => Some(1),
                ('A', 4) => Some(4), ('A', 5) => Some(5),
                ('A', 6) => Some(6), ('A', 7) => Some(7),
                ('B', 0) => Some(8), ('B', 1) => Some(9),
                _ => None,
            };
            let mut afs = Vec::new();
            for p in &peripherals {
                for sp in &p.signals {
                    if sp.pin == name {
                        if let Some(af) = sp.af {
                            afs.push(AltFunction {
                                af,
                                peripheral: p.name.clone(),
                                signal: sp.signal.clone(),
                            });
                        }
                    }
                }
            }
            pins.push(PinDef {
                name,
                port: alloc::format!("{port}"),
                number: n,
                alt_functions: afs,
                adc_channel: adc_ch,
                five_v_tolerant: port == 'B',
            });
        }
    }

    McuDef {
        family: s("Stm32f4"),
        part_number: s("STM32F401CCU6"),
        display_name: s("STM32F401CC (Cortex-M4)"),
        core: CpuCore::CortexM4F,
        clock: ClockTree {
            hsi_mhz: 16.0,
            hse_mhz: Some(8.0),
            pll: Some(PllConfig {
                source: ClockSource::Hse,
                m: 4, n: 168, p: 4, q: Some(7),
            }),
            sys_clk_mhz: 84.0,
            ahb_div: 1,
            apb1_div: 2,
            apb2_div: Some(1),
        },
        memory: alloc::vec![
            MemoryRegion { name: s("FLASH"), kind: MemoryKind::Flash, start: 0x0800_0000, size: 256 * 1024 },
            MemoryRegion { name: s("RAM"), kind: MemoryKind::Ram, start: 0x2000_0000, size: 64 * 1024 },
        ],
        peripherals,
        pins,
        dma_channels: alloc::vec![
            dma("DMA2", 0, "ADC1", DmaDirection::PeripheralToMemory),
            dma("DMA2", 2, "SPI1_RX", DmaDirection::PeripheralToMemory),
            dma("DMA2", 3, "SPI1_TX", DmaDirection::MemoryToPeripheral),
            dma("DMA1", 5, "USART2_RX", DmaDirection::PeripheralToMemory),
            dma("DMA1", 6, "USART2_TX", DmaDirection::MemoryToPeripheral),
            dma("DMA2", 5, "USART1_RX", DmaDirection::PeripheralToMemory),
            dma("DMA2", 7, "USART1_TX", DmaDirection::MemoryToPeripheral),
        ],
        interrupts: alloc::vec![
            irq("USART1", 37, 5), irq("USART2", 38, 5), irq("USART6", 71, 5),
            irq("SPI1", 35, 5), irq("SPI2", 36, 5), irq("SPI3", 51, 5),
            irq("I2C1_EV", 31, 5), irq("I2C2_EV", 33, 5), irq("I2C3_EV", 72, 5),
            irq("ADC", 18, 5),
            irq("TIM1_UP_TIM10", 25, 5), irq("TIM2", 28, 5),
            irq("TIM3", 29, 5), irq("TIM4", 30, 5), irq("TIM5", 50, 5),
            irq("OTG_FS", 67, 5),
            irq("DMA1_STREAM0", 11, 5), irq("DMA2_STREAM0", 56, 5),
        ],
    }
}

// -- ESP32-C3 ---------------------------------------------------------------

#[allow(clippy::vec_init_then_push)]
fn esp32c3_mcu() -> McuDef {
    let mut peripherals = Vec::new();

    peripherals.push(periph("UART0", PeripheralKind::Uart, BusConnection::SysBus,
        Some("UART0"), &[
            ("TX", "GPIO21", 0), ("RX", "GPIO20", 0),
        ]));
    peripherals.push(periph("UART1", PeripheralKind::Uart, BusConnection::SysBus,
        Some("UART1"), &[
            ("TX", "GPIO0", 0), ("TX", "GPIO1", 0),
            ("RX", "GPIO2", 0), ("RX", "GPIO3", 0),
        ]));
    peripherals.push(periph("SPI2", PeripheralKind::Spi, BusConnection::SysBus,
        Some("SPI2"), &[
            ("SCK", "GPIO6", 0), ("MOSI", "GPIO7", 0), ("MISO", "GPIO2", 0), ("CS", "GPIO10", 0),
        ]));
    peripherals.push(periph("I2C0", PeripheralKind::I2c, BusConnection::SysBus,
        Some("I2C_EXT0"), &[
            ("SDA", "GPIO1", 0), ("SCL", "GPIO0", 0),
            ("SDA", "GPIO3", 0), ("SCL", "GPIO2", 0),
            ("SDA", "GPIO5", 0), ("SCL", "GPIO4", 0),
        ]));
    peripherals.push(periph("ADC1", PeripheralKind::Adc, BusConnection::SysBus,
        None, &[
            ("CH0", "GPIO0", 0), ("CH1", "GPIO1", 0),
            ("CH2", "GPIO2", 0), ("CH3", "GPIO3", 0),
            ("CH4", "GPIO4", 0),
        ]));
    peripherals.push(periph("LEDC", PeripheralKind::Ledc, BusConnection::SysBus,
        None, &[
            ("CH0", "GPIO0", 0), ("CH1", "GPIO1", 0),
            ("CH2", "GPIO2", 0), ("CH3", "GPIO3", 0),
            ("CH4", "GPIO4", 0), ("CH5", "GPIO5", 0),
        ]));
    peripherals.push(periph("USB_SERIAL_JTAG", PeripheralKind::Usb, BusConnection::SysBus,
        Some("USB_SERIAL_JTAG"), &[
            ("DM", "GPIO18", 0), ("DP", "GPIO19", 0),
        ]));

    let mut pins = Vec::new();
    for i in 0u8..22 {
        let adc = if i < 5 { Some(i) } else { None };
        let mut afs = Vec::new();
        for p in &peripherals {
            for sp in &p.signals {
                if sp.pin == alloc::format!("GPIO{i}") {
                    afs.push(AltFunction {
                        af: sp.af.unwrap_or(0),
                        peripheral: p.name.clone(),
                        signal: sp.signal.clone(),
                    });
                }
            }
        }
        pins.push(PinDef {
            name: alloc::format!("GPIO{i}"),
            port: s("GPIO"),
            number: i,
            alt_functions: afs,
            adc_channel: adc,
            five_v_tolerant: false,
        });
    }

    McuDef {
        family: s("Esp32c3"),
        part_number: s("ESP32-C3"),
        display_name: s("ESP32-C3 (RISC-V)"),
        core: CpuCore::RiscV32IMC,
        clock: ClockTree {
            hsi_mhz: 17.5,
            hse_mhz: Some(40.0),
            pll: Some(PllConfig {
                source: ClockSource::Hse,
                m: 1, n: 4, p: 1, q: None,
            }),
            sys_clk_mhz: 160.0,
            ahb_div: 1,
            apb1_div: 1,
            apb2_div: None,
        },
        memory: alloc::vec![
            MemoryRegion { name: s("IROM"), kind: MemoryKind::Flash, start: 0x4200_0000, size: 4 * 1024 * 1024 },
            MemoryRegion { name: s("IRAM"), kind: MemoryKind::Ram, start: 0x4037_C000, size: 400 * 1024 },
            MemoryRegion { name: s("DRAM"), kind: MemoryKind::Ram, start: 0x3FC8_0000, size: 400 * 1024 },
        ],
        peripherals,
        pins,
        dma_channels: (0..3).map(|i| DmaChannel {
            controller: s("GDMA"),
            channel: i,
            peripheral: s("*"),
            direction: DmaDirection::PeripheralToMemory,
        }).collect(),
        interrupts: alloc::vec![
            irq("UART0", 21, 1), irq("UART1", 22, 1),
            irq("SPI2", 28, 1), irq("I2C_EXT0", 24, 1),
            irq("USB_SERIAL_JTAG", 32, 1),
        ],
    }
}

// -- STM32G0B1 --------------------------------------------------------------

#[allow(clippy::vec_init_then_push)]
fn stm32g0b1_mcu() -> McuDef {
    let mut peripherals = Vec::new();

    // USARTs
    peripherals.push(periph("USART1", PeripheralKind::Uart, BusConnection::Apb1,
        Some("USART1"), &[
            ("TX", "PA9", 1), ("RX", "PA10", 1),
            ("TX", "PB6", 0), ("RX", "PB7", 0),
        ]));
    peripherals.push(periph("USART2", PeripheralKind::Uart, BusConnection::Apb1,
        Some("USART2"), &[
            ("TX", "PA2", 1), ("RX", "PA3", 1),
            ("TX", "PA14", 1), ("RX", "PA15", 1),
        ]));
    peripherals.push(periph("USART3", PeripheralKind::Uart, BusConnection::Apb1,
        Some("USART3_4_5_6_LPUART1"), &[
            ("TX", "PB8", 4), ("RX", "PB9", 4),
            ("TX", "PB10", 4), ("RX", "PB11", 4),
            ("TX", "PC4", 1), ("RX", "PC5", 1),
        ]));
    peripherals.push(periph("USART4", PeripheralKind::Uart, BusConnection::Apb1,
        Some("USART3_4_5_6_LPUART1"), &[
            ("TX", "PA0", 4), ("RX", "PA1", 4),
            ("TX", "PC10", 1), ("RX", "PC11", 1),
        ]));
    peripherals.push(periph("LPUART1", PeripheralKind::Uart, BusConnection::Apb1,
        Some("USART3_4_5_6_LPUART1"), &[
            ("TX", "PB11", 1), ("RX", "PB10", 1),
        ]));
    // SPI1
    peripherals.push(periph("SPI1", PeripheralKind::Spi, BusConnection::Apb1,
        Some("SPI1"), &[
            ("SCK", "PA1", 0), ("MOSI", "PA2", 0), ("MISO", "PA6", 0),
            ("SCK", "PA5", 0), ("MOSI", "PA7", 0), ("MISO", "PA11", 0),
            ("SCK", "PB3", 0), ("MOSI", "PB5", 0), ("MISO", "PB4", 0),
        ]));
    // SPI2
    peripherals.push(periph("SPI2", PeripheralKind::Spi, BusConnection::Apb1,
        Some("SPI2_3"), &[
            ("SCK", "PB13", 0), ("MOSI", "PB15", 0), ("MISO", "PB14", 0),
            ("SCK", "PB8", 1), ("MOSI", "PB11", 0), ("MISO", "PB2", 1),
        ]));
    // I2C1
    peripherals.push(periph("I2C1", PeripheralKind::I2c, BusConnection::Apb1,
        Some("I2C1"), &[
            ("SDA", "PB7", 6), ("SCL", "PB6", 6),
            ("SDA", "PB9", 6), ("SCL", "PB8", 6),
        ]));
    // I2C2
    peripherals.push(periph("I2C2", PeripheralKind::I2c, BusConnection::Apb1,
        Some("I2C2_3"), &[
            ("SDA", "PA12", 6), ("SCL", "PA11", 6),
            ("SDA", "PB11", 6), ("SCL", "PB10", 6),
            ("SDA", "PB14", 6), ("SCL", "PB13", 6),
        ]));
    // ADC1
    peripherals.push(periph("ADC1", PeripheralKind::Adc, BusConnection::Apb1,
        Some("ADC1_COMP"), &[
            ("IN0", "PA0", 0), ("IN1", "PA1", 0), ("IN2", "PA2", 0), ("IN3", "PA3", 0),
            ("IN4", "PA4", 0), ("IN5", "PA5", 0), ("IN6", "PA6", 0), ("IN7", "PA7", 0),
            ("IN8", "PB0", 0), ("IN9", "PB1", 0),
        ]));
    // Timers
    peripherals.push(periph("TIM1", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM1_BRK_UP_TRG_COM"), &[
            ("CH1", "PA8", 2), ("CH2", "PA9", 2),
            ("CH3", "PA10", 2), ("CH4", "PA11", 2),
        ]));
    peripherals.push(periph("TIM2", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM2"), &[
            ("CH1", "PA0", 2), ("CH2", "PA1", 2),
            ("CH3", "PA2", 10), ("CH4", "PA3", 10),
        ]));
    peripherals.push(periph("TIM3", PeripheralKind::Timer, BusConnection::Apb1,
        Some("TIM3_TIM4"), &[
            ("CH1", "PA6", 1), ("CH2", "PA7", 1),
            ("CH3", "PB0", 1), ("CH4", "PB1", 1),
        ]));
    // FDCAN
    peripherals.push(periph("FDCAN1", PeripheralKind::Can, BusConnection::Apb1,
        Some("TIM16_FDCAN_IT0"), &[
            ("TX", "PA12", 3), ("RX", "PA11", 3),
            ("TX", "PB9", 3), ("RX", "PB8", 3),
            ("TX", "PD1", 3), ("RX", "PD0", 3),
        ]));
    peripherals.push(periph("FDCAN2", PeripheralKind::Can, BusConnection::Apb1,
        Some("TIM17_FDCAN_IT1"), &[
            ("TX", "PB6", 3), ("RX", "PB5", 3),
            ("TX", "PB13", 3), ("RX", "PB12", 3),
        ]));
    // USB
    peripherals.push(periph("USB", PeripheralKind::Usb, BusConnection::Apb1,
        Some("USB_UCPD1_2"), &[
            ("DM", "PA11", 0), ("DP", "PA12", 0),
        ]));

    // GPIO pins
    let mut pins = Vec::new();
    for (port, count) in [('A', 16u8), ('B', 16), ('C', 16), ('D', 16)] {
        for n in 0..count {
            let name = alloc::format!("P{port}{n}");
            let adc_ch = match (port, n) {
                ('A', ch @ 0..=7) => Some(ch),
                ('B', ch @ 0..=1) => Some(8 + ch),
                _ => None,
            };
            let mut afs = Vec::new();
            for p in &peripherals {
                for sp in &p.signals {
                    if sp.pin == name {
                        if let Some(af) = sp.af {
                            afs.push(AltFunction {
                                af,
                                peripheral: p.name.clone(),
                                signal: sp.signal.clone(),
                            });
                        }
                    }
                }
            }
            pins.push(PinDef {
                name,
                port: alloc::format!("{port}"),
                number: n,
                alt_functions: afs,
                adc_channel: adc_ch,
                five_v_tolerant: port == 'B' || port == 'C',
            });
        }
    }

    McuDef {
        family: s("Stm32g0b1"),
        part_number: s("STM32G0B1CBT6"),
        display_name: s("STM32G0B1CB (Cortex-M0+)"),
        core: CpuCore::CortexM0Plus,
        clock: ClockTree {
            hsi_mhz: 16.0,
            hse_mhz: Some(8.0),
            pll: Some(PllConfig {
                source: ClockSource::Hse,
                m: 1, n: 16, p: 2, q: Some(2),
            }),
            sys_clk_mhz: 64.0,
            ahb_div: 1,
            apb1_div: 1,
            apb2_div: None,
        },
        memory: alloc::vec![
            MemoryRegion { name: s("FLASH"), kind: MemoryKind::Flash, start: 0x0800_0000, size: 512 * 1024 },
            MemoryRegion { name: s("RAM"), kind: MemoryKind::Ram, start: 0x2000_0000, size: 144 * 1024 },
        ],
        peripherals,
        pins,
        dma_channels: alloc::vec![
            dma("DMA1", 0, "ADC1", DmaDirection::PeripheralToMemory),
            dma("DMA1", 1, "SPI1_RX", DmaDirection::PeripheralToMemory),
            dma("DMA1", 2, "SPI1_TX", DmaDirection::MemoryToPeripheral),
            dma("DMA1", 3, "USART1_RX", DmaDirection::PeripheralToMemory),
            dma("DMA1", 4, "USART1_TX", DmaDirection::MemoryToPeripheral),
            dma("DMA1", 5, "USART2_RX", DmaDirection::PeripheralToMemory),
            dma("DMA1", 6, "USART2_TX", DmaDirection::MemoryToPeripheral),
        ],
        interrupts: alloc::vec![
            irq("USART1", 27, 3), irq("USART2", 28, 3),
            irq("USART3_4_5_6_LPUART1", 29, 3),
            irq("SPI1", 25, 3), irq("SPI2_3", 26, 3),
            irq("I2C1", 23, 3), irq("I2C2_3", 24, 3),
            irq("ADC1_COMP", 12, 3),
            irq("TIM1_BRK_UP_TRG_COM", 13, 3), irq("TIM2", 15, 3), irq("TIM3_TIM4", 16, 3),
            irq("TIM16_FDCAN_IT0", 21, 3), irq("TIM17_FDCAN_IT1", 22, 3),
            irq("USB_UCPD1_2", 8, 3),
            irq("DMA1_CHANNEL1", 9, 3), irq("DMA1_CH4_7_DMA2_CH1_5_DMAMUX_OVR", 10, 3),
        ],
    }
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

fn s(v: &str) -> String {
    String::from(v)
}

fn periph(
    name: &str,
    kind: PeripheralKind,
    bus: BusConnection,
    irq_name: Option<&str>,
    signal_pins: &[(&str, &str, u8)],
) -> PeripheralInst {
    PeripheralInst {
        name: s(name),
        kind,
        bus,
        irq: irq_name.map(s),
        dma_request: None,
        signals: signal_pins
            .iter()
            .map(|(sig, pin, af)| SignalPin {
                signal: s(sig),
                pin: s(pin),
                af: Some(*af),
            })
            .collect(),
    }
}

fn irq(name: &str, number: u16, priority: u8) -> InterruptDef {
    InterruptDef {
        name: s(name),
        irq_number: number,
        default_priority: priority,
    }
}

fn dma(controller: &str, channel: u8, peripheral: &str, direction: DmaDirection) -> DmaChannel {
    DmaChannel {
        controller: s(controller),
        channel,
        peripheral: s(peripheral),
        direction,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    #[test]
    fn all_families_resolve() {
        for f in supported_families() {
            assert!(mcu_for(f).is_some(), "missing McuDef for {f}");
        }
    }

    #[test]
    fn unknown_family_returns_none() {
        assert!(mcu_for("Stm32h7").is_none());
    }

    #[test]
    fn rp2040_has_30_pins() {
        let mcu = mcu_for("Rp2040").unwrap();
        assert_eq!(mcu.pins.len(), 30);
    }

    #[test]
    fn rp2040_adc_pins_have_channel() {
        let mcu = mcu_for("Rp2040").unwrap();
        let adc_pins: Vec<_> = mcu.pins.iter().filter(|p| p.adc_channel.is_some()).collect();
        assert_eq!(adc_pins.len(), 4);
        assert_eq!(adc_pins[0].name, "GP26");
    }

    #[test]
    fn stm32f4_has_pll_config() {
        let mcu = mcu_for("Stm32f4").unwrap();
        let pll = mcu.clock.pll.as_ref().unwrap();
        assert_eq!(pll.source, ClockSource::Hse);
        assert_eq!(mcu.clock.sys_clk_mhz, 84.0);
    }

    #[test]
    fn stm32f4_usart1_has_tx_rx_pins() {
        let mcu = mcu_for("Stm32f4").unwrap();
        let usart1 = mcu.peripherals.iter().find(|p| p.name == "USART1").unwrap();
        let tx_pins: Vec<_> = usart1.signals.iter().filter(|s| s.signal == "TX").collect();
        let rx_pins: Vec<_> = usart1.signals.iter().filter(|s| s.signal == "RX").collect();
        assert_eq!(tx_pins.len(), 2); // PA9, PB6
        assert_eq!(rx_pins.len(), 2); // PA10, PB7
    }

    #[test]
    fn stm32g0b1_has_fdcan() {
        let mcu = mcu_for("Stm32g0b1").unwrap();
        let can_periphs: Vec<_> = mcu.peripherals.iter()
            .filter(|p| p.kind == PeripheralKind::Can)
            .collect();
        assert_eq!(can_periphs.len(), 2); // FDCAN1, FDCAN2
    }

    #[test]
    fn esp32c3_has_wifi_core() {
        let mcu = mcu_for("Esp32c3").unwrap();
        assert_eq!(mcu.core, CpuCore::RiscV32IMC);
        assert_eq!(mcu.clock.sys_clk_mhz, 160.0);
    }

    #[test]
    fn pin_alt_functions_populated() {
        let mcu = mcu_for("Stm32f4").unwrap();
        let pa9 = mcu.pins.iter().find(|p| p.name == "PA9").unwrap();
        // PA9 should have AFs for USART1 TX, TIM1 CH2
        assert!(pa9.alt_functions.iter().any(|af| af.peripheral == "USART1" && af.signal == "TX"));
        assert!(pa9.alt_functions.iter().any(|af| af.peripheral == "TIM1" && af.signal == "CH2"));
    }

    #[test]
    fn memory_regions_populated() {
        let mcu = mcu_for("Stm32f4").unwrap();
        assert_eq!(mcu.memory.len(), 2);
        let flash = &mcu.memory[0];
        assert_eq!(flash.kind, MemoryKind::Flash);
        assert_eq!(flash.size, 256 * 1024);
    }

    #[test]
    fn dma_channels_populated() {
        let mcu = mcu_for("Stm32f4").unwrap();
        assert!(!mcu.dma_channels.is_empty());
        assert!(mcu.dma_channels.iter().any(|d| d.peripheral == "ADC1"));
    }

    #[test]
    fn serde_roundtrip_mcu_def() {
        let mcu = mcu_for("Rp2040").unwrap();
        let json = serde_json::to_string(&mcu).unwrap();
        let parsed: McuDef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.family, "Rp2040");
        assert_eq!(parsed.pins.len(), 30);
    }
}
