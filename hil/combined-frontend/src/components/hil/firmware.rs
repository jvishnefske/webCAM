//! Firmware update panel — OTA upload with chunked transfer and progress.
//!
//! Provides a file picker for `.bin` firmware images, computes CRC32, and
//! drives a chunked upload protocol over WebSocket using the CBOR message
//! types defined in [`crate::messages`].
//!
//! The upload state machine is driven reactively: an [`Effect`] watches the
//! `fw_response` signal from [`AppContext`] and advances through the protocol
//! states (begin -> ready -> chunk/ack loop -> finish -> complete).

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::app::AppContext;
use crate::firmware_util::{firmware_crc32, DEFAULT_CHUNK_SIZE};
use crate::messages::{Request, Response};

// ---------------------------------------------------------------------------
// Upload state machine
// ---------------------------------------------------------------------------

/// States of the firmware upload protocol.
#[derive(Clone, PartialEq)]
enum UploadState {
    /// No file selected, waiting for user action.
    Idle,
    /// File loaded from disk; displaying info, ready to upload.
    FileSelected {
        name: String,
        size: u32,
        crc32: u32,
        data: Vec<u8>,
    },
    /// `FwBegin` sent, waiting for `FwReady`.
    WaitingReady {
        size: u32,
        crc32: u32,
        data: Vec<u8>,
    },
    /// Actively sending chunks and awaiting acks.
    Uploading {
        offset: u32,
        total: u32,
        crc32: u32,
        chunk_size: u32,
        data: Vec<u8>,
    },
    /// All chunks sent, `FwFinish` sent, waiting for `FwFinishAck`.
    WaitingFinish,
    /// Upload completed successfully.
    Complete,
    /// An error occurred; the message is displayed and retry is possible.
    Error(String),
}

impl UploadState {
    fn status_text(&self) -> String {
        match self {
            Self::Idle => "Select a firmware file".to_string(),
            Self::FileSelected { .. } => "Ready to upload".to_string(),
            Self::WaitingReady { .. } => "Preparing...".to_string(),
            Self::Uploading { offset, total, .. } => {
                let pct = if *total > 0 {
                    (*offset as f64 / *total as f64 * 100.0) as u32
                } else {
                    0
                };
                format!("Uploading ({pct}%)")
            }
            Self::WaitingFinish => "Verifying...".to_string(),
            Self::Complete => "Complete!".to_string(),
            Self::Error(msg) => format!("Error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Firmware update panel with file picker, chunked OTA upload, and progress.
#[component]
pub fn FirmwarePanel() -> impl IntoView {
    let ctx = use_context::<AppContext>().unwrap();

    let (state, set_state) = signal(UploadState::Idle);

    // --- File input ref ---
    let file_input = NodeRef::<leptos::html::Input>::new();

    // --- Handle file selection ---
    let on_file_change = move |_ev: web_sys::Event| {
        let input: web_sys::HtmlInputElement = file_input.get().unwrap();
        let files = match input.files() {
            Some(f) => f,
            None => return,
        };
        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };

        let file_name = file.name();
        let reader = web_sys::FileReader::new().expect("FileReader::new");
        let reader_clone = reader.clone();
        let set_state_clone = set_state;

        let onload = Closure::<dyn FnMut(web_sys::ProgressEvent)>::new(
            move |_ev: web_sys::ProgressEvent| {
                let result = reader_clone.result().expect("FileReader result");
                let array_buf = result
                    .dyn_into::<js_sys::ArrayBuffer>()
                    .expect("ArrayBuffer");
                let uint8 = js_sys::Uint8Array::new(&array_buf);
                let mut data = vec![0u8; uint8.length() as usize];
                uint8.copy_to(&mut data);

                let crc32 = firmware_crc32(&data);
                let size = data.len() as u32;
                let name = file_name.clone();

                set_state_clone.set(UploadState::FileSelected {
                    name,
                    size,
                    crc32,
                    data,
                });
            },
        );

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        reader
            .read_as_array_buffer(&file)
            .expect("read_as_array_buffer");
    };

    // Capture only Copy signal handles so the closures below are Copy,
    // allowing them to be used inside reactive view closures (FnMut).
    let request_tx = ctx.request_tx;

    let start_upload = move || {
        let current = state.get_untracked();
        if let UploadState::FileSelected {
            size, crc32, data, ..
        } = current
        {
            request_tx.update(|q| {
                q.push(Request::FwBegin {
                    total_size: size,
                    crc32,
                })
            });
            set_state.set(UploadState::WaitingReady { size, crc32, data });
        }
    };

    let mark_booted = move || {
        request_tx.update(|q| q.push(Request::FwMarkBooted));
    };

    let reset_state = move || {
        set_state.set(UploadState::Idle);
    };

    // --- Reactive state machine: advance on fw_response changes ---
    let fw_response = ctx.fw_response;
    Effect::new(move |_| {
        let resp = fw_response.get();
        let resp = match resp {
            Some(r) => r,
            None => return,
        };

        let current = state.get_untracked();

        match (&current, &resp) {
            // FwReady received while waiting: start sending chunks.
            (UploadState::WaitingReady { data, crc32, .. }, Response::FwReady { max_chunk }) => {
                let chunk_size = if *max_chunk == 0 {
                    DEFAULT_CHUNK_SIZE as u32
                } else {
                    u32::from(*max_chunk)
                };
                let total = data.len() as u32;
                let crc32 = *crc32;

                // Send first chunk.
                let end = (chunk_size as usize).min(data.len());
                let chunk_data = data[..end].to_vec();
                request_tx.update(|q| {
                    q.push(Request::FwChunk {
                        offset: 0,
                        data: chunk_data,
                    })
                });

                set_state.set(UploadState::Uploading {
                    offset: 0,
                    total,
                    crc32,
                    chunk_size,
                    data: data.clone(),
                });
            }

            // FwChunkAck received while uploading: send next chunk or finish.
            (
                UploadState::Uploading {
                    total,
                    crc32,
                    chunk_size,
                    data,
                    ..
                },
                Response::FwChunkAck { next_offset },
            ) => {
                let next = *next_offset;
                if next >= *total {
                    // All chunks sent; finish.
                    request_tx.update(|q| q.push(Request::FwFinish { crc32: *crc32 }));
                    set_state.set(UploadState::WaitingFinish);
                } else {
                    // Send next chunk.
                    let end = ((next + *chunk_size) as usize).min(data.len());
                    let chunk_data = data[next as usize..end].to_vec();
                    request_tx.update(|q| {
                        q.push(Request::FwChunk {
                            offset: next,
                            data: chunk_data,
                        })
                    });
                    set_state.set(UploadState::Uploading {
                        offset: next,
                        total: *total,
                        crc32: *crc32,
                        chunk_size: *chunk_size,
                        data: data.clone(),
                    });
                }
            }

            // FwFinishAck received while waiting for finish.
            (UploadState::WaitingFinish, Response::FwFinishAck) => {
                set_state.set(UploadState::Complete);
            }

            // Error received at any active upload state.
            (
                UploadState::WaitingReady { .. }
                | UploadState::Uploading { .. }
                | UploadState::WaitingFinish,
                Response::Error { message },
            ) => {
                set_state.set(UploadState::Error(message.clone()));
            }

            _ => {}
        }
    });

    // --- View ---
    view! {
        <h2 class="section-title">"Firmware Update"</h2>
        <div class="card">
            <div class="card-title">"OTA Update"</div>

            // File picker
            <div style="margin-bottom: 0.75rem;">
                <input
                    type="file"
                    accept=".bin"
                    node_ref=file_input
                    on:change=on_file_change
                />
            </div>

            // File info
            {move || {
                let st = state.get();
                match &st {
                    UploadState::FileSelected { name, size, crc32, .. } => {
                        view! {
                            <div class="info-box">
                                <p><strong>"File: "</strong>{name.clone()}</p>
                                <p><strong>"Size: "</strong>{format!("{size} bytes")}</p>
                                <p><strong>"CRC32: "</strong>{format!("0x{crc32:08X}")}</p>
                            </div>
                        }.into_any()
                    }
                    _ => view! { <span></span> }.into_any(),
                }
            }}

            // Progress bar
            {move || {
                let st = state.get();
                match &st {
                    UploadState::Uploading { offset, total, chunk_size, .. } => {
                        // Show progress including the chunk currently in flight.
                        let sent = (*offset + *chunk_size).min(*total);
                        view! {
                            <div style="margin: 0.75rem 0;">
                                <progress
                                    value=sent.to_string()
                                    max=total.to_string()
                                    style="width: 100%; height: 1.25rem;"
                                ></progress>
                            </div>
                        }.into_any()
                    }
                    UploadState::WaitingFinish | UploadState::Complete => {
                        view! {
                            <div style="margin: 0.75rem 0;">
                                <progress value="1" max="1" style="width: 100%; height: 1.25rem;"></progress>
                            </div>
                        }.into_any()
                    }
                    _ => view! { <span></span> }.into_any(),
                }
            }}

            // Status message
            <p class="card-subtitle">{move || state.get().status_text()}</p>

            // Action buttons
            <div style="display: flex; gap: 0.5rem; margin-top: 0.75rem;">
                {move || {
                    let st = state.get();
                    match &st {
                        UploadState::FileSelected { .. } => {
                            view! {
                                <button class="btn" on:click=move |_| start_upload()>"Upload"</button>
                            }.into_any()
                        }
                        UploadState::Complete => {
                            view! {
                                <button class="btn" on:click=move |_| mark_booted()>"Mark Booted"</button>
                                <button class="btn" on:click=move |_| reset_state()>"Reset"</button>
                            }.into_any()
                        }
                        UploadState::Error(_) => {
                            view! {
                                <button class="btn" on:click=move |_| reset_state()>"Retry"</button>
                            }.into_any()
                        }
                        _ => view! { <span></span> }.into_any(),
                    }
                }}
            </div>
        </div>
    }
}
