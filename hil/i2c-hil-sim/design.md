# i2c-hil-sim Functional Requirements

## Core Bus Simulation

- [x] **FR-SIM-01**: `SimBus` implements `embedded_hal::i2c::I2c` trait
- [x] **FR-SIM-02**: Transactions route to the correct device by 7-bit address
- [x] **FR-SIM-03**: Missing address returns `BusError::NoDeviceAtAddress`
- [x] **FR-SIM-04**: `SimBusBuilder` enforces unique addresses per bus (panics on duplicate)
- [x] **FR-SIM-05**: `RegisterDevice<N>` models flat register-pointer protocol with auto-increment
- [x] **FR-SIM-06**: `DeviceSet` type-level linked list dispatches without heap allocation
- [x] **FR-SIM-07**: `Address` newtype validates 7-bit range (0x00–0x7F)

## TMP1075 Temperature Sensor

- [x] **FR-SIM-08**: `Tmp1075` has 4 registers: Temp (0x00), Config (0x01), T_LOW (0x02), T_HIGH (0x03)
- [x] **FR-SIM-09**: Default values: Temp=0x0000, Config=0x00FF, T_LOW=0x4B00, T_HIGH=0x5000
- [x] **FR-SIM-10**: Pointer byte (0–3) selects 16-bit register; values >3 return `DataNak`
- [x] **FR-SIM-11**: Temperature register (pointer 0) is read-only; writes return `DataNak`
- [x] **FR-SIM-12**: Reads return MSB,LSB of selected register, repeating for longer reads
- [x] **FR-SIM-13**: `celsius_to_raw` converts float Celsius to 12-bit left-aligned encoding

## Generic I2C Switch

- [x] **FR-SIM-14**: `I2cSwitch<C>` implements `embedded_hal::i2c::I2c` for any `C: ChannelSet`
- [x] **FR-SIM-15**: Transaction to own address reads/writes 1-byte control register
- [x] **FR-SIM-16**: Control register masked to `(1 << channel_count) - 1`
- [x] **FR-SIM-17**: Transactions to other addresses route to enabled channels' `DeviceSet`s
- [x] **FR-SIM-18**: Multiple channels enabled: try lowest-numbered first, fall through on `NoDeviceAtAddress`
- [x] **FR-SIM-19**: `I2cSwitch` can be placed on `SimBus` via `DeviceSet` impl alongside other devices
- [x] **FR-SIM-20**: `I2cSwitchBuilder::channel()` adds a single-device channel

## SmBus Word Device Abstraction

- [x] **FR-SIM-21**: `SmBusWordDevice` trait abstracts 16-bit register pointer protocol (pointer byte + MSB + LSB)
- [x] **FR-SIM-22**: Blanket `I2cDevice` impl for `SmBusWordDevice` routes Write(1 byte) to `set_pointer`, Write(3+ bytes) to `set_pointer` + `write_register`
- [x] **FR-SIM-23**: Blanket impl routes Read to `read_register` at current pointer, fills buffer with MSB/LSB repeating
- [x] **FR-SIM-24**: Empty write operations are silently ignored (no-op) in blanket impl
