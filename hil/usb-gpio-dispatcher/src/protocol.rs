//! Viperboard GPIO wire protocol types.
//!
//! These types match the Linux `gpio-viperboard.c` and `mfd/viperboard.c`
//! kernel drivers exactly.
//!
//! The viperboard uses USB control transfers for GPIO:
//! - **GPIO-A**: 16 individual pins, per-pin commands via control transfers
//! - **GPIO-B**: 16 pins as a port, bulk set/read via control transfers
//!
//! Both banks use `usb_control_msg()` with vendor request types.

// ──────────────────────────────────────────────────────────────────
// USB identifiers — match `mfd/viperboard.c`
// ──────────────────────────────────────────────────────────────────

/// Viperboard USB Vendor ID (Nano River Technologies).
pub const VPRBRD_VID: u16 = 0x2058;
/// Viperboard USB Product ID.
pub const VPRBRD_PID: u16 = 0x1005;

// ──────────────────────────────────────────────────────────────────
// USB request constants — match `include/linux/mfd/viperboard.h`
// ──────────────────────────────────────────────────────────────────

/// USB bmRequestType for OUT transfers (host → device).
/// `USB_DIR_OUT | USB_TYPE_VENDOR | USB_RECIP_INTERFACE` = 0x40
pub const VPRBRD_USB_TYPE_OUT: u8 = 0x40;

/// USB bmRequestType for IN transfers (device → host).
/// `USB_DIR_IN | USB_TYPE_VENDOR | USB_RECIP_INTERFACE` = 0xC0
pub const VPRBRD_USB_TYPE_IN: u8 = 0xC0;

/// bRequest code for GPIO-A operations.
pub const VPRBRD_USB_REQUEST_GPIOA: u8 = 0xED;

/// bRequest code for GPIO-B operations.
pub const VPRBRD_USB_REQUEST_GPIOB: u8 = 0xEE;

/// bRequest code for I2C operations (for future MFD expansion).
pub const VPRBRD_USB_REQUEST_I2C: u8 = 0xE9;

/// USB timeout in milliseconds.
pub const VPRBRD_USB_TIMEOUT_MS: u32 = 100;

// ──────────────────────────────────────────────────────────────────
// GPIO-A commands — match `gpio-viperboard.c`
// ──────────────────────────────────────────────────────────────────

/// GPIO-A: Continuous output.
pub const GPIOA_CMD_CONT: u8 = 0x00;
/// GPIO-A: Pulsed output.
pub const GPIOA_CMD_PULSE: u8 = 0x01;
/// GPIO-A: PWM output.
pub const GPIOA_CMD_PWM: u8 = 0x02;
/// GPIO-A: Set pin as output.
pub const GPIOA_CMD_SETOUT: u8 = 0x03;
/// GPIO-A: Set pin as input.
pub const GPIOA_CMD_SETIN: u8 = 0x04;
/// GPIO-A: Set interrupt mode.
pub const GPIOA_CMD_SETINT: u8 = 0x05;
/// GPIO-A: Get (read) input value.
pub const GPIOA_CMD_GETIN: u8 = 0x06;

// ──────────────────────────────────────────────────────────────────
// GPIO-A clock divider constants
// ──────────────────────────────────────────────────────────────────

pub const GPIOA_CLK_1MHZ: u8 = 0;
pub const GPIOA_CLK_100KHZ: u8 = 1;
pub const GPIOA_CLK_10KHZ: u8 = 2;
pub const GPIOA_CLK_1KHZ: u8 = 3;
pub const GPIOA_CLK_100HZ: u8 = 4;
pub const GPIOA_CLK_10HZ: u8 = 5;

// ──────────────────────────────────────────────────────────────────
// GPIO-B commands — match `gpio-viperboard.c`
// ──────────────────────────────────────────────────────────────────

/// GPIO-B: Set direction mask.
pub const GPIOB_CMD_SETDIR: u8 = 0x00;
/// GPIO-B: Set output values.
pub const GPIOB_CMD_SETVAL: u8 = 0x01;

// ──────────────────────────────────────────────────────────────────
// Wire-format message structs
// ──────────────────────────────────────────────────────────────────

/// GPIO-A message — 11 bytes, matches `struct vprbrd_gpioa_msg`.
///
/// Sent/received via USB control transfer with `bRequest = 0xED`.
/// The host sends this to configure or query a single GPIO-A pin.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(C, packed)]
pub struct GpioAMsg {
    /// Command byte (see `GPIOA_CMD_*`).
    pub cmd: u8,
    /// Clock divider (see `GPIOA_CLK_*`).
    pub clk: u8,
    /// Pin offset (0–15).
    pub offset: u8,
    /// Timing parameter 1.
    pub t1: u8,
    /// Timing parameter 2.
    pub t2: u8,
    /// Invert flag.
    pub invert: u8,
    /// PWM level (0–255).
    pub pwmlevel: u8,
    /// Output value (0 or 1) for SETOUT/CONT commands.
    pub outval: u8,
    /// Rise/fall edge selection for interrupts.
    pub risefall: u8,
    /// Answer byte — device writes the pin state here for GETIN.
    pub answer: u8,
    /// Padding to align.
    pub _fill: u8,
}

impl GpioAMsg {
    pub const SIZE: usize = 11;

    /// Parse from a byte slice (control transfer data stage).
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            cmd: data[0],
            clk: data[1],
            offset: data[2],
            t1: data[3],
            t2: data[4],
            invert: data[5],
            pwmlevel: data[6],
            outval: data[7],
            risefall: data[8],
            answer: data[9],
            _fill: data[10],
        })
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self, buf: &mut [u8]) -> usize {
        if buf.len() < Self::SIZE {
            return 0;
        }
        buf[0] = self.cmd;
        buf[1] = self.clk;
        buf[2] = self.offset;
        buf[3] = self.t1;
        buf[4] = self.t2;
        buf[5] = self.invert;
        buf[6] = self.pwmlevel;
        buf[7] = self.outval;
        buf[8] = self.risefall;
        buf[9] = self.answer;
        buf[10] = self._fill;
        Self::SIZE
    }

    /// Create a response for GETIN (read input pin).
    pub fn new_getin_response(offset: u8, value: bool) -> Self {
        Self {
            cmd: GPIOA_CMD_GETIN,
            clk: 0,
            offset,
            t1: 0,
            t2: 0,
            invert: 0,
            pwmlevel: 0,
            outval: 0,
            risefall: 0,
            answer: if value { 1 } else { 0 },
            _fill: 0,
        }
    }
}

/// GPIO-B message — 5 bytes, matches `struct vprbrd_gpiob_msg`.
///
/// Sent/received via USB control transfer with `bRequest = 0xEE`.
/// GPIO-B operates on all 16 pins simultaneously as a port.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(C, packed)]
pub struct GpioBMsg {
    /// Command byte (`GPIOB_CMD_SETDIR` or `GPIOB_CMD_SETVAL`).
    pub cmd: u8,
    /// 16-bit value (pin states or read-back).
    pub val: [u8; 2],
    /// 16-bit mask (which pins to affect).
    pub mask: [u8; 2],
}

impl GpioBMsg {
    pub const SIZE: usize = 5;

    /// Parse from a byte slice.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            cmd: data[0],
            val: [data[1], data[2]],
            mask: [data[3], data[4]],
        })
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self, buf: &mut [u8]) -> usize {
        if buf.len() < Self::SIZE {
            return 0;
        }
        buf[0] = self.cmd;
        buf[1] = self.val[0];
        buf[2] = self.val[1];
        buf[3] = self.mask[0];
        buf[4] = self.mask[1];
        Self::SIZE
    }

    pub fn val_u16(&self) -> u16 {
        u16::from_le_bytes(self.val)
    }

    pub fn mask_u16(&self) -> u16 {
        u16::from_le_bytes(self.mask)
    }

    /// Create a response with port values for a read-back.
    pub fn new_readback(val: u16) -> Self {
        Self {
            cmd: GPIOB_CMD_SETVAL,
            val: val.to_le_bytes(),
            mask: 0xFFFFu16.to_le_bytes(),
        }
    }
}
