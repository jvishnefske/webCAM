//! Mouse drag sessions for the DAG editor canvas.
//!
//! Drags (node reposition, wire-to-port) must continue tracking even when the
//! cursor leaves the SVG viewport. SVG-scoped listeners cannot do this — they
//! stop firing the moment the pointer exits the element's bounding box, and a
//! `mouseup` outside the SVG never clears drag state.
//!
//! This module registers `mousemove` + `mouseup` on `web_sys::window()` when a
//! drag begins, and removes them when it ends. It mirrors the global-listener
//! idiom used by [`crate::components::keyboard`], but pairs every
//! `add_event_listener_with_callback` with a matching
//! `remove_event_listener_with_callback` so there is no per-drag leak.
//!
//! For wire drops the drop target is located via
//! [`web_sys::Document::element_from_point`] rather than `ev.target()`, which
//! is unreliable when the mouse crosses many elements during a drag. This
//! matches the behavior of the original TypeScript editor.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

use crate::graph_state::GraphState;

/// L1-distance (in client pixels) below which a press-release pair is treated
/// as a click, not a drag.
const CLICK_THRESHOLD_PX: f64 = 3.0;

/// In-progress wire drag from an output port.
///
/// The dashed rubber-band line in the editor is rendered by reading a signal
/// of `Option<DraggingWire>`; the drag session updates `mouse_x`/`mouse_y` on
/// every `mousemove` and clears the signal on release.
#[derive(Clone, Copy)]
pub struct DraggingWire {
    pub from_block: usize,
    pub from_port: usize,
    /// Current mouse X in SVG viewBox coordinates.
    pub mouse_x: f64,
    /// Current mouse Y in SVG viewBox coordinates.
    pub mouse_y: f64,
}

type ListenerHandles = Rc<RefCell<Option<(JsValue, JsValue)>>>;

struct NodeDragState {
    block_id: usize,
    start_mouse_x: f64,
    start_mouse_y: f64,
    start_node_x: f64,
    start_node_y: f64,
    moved: bool,
}

/// Begin a node-drag session.
///
/// Registers window-level `mousemove` / `mouseup` listeners that live until
/// the user releases. `on_click_select` fires only if the release occurs
/// before the cursor exceeds [`CLICK_THRESHOLD_PX`] — i.e. treat a static
/// press as a click.
pub fn start_node_drag(
    ev: &MouseEvent,
    svg_el: web_sys::Element,
    block_id: usize,
    start_node_x: f64,
    start_node_y: f64,
    gs: GraphState,
    on_click_select: impl Fn(usize) + 'static,
) {
    let state = Rc::new(RefCell::new(NodeDragState {
        block_id,
        start_mouse_x: ev.client_x() as f64,
        start_mouse_y: ev.client_y() as f64,
        start_node_x,
        start_node_y,
        moved: false,
    }));
    let listeners: ListenerHandles = Rc::new(RefCell::new(None));

    let Some(window) = web_sys::window() else {
        return;
    };

    let mm_state = state.clone();
    let mm_svg = svg_el.clone();
    let mm_gs = gs.clone();
    let mousemove_cb = Closure::<dyn FnMut(MouseEvent)>::new(move |ev: MouseEvent| {
        let mut s = mm_state.borrow_mut();
        let dx = ev.client_x() as f64 - s.start_mouse_x;
        let dy = ev.client_y() as f64 - s.start_mouse_y;
        if !s.moved && (dx.abs() + dy.abs()) < CLICK_THRESHOLD_PX {
            return;
        }
        let rect = mm_svg.get_bounding_client_rect();
        if rect.width() == 0.0 || rect.height() == 0.0 {
            return;
        }
        let (vb_w, vb_h) = viewbox_dims(&mm_svg);
        let scale_x = vb_w / rect.width();
        let scale_y = vb_h / rect.height();
        let new_x = s.start_node_x + dx * scale_x;
        let new_y = s.start_node_y + dy * scale_y;
        mm_gs.move_block(s.block_id, new_x, new_y);
        s.moved = true;
    });
    let mousemove_js = mousemove_cb.into_js_value();

    let mu_state = state;
    let mu_listeners = listeners.clone();
    let mu_gs = gs;
    let mouseup_cb = Closure::<dyn FnMut(MouseEvent)>::new(move |_ev: MouseEvent| {
        let (moved, block_id) = {
            let s = mu_state.borrow();
            (s.moved, s.block_id)
        };
        if moved {
            mu_gs.bump_revision();
        } else {
            on_click_select(block_id);
        }
        unregister(&mu_listeners);
    });
    let mouseup_js = mouseup_cb.into_js_value();

    let _ = window.add_event_listener_with_callback("mousemove", mousemove_js.unchecked_ref());
    let _ = window.add_event_listener_with_callback("mouseup", mouseup_js.unchecked_ref());
    *listeners.borrow_mut() = Some((mousemove_js, mouseup_js));
}

/// Begin a wire-drag session from an output port.
///
/// Registers window-level `mousemove` / `mouseup` listeners. The rubber-band
/// signal is updated on every move. On release the drop target is resolved
/// via `document.elementFromPoint(clientX, clientY)` and `gs.connect_edge` is
/// attempted. Any error message is written into `on_edge_error`.
pub fn start_wire_drag(
    ev: &MouseEvent,
    svg_el: web_sys::Element,
    from_block: usize,
    from_port: usize,
    set_dragging_wire: WriteSignal<Option<DraggingWire>>,
    gs: GraphState,
    on_edge_error: WriteSignal<Option<String>>,
) {
    let (mx0, my0) = client_to_svg(&svg_el, ev.client_x() as f64, ev.client_y() as f64);
    set_dragging_wire.set(Some(DraggingWire {
        from_block,
        from_port,
        mouse_x: mx0,
        mouse_y: my0,
    }));

    let listeners: ListenerHandles = Rc::new(RefCell::new(None));

    let Some(window) = web_sys::window() else {
        return;
    };

    let mm_svg = svg_el;
    let mousemove_cb = Closure::<dyn FnMut(MouseEvent)>::new(move |ev: MouseEvent| {
        let (mx, my) = client_to_svg(&mm_svg, ev.client_x() as f64, ev.client_y() as f64);
        set_dragging_wire.update(|dw| {
            if let Some(ref mut w) = dw {
                w.mouse_x = mx;
                w.mouse_y = my;
            }
        });
    });
    let mousemove_js = mousemove_cb.into_js_value();

    let mu_listeners = listeners.clone();
    let mouseup_cb = Closure::<dyn FnMut(MouseEvent)>::new(move |ev: MouseEvent| {
        set_dragging_wire.set(None);

        match extract_input_port(ev.client_x(), ev.client_y()) {
            Some((to_block, to_port)) => {
                match gs.connect_edge(from_block, from_port, to_block, to_port) {
                    Ok(()) => on_edge_error.set(None),
                    Err(msg) => on_edge_error.set(Some(msg)),
                }
            }
            None => {
                // Dropped on empty canvas or unrelated element — cancel.
            }
        }
        unregister(&mu_listeners);
    });
    let mouseup_js = mouseup_cb.into_js_value();

    let _ = window.add_event_listener_with_callback("mousemove", mousemove_js.unchecked_ref());
    let _ = window.add_event_listener_with_callback("mouseup", mouseup_js.unchecked_ref());
    *listeners.borrow_mut() = Some((mousemove_js, mouseup_js));
}

/// Resolve the element at the given screen point and return its
/// `(block_id, port_idx)` if it is an input-port hit-area (carries
/// `data-side="in"` plus `data-block-id` and `data-port-idx`).
fn extract_input_port(client_x: i32, client_y: i32) -> Option<(usize, usize)> {
    let document = web_sys::window()?.document()?;
    let el = document.element_from_point(client_x as f32, client_y as f32)?;
    if el.get_attribute("data-side").as_deref() != Some("in") {
        return None;
    }
    let block_id = el.get_attribute("data-block-id")?.parse::<usize>().ok()?;
    let port_idx = el.get_attribute("data-port-idx")?.parse::<usize>().ok()?;
    Some((block_id, port_idx))
}

/// Remove window-level listeners for the current drag session. Safe to call
/// more than once.
fn unregister(listeners: &ListenerHandles) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Some((mm, mu)) = listeners.borrow_mut().take() {
        let _ = window.remove_event_listener_with_callback("mousemove", mm.unchecked_ref());
        let _ = window.remove_event_listener_with_callback("mouseup", mu.unchecked_ref());
    }
}

/// Convert client-space mouse coordinates to SVG user-space coordinates for
/// an element whose `viewBox` attribute defines its internal coordinate
/// system. Falls back to `700 × 400` (the editor's canvas) if absent.
pub(super) fn client_to_svg(svg: &web_sys::Element, client_x: f64, client_y: f64) -> (f64, f64) {
    let rect = svg.get_bounding_client_rect();
    let rect_w = rect.width();
    let rect_h = rect.height();
    let (vb_w, vb_h) = viewbox_dims(svg);
    if rect_w == 0.0 || rect_h == 0.0 {
        return (0.0, 0.0);
    }
    let scale_x = vb_w / rect_w;
    let scale_y = vb_h / rect_h;
    let x = (client_x - rect.left()) * scale_x;
    let y = (client_y - rect.top()) * scale_y;
    (x, y)
}

fn viewbox_dims(svg: &web_sys::Element) -> (f64, f64) {
    svg.get_attribute("viewBox")
        .and_then(|vb| {
            let parts: Vec<f64> = vb
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() == 4 {
                Some((parts[2], parts[3]))
            } else {
                None
            }
        })
        .unwrap_or((700.0, 400.0))
}
