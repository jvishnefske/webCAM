//! Built-in configurable blocks.

pub mod basic;
pub mod i2c;
pub mod pid;

use crate::lower::ConfigurableBlock;
use crate::schema::BlockCategory;

/// Metadata for a block type shown in the palette.
pub struct BlockEntry {
    pub block_type: &'static str,
    pub display_name: &'static str,
    pub category: BlockCategory,
    pub description: &'static str,
    /// Factory function to create a default instance.
    pub create: fn() -> Box<dyn ConfigurableBlock>,
}

/// All registered configurable block types.
pub fn registry() -> Vec<BlockEntry> {
    vec![
        // Math
        BlockEntry {
            block_type: "constant",
            display_name: "Constant",
            category: BlockCategory::Math,
            description: "Output a fixed value, optionally publish to a topic",
            create: || Box::new(basic::ConstantBlock::default()),
        },
        BlockEntry {
            block_type: "gain",
            display_name: "Gain",
            category: BlockCategory::Math,
            description: "Multiply input by a constant factor",
            create: || Box::new(pid::SimpleGainBlock::default()),
        },
        BlockEntry {
            block_type: "add",
            display_name: "Add",
            category: BlockCategory::Math,
            description: "Add two pubsub inputs and publish the sum",
            create: || Box::new(basic::AddBlock::default()),
        },
        BlockEntry {
            block_type: "multiply",
            display_name: "Multiply",
            category: BlockCategory::Math,
            description: "Multiply two pubsub inputs and publish the product",
            create: || Box::new(basic::MultiplyBlock::default()),
        },
        BlockEntry {
            block_type: "clamp",
            display_name: "Clamp",
            category: BlockCategory::Math,
            description: "Clamp input to a min/max range",
            create: || Box::new(basic::ClampBlock::default()),
        },
        // Control
        BlockEntry {
            block_type: "pid",
            display_name: "PID Controller",
            category: BlockCategory::Control,
            description: "PID controller with configurable gains, pubsub I/O, and output clamping",
            create: || Box::new(pid::PidBlock::default()),
        },
        BlockEntry {
            block_type: "subtract",
            display_name: "Subtract",
            category: BlockCategory::Math,
            description: "Subtract two pubsub inputs (a - b) and publish the difference",
            create: || Box::new(basic::SubtractBlock::default()),
        },
        BlockEntry {
            block_type: "negate",
            display_name: "Negate",
            category: BlockCategory::Math,
            description: "Flip the sign of a pubsub input",
            create: || Box::new(basic::NegateBlock::default()),
        },
        BlockEntry {
            block_type: "map_scale",
            display_name: "Map/Scale",
            category: BlockCategory::Math,
            description: "Linear mapping: (in - in_min)/(in_max - in_min) * (out_max - out_min) + out_min",
            create: || Box::new(basic::MapScaleBlock::default()),
        },
        BlockEntry {
            block_type: "lowpass",
            display_name: "Low-Pass Filter",
            category: BlockCategory::Math,
            description: "Exponential moving average: y = alpha*x + (1-alpha)*y_prev",
            create: || Box::new(basic::LowPassBlock::default()),
        },
        // I/O
        BlockEntry {
            block_type: "adc",
            display_name: "ADC Input",
            category: BlockCategory::Io,
            description: "Read a hardware ADC channel and expose as a hardware input port",
            create: || Box::new(basic::AdcBlock::default()),
        },
        BlockEntry {
            block_type: "pwm",
            display_name: "PWM Output",
            category: BlockCategory::Io,
            description: "Write duty cycle to hardware PWM channel",
            create: || Box::new(basic::PwmBlock::default()),
        },
        // PubSub
        BlockEntry {
            block_type: "subscribe",
            display_name: "Subscribe",
            category: BlockCategory::PubSub,
            description: "Subscribe to a pubsub topic (data source)",
            create: || Box::new(basic::SubscribeBlock::default()),
        },
        BlockEntry {
            block_type: "publish",
            display_name: "Publish",
            category: BlockCategory::PubSub,
            description: "Publish a value to a pubsub topic (data sink)",
            create: || Box::new(basic::PublishBlock::default()),
        },
        BlockEntry {
            block_type: "pubsub_bridge",
            display_name: "PubSub Bridge",
            category: BlockCategory::PubSub,
            description: "Subscribe to a topic, apply gain, publish to another topic",
            create: || Box::new(pid::PubSubBridgeBlock::default()),
        },
        // I2C
        BlockEntry {
            block_type: "i2c_mux",
            display_name: "I2C Mux (TCA9548A)",
            category: BlockCategory::Io,
            description: "Route I2C bus to one of 2/4/8 downstream channels",
            create: || Box::new(i2c::I2cMuxBlock::default()),
        },
    ]
}

/// Return block entries grouped by category.
pub fn registry_by_category() -> Vec<(BlockCategory, Vec<BlockEntry>)> {
    let mut groups: Vec<(BlockCategory, Vec<BlockEntry>)> = BlockCategory::all()
        .iter()
        .map(|cat| (cat.clone(), Vec::new()))
        .collect();

    for entry in registry() {
        if let Some(group) = groups.iter_mut().find(|(cat, _)| *cat == entry.category) {
            group.1.push(entry);
        }
    }

    // Remove empty categories
    groups.retain(|(_, entries)| !entries.is_empty());
    groups
}
