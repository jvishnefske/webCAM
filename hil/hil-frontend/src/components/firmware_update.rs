//! Firmware update panel with A/B OTA upload over WebSocket.
//!
//! Provides a file picker for `.bin` firmware images, computes CRC32,
//! streams chunks with ack-based flow control, and displays upload progress.
//! Falls back to BOOTSEL reboot for manual flashing.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::messages::{Request, Response};

/// Upload state machine displayed to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UploadStatus {
    /// No upload in progress.
    Idle,
    /// File loaded, ready to upload.
    Ready {
        /// File name.
        name: String,
        /// File size in bytes.
        size: u32,
        /// CRC32 of the file contents.
        crc32: u32,
    },
    /// Erasing DFU partition.
    Erasing,
    /// Uploading firmware chunks.
    Uploading {
        /// Bytes sent so far.
        sent: u32,
        /// Total bytes.
        total: u32,
    },
    /// All chunks sent, verifying CRC32.
    Verifying,
    /// Update complete, device rebooting.
    Rebooting,
    /// Update completed successfully.
    Done,
    /// An error occurred.
    Error(String),
}

/// Firmware update panel component.
///
/// Provides OTA firmware upload over WebSocket with progress display.
/// Keeps BOOTSEL reboot as fallback.
#[component]
pub fn FirmwareUpdatePanel(
    /// Callback to send requests to the Pico.
    send: impl Fn(Request) + Copy + 'static,
    /// Signal carrying responses from the Pico for upload flow control.
    fw_response: ReadSignal<Option<Response>>,
) -> impl IntoView {
    let (status, set_status) = signal(UploadStatus::Idle);
    let (file_data, set_file_data) = signal(Option::<Vec<u8>>::None);

    // Handle file selection
    let on_file_change = move |ev: web_sys::Event| {
        let target = ev.target().expect("target");
        let input: web_sys::HtmlInputElement = target.unchecked_into();
        let files = match input.files() {
            Some(f) => f,
            None => return,
        };
        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };
        let name = file.name();
        let reader = web_sys::FileReader::new().expect("FileReader");
        let reader_clone = reader.clone();
        let onload = Closure::<dyn FnMut(web_sys::ProgressEvent)>::new(
            move |_evt: web_sys::ProgressEvent| {
                let result = reader_clone.result().expect("result");
                let buf = js_sys::Uint8Array::new(&result);
                let mut bytes = vec![0u8; buf.length() as usize];
                buf.copy_to(&mut bytes);

                let crc = crc32fast::hash(&bytes);
                let size = bytes.len() as u32;
                let name = name.clone();

                set_file_data.set(Some(bytes));
                set_status.set(UploadStatus::Ready {
                    name,
                    size,
                    crc32: crc,
                });
            },
        );
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
        reader.read_as_array_buffer(&file).expect("read");
    };

    // Start upload
    let on_upload = move |_| {
        let data = file_data.get_untracked();
        let st = status.get_untracked();
        if let (Some(_data), UploadStatus::Ready { size, crc32, .. }) = (&data, &st) {
            set_status.set(UploadStatus::Erasing);
            send(Request::FwBegin {
                total_size: *size,
                crc32: *crc32,
            });
        }
    };

    // React to firmware responses for flow control
    Effect::new(move |_| {
        let resp = fw_response.get();
        if let Some(resp) = resp {
            match resp {
                Response::FwReady { max_chunk } => {
                    // DFU erased, start sending chunks
                    let data = file_data.get_untracked();
                    if let Some(ref bytes) = data {
                        let chunk_size = max_chunk as usize;
                        let total = bytes.len() as u32;
                        let end = if chunk_size > bytes.len() {
                            bytes.len()
                        } else {
                            chunk_size
                        };
                        set_status.set(UploadStatus::Uploading {
                            sent: end as u32,
                            total,
                        });
                        send(Request::FwChunk {
                            offset: 0,
                            data: bytes[..end].to_vec(),
                        });
                    }
                }
                Response::FwChunkAck { next_offset } => {
                    let data = file_data.get_untracked();
                    if let Some(ref bytes) = data {
                        let total = bytes.len() as u32;
                        if next_offset >= total {
                            // All chunks sent, finish
                            let crc = crc32fast::hash(bytes);
                            set_status.set(UploadStatus::Verifying);
                            send(Request::FwFinish { crc32: crc });
                        } else {
                            // Send next chunk (1024 bytes max)
                            let start = next_offset as usize;
                            let end = if start + 1024 > bytes.len() {
                                bytes.len()
                            } else {
                                start + 1024
                            };
                            set_status.set(UploadStatus::Uploading {
                                sent: end as u32,
                                total,
                            });
                            send(Request::FwChunk {
                                offset: next_offset,
                                data: bytes[start..end].to_vec(),
                            });
                        }
                    }
                }
                Response::FwFinishAck => {
                    set_status.set(UploadStatus::Rebooting);
                    set_file_data.set(None);
                    // Device will reset itself after sending this ack
                }
                Response::FwMarkBootedAck => {
                    set_status.set(UploadStatus::Done);
                }
                Response::Error { ref message } => {
                    set_status.set(UploadStatus::Error(message.clone()));
                }
                _ => {}
            }
        }
    });

    // BOOTSEL reboot fallback
    let on_reboot = move |_| {
        send(Request::RebootBootsel);
    };

    let status_view = move || {
        let st = status.get();
        match st {
            UploadStatus::Idle => view! {
                <div class="info-box">
                    <p>"Select a .bin firmware file to upload over WebSocket."</p>
                </div>
            }
            .into_any(),
            UploadStatus::Ready { name, size, .. } => view! {
                <div class="info-box">
                    <p>{format!("File: {name} ({size} bytes)")}</p>
                    <button class="btn btn-primary" on:click=on_upload>
                        "Upload Firmware"
                    </button>
                </div>
            }
            .into_any(),
            UploadStatus::Erasing => view! {
                <div class="info-box">
                    <p>"Erasing DFU partition..."</p>
                </div>
            }
            .into_any(),
            UploadStatus::Uploading { sent, total } => {
                let pct = if total > 0 {
                    (sent as f64 / total as f64 * 100.0) as u32
                } else {
                    0
                };
                view! {
                    <div class="info-box">
                        <p>{format!("Uploading: {sent}/{total} bytes ({pct}%)")}</p>
                        <div style="background: #333; border-radius: 4px; height: 20px; width: 100%; margin-top: 0.5rem;">
                            <div style={format!("background: #4CAF50; height: 100%; border-radius: 4px; width: {pct}%; transition: width 0.2s;")}></div>
                        </div>
                    </div>
                }
                .into_any()
            }
            UploadStatus::Verifying => view! {
                <div class="info-box">
                    <p>"Verifying CRC32..."</p>
                </div>
            }
            .into_any(),
            UploadStatus::Rebooting => view! {
                <div class="info-box">
                    <p>"Firmware update complete. Device is rebooting..."</p>
                    <p>"The page will reconnect automatically."</p>
                </div>
            }
            .into_any(),
            UploadStatus::Done => view! {
                <div class="info-box" style="border-color: #4CAF50;">
                    <p>"Firmware update successful! Device is running new firmware."</p>
                </div>
            }
            .into_any(),
            UploadStatus::Error(msg) => view! {
                <div class="info-box" style="border-color: #f44336;">
                    <p>{format!("Error: {msg}")}</p>
                </div>
            }
            .into_any(),
        }
    };

    view! {
        <div>
            <h2 class="section-title">"Firmware Update"</h2>

            <div class="card" style="max-width: 600px;">
                <div class="card-title">"OTA Update"</div>

                <div class="form-group">
                    <label>"Firmware Binary (.bin)"</label>
                    <input
                        type="file"
                        accept=".bin"
                        on:change=on_file_change
                    />
                </div>

                {status_view}
            </div>

            <div class="card" style="max-width: 600px; margin-top: 1rem;">
                <div class="card-title">"BOOTSEL Reboot (Fallback)"</div>
                <div class="info-box">
                    <p>"Reboot to BOOTSEL mode for manual UF2 flashing."</p>
                </div>
                <button class="btn btn-danger" on:click=on_reboot>
                    "Reboot to Bootloader"
                </button>
            </div>
        </div>
    }
}
