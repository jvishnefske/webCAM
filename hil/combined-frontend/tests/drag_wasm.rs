//! Browser-driven regression tests for the DAG editor drag module.
//!
//! These tests run under `wasm-pack test --headless --chrome` (or
//! `--firefox`). They dispatch synthesized `mousemove`/`mouseup` events on
//! `window` — the exact path that SVG-scoped listeners missed — and verify
//! that the drag module's window-level listeners pick them up, update graph
//! state, and clean up after themselves.
//!
//! Run locally:
//!
//! ```sh
//! cd hil/combined-frontend && wasm-pack test --headless --chrome
//! ```

#![cfg(target_arch = "wasm32")]

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{MouseEvent, MouseEventInit};

use combined_frontend::components::dag::drag::{self, DraggingWire};
use combined_frontend::graph_state::GraphState;

wasm_bindgen_test_configure!(run_in_browser);

const SVG_NS: &str = "http://www.w3.org/2000/svg";

/// Build a fresh `<svg>` fixture, append to body, return it and a cleanup
/// closure.
fn mount_svg() -> web_sys::Element {
    let document = web_sys::window().unwrap().document().unwrap();
    let svg = document.create_element_ns(Some(SVG_NS), "svg").unwrap();
    svg.set_attribute("viewBox", "0 0 700 400").unwrap();
    svg.set_attribute("width", "700").unwrap();
    svg.set_attribute("height", "400").unwrap();
    // Inline style to force layout so get_bounding_client_rect is non-zero.
    svg.set_attribute(
        "style",
        "position:absolute;left:0;top:0;width:700px;height:400px",
    )
    .unwrap();
    document.body().unwrap().append_child(&svg).unwrap();
    svg
}

fn make_mouse_event(ty: &str, client_x: i32, client_y: i32) -> MouseEvent {
    let init = MouseEventInit::new();
    init.set_client_x(client_x);
    init.set_client_y(client_y);
    init.set_bubbles(true);
    init.set_cancelable(true);
    MouseEvent::new_with_mouse_event_init_dict(ty, &init).unwrap()
}

fn dispatch_on_window(ev: &MouseEvent) {
    let window = web_sys::window().unwrap();
    let target: web_sys::EventTarget = window.unchecked_into();
    target.dispatch_event(ev).unwrap();
}

fn cleanup(svg: &web_sys::Element) {
    // Ensure any lingering drag session is closed so later tests are clean.
    dispatch_on_window(&make_mouse_event("mouseup", 0, 0));
    if let Some(parent) = svg.parent_node() {
        let _ = parent.remove_child(svg);
    }
}

// ---------------------------------------------------------------------------
// Node-drag tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn node_drag_tracks_mousemove_dispatched_on_window() {
    let owner = Owner::new();
    owner.with(|| {
        let svg = mount_svg();
        let gs = GraphState::new();
        let block_id = gs.add_block("gain").expect("add_block");
        let start = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();

        let md = make_mouse_event("mousedown", 100, 100);
        drag::start_node_drag(
            &md,
            svg.clone(),
            block_id,
            start.x,
            start.y,
            gs.clone(),
            |_| {},
        );

        // Dispatch on window — this is what SVG-scoped listeners couldn't see.
        dispatch_on_window(&make_mouse_event("mousemove", 150, 130));

        let after = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();
        assert!(
            (after.x - start.x).abs() > 1.0,
            "block x should move after window mousemove (start={}, after={})",
            start.x,
            after.x,
        );
        assert!(
            (after.y - start.y).abs() > 1.0,
            "block y should move after window mousemove (start={}, after={})",
            start.y,
            after.y,
        );

        cleanup(&svg);
    });
}

#[wasm_bindgen_test]
fn node_drag_mouseup_on_window_clears_state() {
    let owner = Owner::new();
    owner.with(|| {
        let svg = mount_svg();
        let gs = GraphState::new();
        let block_id = gs.add_block("gain").expect("add_block");
        let start = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();

        let md = make_mouse_event("mousedown", 200, 200);
        drag::start_node_drag(
            &md,
            svg.clone(),
            block_id,
            start.x,
            start.y,
            gs.clone(),
            |_| {},
        );

        dispatch_on_window(&make_mouse_event("mousemove", 260, 240));
        let first_move = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();

        // End drag via window mouseup, then dispatch another move.
        dispatch_on_window(&make_mouse_event("mouseup", 260, 240));
        dispatch_on_window(&make_mouse_event("mousemove", 400, 400));

        let final_pos = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();
        assert!(
            (final_pos.x - first_move.x).abs() < 0.01 && (final_pos.y - first_move.y).abs() < 0.01,
            "post-mouseup mousemove must not move the block (first={:?}, final={:?})",
            (first_move.x, first_move.y),
            (final_pos.x, final_pos.y),
        );

        cleanup(&svg);
    });
}

#[wasm_bindgen_test]
fn node_drag_below_threshold_fires_click_callback() {
    let owner = Owner::new();
    owner.with(|| {
        let svg = mount_svg();
        let gs = GraphState::new();
        let block_id = gs.add_block("gain").expect("add_block");
        let start = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();

        let (click_signal, set_click_signal) = signal(None::<usize>);

        let md = make_mouse_event("mousedown", 100, 100);
        drag::start_node_drag(
            &md,
            svg.clone(),
            block_id,
            start.x,
            start.y,
            gs.clone(),
            move |id| set_click_signal.set(Some(id)),
        );

        // Tiny motion (below 3px threshold), then release.
        dispatch_on_window(&make_mouse_event("mousemove", 101, 101));
        dispatch_on_window(&make_mouse_event("mouseup", 101, 101));

        assert_eq!(
            click_signal.get_untracked(),
            Some(block_id),
            "below-threshold release should fire click callback with block id",
        );
        let unchanged = gs
            .blocks
            .get_untracked()
            .iter()
            .find(|b| b.id == block_id)
            .cloned()
            .unwrap();
        assert!(
            (unchanged.x - start.x).abs() < 0.01,
            "below-threshold release must not move block",
        );

        cleanup(&svg);
    });
}

// ---------------------------------------------------------------------------
// Wire-drag tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn wire_drag_drop_on_empty_canvas_clears_state() {
    let owner = Owner::new();
    owner.with(|| {
        let svg = mount_svg();
        let gs = GraphState::new();
        let src = gs.add_block("gain").expect("add_block");

        let (dragging_wire, set_dragging_wire) = signal(None::<DraggingWire>);
        let (_edge_error, set_edge_error) = signal(None::<String>);

        let md = make_mouse_event("mousedown", 300, 100);
        drag::start_wire_drag(
            &md,
            svg.clone(),
            src,
            0,
            set_dragging_wire,
            gs.clone(),
            set_edge_error,
        );

        assert!(
            dragging_wire.get_untracked().is_some(),
            "rubber-band should be set at drag start",
        );

        // Mouseup with no input port under the cursor -> cancel.
        dispatch_on_window(&make_mouse_event("mouseup", 400, 100));

        assert!(
            dragging_wire.get_untracked().is_none(),
            "rubber-band must clear on mouseup over empty canvas",
        );

        cleanup(&svg);
    });
}

#[wasm_bindgen_test]
fn wire_drag_mousemove_updates_rubber_band_position() {
    let owner = Owner::new();
    owner.with(|| {
        let svg = mount_svg();
        let gs = GraphState::new();
        let src = gs.add_block("gain").expect("add_block");

        let (dragging_wire, set_dragging_wire) = signal(None::<DraggingWire>);
        let (_edge_error, set_edge_error) = signal(None::<String>);

        let md = make_mouse_event("mousedown", 300, 100);
        drag::start_wire_drag(
            &md,
            svg.clone(),
            src,
            0,
            set_dragging_wire,
            gs.clone(),
            set_edge_error,
        );

        let before = dragging_wire.get_untracked().unwrap();
        dispatch_on_window(&make_mouse_event("mousemove", 500, 300));
        let after = dragging_wire.get_untracked().unwrap();

        assert!(
            (after.mouse_x - before.mouse_x).abs() > 1.0
                || (after.mouse_y - before.mouse_y).abs() > 1.0,
            "rubber-band end must follow cursor across window mousemove \
             (before=({},{}), after=({},{}))",
            before.mouse_x,
            before.mouse_y,
            after.mouse_x,
            after.mouse_y,
        );

        dispatch_on_window(&make_mouse_event("mouseup", 500, 300));
        cleanup(&svg);
    });
}

#[wasm_bindgen_test]
fn wire_drag_drop_on_input_port_resolves_via_element_from_point() {
    let owner = Owner::new();
    owner.with(|| {
        let svg = mount_svg();
        let gs = GraphState::new();
        let src = gs.add_block("gain").expect("add_block src");
        let dst = gs.add_block("gain").expect("add_block dst");

        // Plant an input-port hit-area where we'll drop. It must be at the
        // same screen coordinates we dispatch mouseup at, so elementFromPoint
        // resolves to it. Positioned as a small absolute div above the SVG.
        let document = web_sys::window().unwrap().document().unwrap();
        let hit = document.create_element("div").unwrap();
        hit.set_attribute(
            "style",
            "position:absolute;left:400px;top:200px;width:20px;height:20px;background:red;z-index:999",
        )
        .unwrap();
        hit.set_attribute("data-side", "in").unwrap();
        hit.set_attribute("data-block-id", &dst.to_string()).unwrap();
        hit.set_attribute("data-port-idx", "0").unwrap();
        document.body().unwrap().append_child(&hit).unwrap();

        let (_dragging_wire, set_dragging_wire) = signal(None::<DraggingWire>);
        let (edge_error, set_edge_error) = signal(None::<String>);

        let md = make_mouse_event("mousedown", 300, 100);
        drag::start_wire_drag(
            &md,
            svg.clone(),
            src,
            0,
            set_dragging_wire,
            gs.clone(),
            set_edge_error,
        );

        // Drop inside the hit div (clientX=410, clientY=210 — inside 400..420 / 200..220).
        dispatch_on_window(&make_mouse_event("mouseup", 410, 210));

        // connect_edge either succeeded (no error) or failed with a
        // type-mismatch message. The test's core assertion is: the drop
        // target WAS resolved via elementFromPoint — which we detect by
        // either an Ok or a domain error (not a silent "no drop target").
        let blks = gs.blocks.get_untracked();
        let src_block = blks.iter().find(|b| b.id == src).unwrap();
        let dst_block = blks.iter().find(|b| b.id == dst).unwrap();
        let connected = src_block
            .config
            .get("output_topic")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
            && dst_block
                .config
                .get("input_topic")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty());
        let has_error = edge_error.get_untracked().is_some();
        assert!(
            connected || has_error,
            "mouseup over an input port must trigger connect_edge (either success \
             or domain error). Neither observed — elementFromPoint fallback is broken.",
        );

        hit.parent_node().unwrap().remove_child(&hit).unwrap();
        cleanup(&svg);
    });
}
