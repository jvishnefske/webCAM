//! Reusable BlockNode component for the DAG editor canvas.
//!
//! Renders an HTML div node with port circles via the [`Port`] component.
//! Nodes are absolutely positioned on the canvas and display block name,
//! type label, and input/output ports.

use leptos::prelude::*;

use super::port::{Port, WireDrag};

/// Descriptor for a single port on a block node.
#[derive(Clone, Debug)]
pub struct PortDef {
    /// Display name (channel topic name).
    pub name: String,
    /// `"input"` or `"output"`.
    pub side: &'static str,
}

/// A single block node on the canvas.
///
/// Renders the node box, title, type label, and input/output ports.
/// Port interaction (wire drag) is delegated to [`Port`] components.
#[component]
pub fn BlockNode(
    /// Numeric block identifier.
    block_id: u32,
    /// Display name (e.g. "Constant").
    name: String,
    /// Block type string (e.g. "constant").
    block_type: String,
    /// X position on the canvas (CSS pixels).
    x: f64,
    /// Y position on the canvas (CSS pixels).
    y: f64,
    /// Whether this node is currently selected.
    selected: Signal<bool>,
    /// Ordered list of input ports.
    inputs: Vec<PortDef>,
    /// Ordered list of output ports.
    outputs: Vec<PortDef>,
    /// Called when the node body is clicked (for selection).
    on_select: Callback<u32>,
    /// Current wire drag state (passed through to Port components).
    wire_drag: ReadSignal<Option<WireDrag>>,
    /// Called when user starts dragging from an output port.
    on_wire_start: Callback<WireDrag>,
    /// Called when user drops wire onto an input port.
    on_wire_end: Callback<(u32, usize)>,
) -> impl IntoView {
    let port_start_y = 46.0_f64;
    let port_spacing = 16.0_f64;
    let port_count = inputs.len().max(outputs.len());
    let height = 50.0 + port_count as f64 * port_spacing;

    // Build input port views.
    let input_views = inputs
        .into_iter()
        .enumerate()
        .map(|(i, pd)| {
            let y_off = port_start_y + i as f64 * port_spacing;
            view! {
                <Port
                    block_id=block_id
                    index=i
                    side="input"
                    name=pd.name
                    y_offset=y_off
                    wire_drag=wire_drag
                    on_drag_start=on_wire_start
                    on_drag_end=on_wire_end
                />
            }
        })
        .collect_view();

    // Build output port views.
    let output_views = outputs
        .into_iter()
        .enumerate()
        .map(|(i, pd)| {
            let y_off = port_start_y + i as f64 * port_spacing;
            view! {
                <Port
                    block_id=block_id
                    index=i
                    side="output"
                    name=pd.name
                    y_offset=y_off
                    wire_drag=wire_drag
                    on_drag_start=on_wire_start
                    on_drag_end=on_wire_end
                />
            }
        })
        .collect_view();

    view! {
        <div
            class="df-node"
            class:selected=move || selected.get()
            style=format!(
                "position:absolute;left:{}px;top:{}px;width:190px;height:{}px",
                x, y, height
            )
            on:mousedown=move |ev: web_sys::MouseEvent| {
                // Only select on primary button, don't interfere with port drags.
                if ev.button() == 0 {
                    on_select.run(block_id);
                }
            }
        >
            <div class="df-node-title">{name}</div>
            <div class="df-node-type">{block_type}</div>
            {input_views}
            {output_views}
        </div>
    }
}
