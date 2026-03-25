# STM32G0B1CBT6 MBD Controller — Design Spec

## Overview

Add support for an embedded controller targeting the STM32G0B1CBT6 (Cortex-M0+) with:
- 1 quadrature encoder (A/B phases) for position feedback
- 1 TMC2209 stepper driver (step/dir + StallGuard via UART)
- 1 SSD1306 OLED display (64x32 pixels, I2C) for status text
- 1 kHz control loop tick rate

This requires: a new codegen target, 4 new dataflow blocks, extensions to the `Peripherals` trait, and new pin binding variants.

## Approach

Extend the existing `Peripherals` trait with new methods for encoder, display, stepper, and StallGuard. Each new peripheral gets its own block type following the exact pattern of existing embedded blocks (ADC, PWM, GPIO, UART). The STM32G0B1 target generator follows the STM32F4 pattern but with real Embassy drivers for the new peripherals.

## New Block Types

### 1. `encoder` — Quadrature Encoder Source

| Field | Value |
|-------|-------|
| Type ID | `encoder` |
| Category | Embedded |
| Config | `{ "channel": 0 }` |
| Inputs | none |
| Outputs | `position` (Float — accumulated count as f64), `velocity` (Float — counts/sec) |
| WASM | stub, returns None |
| Codegen | `state.out_{id}_p0 = hw.encoder_read(channel) as f64;` + velocity = delta/dt in logic |

### 2. `ssd1306_display` — I2C OLED Display Sink

| Field | Value |
|-------|-------|
| Type ID | `ssd1306_display` |
| Category | Embedded |
| Config | `{ "i2c_bus": 0, "address": 60 }` (0x3C default) |
| Inputs | `line1` (Text), `line2` (Text) |
| Outputs | none |
| WASM | stub, no-op |
| Codegen | `hw.display_write(bus, addr, &line1, &line2);` |

### 3. `tmc2209_stepper` — TMC2209 Stepper Motor Driver

| Field | Value |
|-------|-------|
| Type ID | `tmc2209_stepper` |
| Category | Embedded |
| Config | `{ "uart_port": 0, "uart_addr": 0, "steps_per_rev": 200, "microsteps": 16 }` |
| Inputs | `target_position` (Float — target step count), `enable` (Float — >0.5 enables) |
| Outputs | `actual_position` (Float — current step count) |
| WASM | stub |
| Codegen | `hw.stepper_enable(port, enable > 0.5); hw.stepper_move(port, target as i64); state.out_{id}_p0 = hw.stepper_position(port) as f64;` |

### 4. `tmc2209_stallguard` — TMC2209 StallGuard Reader

| Field | Value |
|-------|-------|
| Type ID | `tmc2209_stallguard` |
| Category | Embedded |
| Config | `{ "uart_port": 0, "uart_addr": 0, "threshold": 50 }` |
| Inputs | none |
| Outputs | `sg_value` (Float — 0–1023), `stall_detected` (Float — 1.0 if below threshold) |
| WASM | stub, returns None |
| Codegen | `state.out_{id}_p0 = hw.stallguard_read(port, addr) as f64; state.out_{id}_p1 = if (sg_value as u16) < threshold { 1.0 } else { 0.0 };` |

## Peripherals Trait Extensions

Add to `dataflow-rt/src/lib.rs` and `generate_rt_lib_rs()` in `emit.rs`:

```rust
pub trait Peripherals {
    // ... existing 6 methods ...

    // Quadrature encoder
    fn encoder_read(&mut self, channel: u8) -> i64 { 0 }

    // SSD1306 display
    fn display_write(&mut self, bus: u8, addr: u8, line1: &str, line2: &str) {}

    // TMC2209 stepper
    fn stepper_move(&mut self, port: u8, target: i64) {}
    fn stepper_position(&self, port: u8) -> i64 { 0 }
    fn stepper_enable(&mut self, port: u8, enabled: bool) {}

    // TMC2209 StallGuard
    fn stallguard_read(&mut self, port: u8, addr: u8) -> u16 { 0 }
}
```

Default implementations ensure existing targets compile without changes.

## New Target: STM32G0B1CBT6

### Target Definition (`target.rs`)

```rust
TargetFamily::Stm32g0b1 // new enum variant

TargetDef {
    family: TargetFamily::Stm32g0b1,
    name: "stm32g0b1cb",
    rust_target: "thumbv6m-none-eabi",
    embassy_chip: "stm32g0b1cb",
    peripherals: PeripheralSet {
        adc_channels: 16,
        pwm_channels: 12,
        gpio_pins: 64,
        uart_ports: 4,
    },
}
```

### Target Generator (`targets/stm32g0b1.rs`)

Follows the STM32F4 pattern but generates real Embassy driver initialization:

**Generated `Cargo.toml` deps:**
- `embassy-stm32` with `stm32g0b1cb`, `time-driver-tim3` features
- `ssd1306` crate for display driver
- `embedded-graphics` for text rendering on display

**Generated `main.rs` structure:**
- Embassy async main
- `HwPeripherals` struct holding:
  - Timer in encoder mode (TIM1 CH1+CH2 for A/B quadrature)
  - I2C peripheral for SSD1306
  - UART for TMC2209 protocol
  - Step/Dir GPIO pins for stepper pulse generation
- Implements `Peripherals` trait with real drivers
- 1kHz ticker control loop

### Pin Binding Extensions (`binding.rs`)

```rust
pub enum PinBinding {
    // ... existing variants ...
    Encoder {
        logical_channel: u8,
        pin_a: String,
        pin_b: String,
        timer: String,
    },
    I2cDisplay {
        logical_bus: u8,
        sda_pin: String,
        scl_pin: String,
        peripheral: String,
    },
    Stepper {
        logical_port: u8,
        step_pin: String,
        dir_pin: String,
        enable_pin: String,
        uart_tx: String,
        uart_rx: String,
        peripheral: String,
    },
}
```

## Files to Modify

| File | Change |
|------|--------|
| `src/dataflow/codegen/target.rs` | Add `Stm32g0b1` variant, add `TargetDef`, update test |
| `src/dataflow/codegen/targets/mod.rs` | Add `pub mod stm32g0b1;`, update `generator_for()` |
| `src/dataflow/codegen/targets/stm32g0b1.rs` | **NEW** — target generator |
| `src/dataflow/codegen/emit.rs` | Add new block types to `PERIPHERAL_BLOCK_TYPES`, `is_peripheral_source()`, codegen match arms, `build_workspace_members()`, `generate_rt_lib_rs()` |
| `src/dataflow/codegen/binding.rs` | Add `Encoder`, `I2cDisplay`, `Stepper` variants |
| `src/dataflow/blocks/embedded.rs` | Add 4 new block structs + configs + tests |
| `src/dataflow/blocks/mod.rs` | Register 4 new types in `create_block()` and `available_block_types()` |
| `dataflow-rt/src/lib.rs` | Add 6 new methods with defaults to `Peripherals` trait |
| `www/src/dataflow/types.ts` | Add new block types to TypeScript mirror |
| `src/lib.rs` | Expose `Stm32g0b1` in WASM API if needed |

## STM32G0B1 Hardware Mapping

| Peripheral | STM32G0B1 Resource | Notes |
|------------|-------------------|-------|
| Encoder A/B | TIM1_CH1 (PA8), TIM1_CH2 (PA9) | Hardware encoder mode |
| I2C (SSD1306) | I2C1: SDA=PB7, SCL=PB6 | 400kHz Fast mode |
| TMC2209 UART | USART2: TX=PA2, RX=PA3 | 115200 baud, single-wire possible |
| Stepper STEP | PA0 (GPIO out) | Pulse generation via timer or bitbang |
| Stepper DIR | PA1 (GPIO out) | Direction signal |
| Stepper EN | PA4 (GPIO out) | Active low enable |

(These are default pins — actual mapping configured via `PinBinding`.)

## Verification

1. `cargo test` — all existing + new block tests pass
2. `cargo test dataflow` — codegen produces valid workspace with stm32g0b1 target
3. `wasm-pack build --target web` — WASM build succeeds (new blocks stubbed)
4. `cd www && npm run dev` — new blocks appear in palette under "Embedded" category
5. Generated workspace compiles: `cd /tmp/generated && cargo build --release -p target-stm32g0b1`
