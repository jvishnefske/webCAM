//! Reusable Port component for wire drag interaction.
//!
//! Each port is a small circle rendered on the edge of a block node.
//! Output ports initiate wire drags; input ports complete them.

use leptos::prelude::*;

/// Wire drag state shared between Port components and the Editor.
///
/// When a user starts dragging from an output port, this tracks the source
/// and cursor position. The Editor renders a preview SVG path from this state.
#[derive(Clone, Copy, Debug)]
pub struct WireDrag {
    /// Block id of the source (output) block.
    pub from_block: u32,
    /// Output port index on the source block.
    pub from_port: usize,
    /// Current cursor X in client coordinates.
    pub cursor_x: f64,
    /// Current cursor Y in client coordinates.
    pub cursor_y: f64,
}

/// A single port circle on a block node.
///
/// Handles wire drag initiation (on output ports) and connection completion
/// (on input ports). Port highlight changes during active wire drag.
#[component]
pub fn Port(
    /// Numeric block identifier.
    block_id: u32,
    /// Port index (0-based among ports of the same direction).
    index: usize,
    /// `"input"` or `"output"`.
    side: &'static str,
    /// Display name of the port (e.g. channel topic).
    name: String,
    /// Y position within the node (absolute, from top of node).
    y_offset: f64,
    /// Current wire drag state (`None` if no drag active).
    wire_drag: ReadSignal<Option<WireDrag>>,
    /// Called when user starts dragging from an output port.
    on_drag_start: Callback<WireDrag>,
    /// Called when user drops wire onto an input port: `(target_block_id, target_port_index)`.
    on_drag_end: Callback<(u32, usize)>,
) -> impl IntoView {
    let is_output = side == "output";
    let is_input = side == "input";

    // Highlight input ports when a wire is being dragged (drop target feedback).
    let is_drop_target = move || is_input && wire_drag.get().is_some();

    view! {
        <div
            class="df-port"
            class:input=is_input
            class:output=is_output
            class:drop-target=is_drop_target
            style=move || {
                if is_output {
                    format!("position:absolute;right:-6px;top:{}px", y_offset)
                } else {
                    format!("position:absolute;left:-6px;top:{}px", y_offset)
                }
            }
            data-side=side
            data-block-id=block_id.to_string()
            data-port-idx=index.to_string()
            on:mousedown=move |ev: web_sys::MouseEvent| {
                if is_output {
                    ev.stop_propagation();
                    ev.prevent_default();
                    on_drag_start.run(WireDrag {
                        from_block: block_id,
                        from_port: index,
                        cursor_x: ev.client_x() as f64,
                        cursor_y: ev.client_y() as f64,
                    });
                }
            }
            on:mouseup=move |ev: web_sys::MouseEvent| {
                if is_input {
                    ev.stop_propagation();
                    on_drag_end.run((block_id, index));
                }
            }
        />
        <span
            class="df-port-label"
            style=move || {
                if is_output {
                    format!(
                        "position:absolute;right:10px;top:{}px;text-align:right",
                        y_offset - 5.0
                    )
                } else {
                    format!("position:absolute;left:10px;top:{}px", y_offset - 5.0)
                }
            }
        >
            {name}
        </span>
    }
}
