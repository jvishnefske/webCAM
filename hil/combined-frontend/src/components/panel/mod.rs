//! Panel editor: widget-based control panels with live topic values.

pub mod types;

#[cfg(target_arch = "wasm32")]
mod editor;

#[cfg(target_arch = "wasm32")]
pub use editor::PanelEditor;
