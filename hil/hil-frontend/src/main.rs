//! WASM entry point for the HIL dashboard.
//!
//! Initializes panic hook and mounts the root Leptos application component
//! to the document body.
#![forbid(unsafe_code)]

/// Entry point for the WASM application.
///
/// On non-WASM targets this is an empty stub so the binary compiles for
/// host-target testing.
fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        leptos::mount::mount_to_body(hil_frontend::app::App);
    }
}
