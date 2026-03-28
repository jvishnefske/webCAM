//! WebSocket client for communicating with the HIL Pico over CBOR.
//!
//! Provides a [`WsClient`] wrapper around `web_sys::WebSocket` that handles
//! binary CBOR framing, connection lifecycle, and automatic reconnection
//! with exponential backoff.
//!
//! The WebSocket is stored in a thread-local `RefCell` because
//! `web_sys::WebSocket` is `!Send` and cannot be placed in Leptos signals.
//! This is safe because WASM is single-threaded.
//!
//! # Connection Lifecycle
//!
//! 1. [`connect`] creates a new WebSocket and stores it in the thread-local.
//! 2. `onopen` transitions state to [`ConnState::Open`].
//! 3. `onmessage` decodes incoming CBOR binary frames via [`messages::decode_response`].
//! 4. `onclose` transitions state to [`ConnState::Closed`] and invokes the
//!    reconnect callback.
//! 5. Reconnect uses exponential backoff: 1s, 2s, 4s, ... up to 30s max.

use std::cell::RefCell;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, MessageEvent, WebSocket};

use crate::messages::{self, Request, Response};

pub use crate::backoff::{initial_backoff, next_backoff};

/// Connection state for the WebSocket link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    /// Attempting to establish connection.
    Connecting,
    /// Connection is active and ready for messages.
    Open,
    /// Connection is closed (will auto-reconnect).
    Closed,
}

thread_local! {
    /// Thread-local storage for the active WebSocket connection.
    ///
    /// This is used because `web_sys::WebSocket` is `!Send` and cannot be
    /// stored in Leptos signals. WASM is single-threaded, so `thread_local!`
    /// effectively acts as a global.
    static WS: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

/// Send a [`Request`] to the Pico, encoding it as CBOR binary.
///
/// Returns `Ok(())` if the message was queued, or `Err` with a description
/// if no WebSocket is connected or the send fails.
pub fn send_request(req: &Request) -> Result<(), String> {
    let data = messages::encode_request(req);
    WS.with(|ws| {
        let ws = ws.borrow();
        match ws.as_ref() {
            Some(socket) => socket
                .send_with_u8_array(&data)
                .map_err(|e| format!("{e:?}")),
            None => Err("Not connected".to_string()),
        }
    })
}

/// Establish a WebSocket connection and wire up callbacks.
///
/// The WebSocket is stored in a thread-local so that [`send_request`] can
/// access it without needing a reference to a non-Send type.
///
/// # Arguments
///
/// * `url` - WebSocket URL, e.g. `ws://169.254.1.61:8080`.
/// * `state_set` - Signal setter for connection state updates.
/// * `on_response` - Callback invoked for each decoded [`Response`].
/// * `on_close` - Callback invoked when the connection closes (for reconnect scheduling).
pub fn connect(
    url: &str,
    state_set: WriteSignal<ConnState>,
    on_response: impl Fn(Response) + 'static,
    on_close: impl Fn() + 'static,
) {
    let ws = match WebSocket::new(url) {
        Ok(ws) => ws,
        Err(e) => {
            web_sys::console::error_1(&format!("WebSocket::new failed: {e:?}").into());
            state_set.set(ConnState::Closed);
            on_close();
            return;
        }
    };
    ws.set_binary_type(BinaryType::Arraybuffer);
    state_set.set(ConnState::Connecting);

    // onopen
    let state_set_open = state_set;
    let onopen = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        state_set_open.set(ConnState::Open);
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // onmessage
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let data = event.data();
        if let Ok(buf) = data.dyn_into::<js_sys::ArrayBuffer>() {
            let arr = js_sys::Uint8Array::new(&buf);
            let mut bytes = vec![0u8; arr.length() as usize];
            arr.copy_to(&mut bytes);
            match messages::decode_response(&bytes) {
                Ok(resp) => on_response(resp),
                Err(e) => {
                    web_sys::console::warn_1(&format!("CBOR decode error: {e}").into());
                }
            }
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // onclose
    let onclose = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        state_set.set(ConnState::Closed);
        WS.with(|cell| {
            *cell.borrow_mut() = None;
        });
        on_close();
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    // onerror
    let onerror = Closure::<dyn FnMut(JsValue)>::new(move |e: JsValue| {
        web_sys::console::error_1(&format!("WebSocket error: {e:?}").into());
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    // Store in thread-local
    WS.with(|cell| {
        *cell.borrow_mut() = Some(ws);
    });
}
