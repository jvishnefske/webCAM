//! Target-specific code generators.

pub mod esp32c3;
pub mod host;
pub mod rp2040;
pub mod stm32f4;
pub mod stm32g0b1;

use crate::binding::Binding;
use crate::target::TargetFamily;
use graph_model::GraphSnapshot;

/// Trait for target-specific firmware generators.
///
/// Each target generates its own `target-<name>/` subdirectory with
/// Cargo.toml, main.rs, and any target-specific files (memory.x, .cargo/config.toml).
pub trait TargetGenerator {
    fn generate(
        &self,
        snap: &GraphSnapshot,
        binding: &Binding,
        dt: f64,
    ) -> Result<Vec<(String, String)>, String>;
}

/// Dispatch to the appropriate target generator.
pub fn generator_for(family: TargetFamily) -> Box<dyn TargetGenerator> {
    match family {
        TargetFamily::Host => Box::new(host::HostGenerator),
        TargetFamily::Rp2040 => Box::new(rp2040::Rp2040Generator),
        TargetFamily::Stm32f4 => Box::new(stm32f4::Stm32f4Generator),
        TargetFamily::Esp32c3 => Box::new(esp32c3::Esp32c3Generator),
        TargetFamily::Stm32g0b1 => Box::new(stm32g0b1::Stm32g0b1Generator),
    }
}
