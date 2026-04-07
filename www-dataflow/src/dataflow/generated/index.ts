// Block config types generated from Rust via ts-rs.
// Regenerate with: cargo test -p rustsim export_ts_block_configs -- --ignored

export type { ConstantConfig } from './ConstantConfig.js';
export type { FunctionOp } from './FunctionOp.js';
export type { FunctionConfig } from './FunctionConfig.js';
export type { PlotConfig } from './PlotConfig.js';
export type { UdpConfig } from './UdpConfig.js';
export type { PubSubConfig } from './PubSubConfig.js';
export type { StateMachineConfig } from './StateMachineConfig.js';
export type { AdcConfig } from './AdcConfig.js';
export type { PwmConfig } from './PwmConfig.js';
export type { GpioOutConfig } from './GpioOutConfig.js';
export type { GpioInConfig } from './GpioInConfig.js';
export type { UartTxConfig } from './UartTxConfig.js';
export type { UartRxConfig } from './UartRxConfig.js';
export type { EncoderConfig } from './EncoderConfig.js';
export type { Ssd1306DisplayConfig } from './Ssd1306DisplayConfig.js';
export type { Tmc2209StepperConfig } from './Tmc2209StepperConfig.js';
export type { Tmc2209StallGuardConfig } from './Tmc2209StallGuardConfig.js';

import type { ConstantConfig } from './ConstantConfig.js';
import type { FunctionConfig } from './FunctionConfig.js';
import type { PlotConfig } from './PlotConfig.js';
import type { UdpConfig } from './UdpConfig.js';
import type { PubSubConfig } from './PubSubConfig.js';
import type { StateMachineConfig } from './StateMachineConfig.js';
import type { AdcConfig } from './AdcConfig.js';
import type { PwmConfig } from './PwmConfig.js';
import type { GpioOutConfig } from './GpioOutConfig.js';
import type { GpioInConfig } from './GpioInConfig.js';
import type { UartTxConfig } from './UartTxConfig.js';
import type { UartRxConfig } from './UartRxConfig.js';
import type { EncoderConfig } from './EncoderConfig.js';
import type { Ssd1306DisplayConfig } from './Ssd1306DisplayConfig.js';
import type { Tmc2209StepperConfig } from './Tmc2209StepperConfig.js';
import type { Tmc2209StallGuardConfig } from './Tmc2209StallGuardConfig.js';

/** Maps block_type strings to their config types. */
export interface BlockConfigMap {
  constant: ConstantConfig;
  gain: FunctionConfig;
  clamp: FunctionConfig;
  plot: PlotConfig;
  udp_source: UdpConfig;
  udp_sink: UdpConfig;
  adc_source: AdcConfig;
  pwm_sink: PwmConfig;
  gpio_out: GpioOutConfig;
  gpio_in: GpioInConfig;
  uart_tx: UartTxConfig;
  uart_rx: UartRxConfig;
  pubsub_source: PubSubConfig;
  pubsub_sink: PubSubConfig;
  state_machine: StateMachineConfig;
  encoder: EncoderConfig;
  ssd1306_display: Ssd1306DisplayConfig;
  tmc2209_stepper: Tmc2209StepperConfig;
  tmc2209_stallguard: Tmc2209StallGuardConfig;
}
