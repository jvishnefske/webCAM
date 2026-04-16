//! A single block node rendered as an HTML div with CSS transform positioning.

use leptos::prelude::*;

/// A single block node rendered as an HTML div.
///
/// Positioned absolutely via CSS transform within the node layer.
/// Contains a header (block name), type label, and port circles.
#[component]
pub fn BlockNode(
    block_id: u32,
    name: String,
    block_type: String,
    x: f64,
    y: f64,
    is_selected: Signal<bool>,
    input_ports: Vec<String>,
    output_ports: Vec<String>,
    on_select: Callback<u32>,
    on_drag_end: Callback<(u32, f64, f64)>,
) -> impl IntoView {
    // Suppress unused-variable warning; drag-end will be wired in task-005.
    let _ = on_drag_end;

    // Port spacing constants
    let port_offset_y = 30.0;
    let port_spacing = 20.0;
    let node_width = 140.0;
    let in_count = input_ports.len();
    let out_count = output_ports.len();
    let node_height = 40.0 + (in_count.max(out_count) as f64) * port_spacing;

    view! {
        <div
            class="df-node"
            class:selected=move || is_selected.get()
            style=move || {
                format!(
                    "position:absolute;transform:translate({}px,{}px);width:{}px;height:{}px",
                    x, y, node_width, node_height,
                )
            }
            data-id=block_id.to_string()
            on:mousedown=move |ev: web_sys::MouseEvent| {
                ev.stop_propagation();
                on_select.run(block_id);
            }
        >
            <div class="df-node-header">{name}</div>
            <div class="df-node-type">{block_type}</div>
            // Input ports (left side)
            {input_ports
                .into_iter()
                .enumerate()
                .map(|(i, port_name)| {
                    let py = port_offset_y + i as f64 * port_spacing;
                    view! {
                        <div
                            class="df-port input"
                            style=format!("position:absolute;left:-6px;top:{}px", py)
                            data-side="in"
                            data-block-id=block_id.to_string()
                            data-port-idx=i.to_string()
                        />
                        <span
                            class="df-port-label"
                            style=format!(
                                "position:absolute;left:10px;top:{}px",
                                py - 5.0,
                            )
                        >
                            {port_name}
                        </span>
                    }
                })
                .collect_view()}
            // Output ports (right side)
            {output_ports
                .into_iter()
                .enumerate()
                .map(|(i, port_name)| {
                    let py = port_offset_y + i as f64 * port_spacing;
                    view! {
                        <div
                            class="df-port output"
                            style=format!("position:absolute;right:-6px;top:{}px", py)
                            data-side="out"
                            data-block-id=block_id.to_string()
                            data-port-idx=i.to_string()
                        />
                        <span
                            class="df-port-label"
                            style=format!(
                                "position:absolute;right:10px;top:{}px;text-align:right",
                                py - 5.0,
                            )
                        >
                            {port_name}
                        </span>
                    }
                })
                .collect_view()}
        </div>
    }
}
