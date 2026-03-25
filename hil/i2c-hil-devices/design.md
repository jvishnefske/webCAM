# i2c-hil-devices Functional Requirements

## INA230 Power/Current Monitor

- [x] **FR-DEV-01**: INA230 implements `SmBusWordDevice` with pointer byte + 16-bit register protocol
- [x] **FR-DEV-02**: Valid pointer values are 0x00–0x07 and 0xFF; all others return `DataNak`
- [x] **FR-DEV-03**: Configuration register (0x00) defaults to 0x4127 and is read/write
- [x] **FR-DEV-04**: RST bit (D15) in configuration register resets all registers to POR defaults
- [x] **FR-DEV-05**: Shunt voltage (0x01), bus voltage (0x02), power (0x03), current (0x04), die ID (0xFF) are read-only; writes return `DataNak`
- [x] **FR-DEV-06**: Current register computed as `(shunt_voltage * calibration) / 2048`
- [x] **FR-DEV-07**: Power register computed as `(|current| * bus_voltage) / 20_000`
- [x] **FR-DEV-08**: Current and power return 0 when calibration register is 0
- [x] **FR-DEV-09**: Calibration register (0x05) masks D15 to 0 on write (reserved bit)
- [x] **FR-DEV-10**: Mask/Enable register (0x06) clears CVRF flag (D3) on read
- [x] **FR-DEV-11**: Bus voltage is masked to 15 bits (D15 always 0) on injection
- [x] **FR-DEV-12**: Alert limit register (0x07) stores full 16-bit value
- [x] **FR-DEV-13**: Die ID register (0xFF) defaults to 0x2260 and is configurable via constructor
- [x] **FR-DEV-14**: INA230 works on SimBus via blanket `I2cDevice` impl from `SmBusWordDevice`

## EMC2305 5-Fan PWM Controller

- [x] **FR-DEV-21**: EMC2305 implements I2cDevice with SMBus byte/word protocol
- [x] **FR-DEV-22**: EMC2305 5 independent fan channels with PWM setting registers
- [x] **FR-DEV-23**: EMC2305 TACH reading computed on read via linear transfer function (PWM→RPM→tach)
- [x] **FR-DEV-24**: EMC2305 configurable max RPM per fan for transfer function slope
- [x] **FR-DEV-25**: EMC2305 Product ID=0x34, Manufacturer ID=0x5D, Revision=0x01
- [x] **FR-DEV-26**: EMC2305 POR defaults per datasheet
- [x] **FR-DEV-27**: EMC2305 read-only registers reject writes with DataNak
- [x] **FR-DEV-28**: No unsafe code
