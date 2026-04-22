//! Global keyboard shortcut handler.
//!
//! Installs a document-level `keydown` listener that dispatches actions
//! based on the currently active tab. The listener is installed once in
//! the [`App`] component and reads [`AppContext`] to determine which tab
//! is active.
//!
//! Currently supported shortcuts:
//! - **Delete / Backspace**: fire "delete" action (used by DAG editor to remove selected block)
//! - **Space**: fire "toggle play/pause" action (used by DAG editor simulation)

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::app::DataflowTab;

/// Keyboard action signals, provided via Leptos context.
///
/// Child components (e.g. DAG editor) read these signals to react to
/// keyboard shortcuts without coupling the keyboard module to their
/// internal state.
#[derive(Clone)]
pub struct KeyboardActions {
    /// Incremented each time Delete or Backspace is pressed while on the DAG Editor tab.
    pub delete_pressed: ReadSignal<u64>,
    /// Incremented each time Space is pressed while on the DAG Editor tab.
    pub toggle_play_pressed: ReadSignal<u64>,
}

/// Install the document-level `keydown` listener and provide [`KeyboardActions`] context.
///
/// Must be called once from the root App component, after `active_tab` is available.
pub fn install_keyboard_handler(active_tab: ReadSignal<DataflowTab>) {
    let (delete_pressed, set_delete_pressed) = signal(0_u64);
    let (toggle_play_pressed, set_toggle_play_pressed) = signal(0_u64);

    provide_context(KeyboardActions {
        delete_pressed,
        toggle_play_pressed,
    });

    let closure =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
            // Only handle shortcuts when on the DAG Editor tab.
            if active_tab.get_untracked() != DataflowTab::DagEditor {
                return;
            }

            // Ignore key events when an input/textarea/select is focused to avoid
            // interfering with text entry.
            if let Some(target) = ev.target() {
                if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                    let tag = el.tag_name();
                    if tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT" {
                        return;
                    }
                }
            }

            let key = ev.key();
            match key.as_str() {
                "Delete" | "Backspace" => {
                    ev.prevent_default();
                    set_delete_pressed.update(|n| *n += 1);
                }
                " " => {
                    ev.prevent_default();
                    set_toggle_play_pressed.update(|n| *n += 1);
                }
                _ => {}
            }
        });

    let window = web_sys::window().expect("no global window");
    let document = window.document().expect("no document");
    document
        .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
        .expect("failed to add keydown listener");
    closure.forget();
}
