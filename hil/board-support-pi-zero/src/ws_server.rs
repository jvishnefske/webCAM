//! WebSocket server for CBOR I2C and DAP dispatch.
//!
//! Accepts binary WebSocket connections on the configured port and
//! forwards each binary frame through either the DAP processor
//! (for CBOR tag 40 messages) or
//! [`handle_request`](hil_firmware_support::ws_dispatch::handle_request)
//! (for I2C messages), which decodes the CBOR request, performs the
//! operation, and returns a CBOR-encoded response.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use dap_dispatch::cbor_dispatch;
use dap_dispatch::protocol::DapProcessor;
use hil_firmware_support::ws_dispatch::handle_request;

use crate::combined_buses::CombinedBusSet;

/// Runs the WebSocket server, accepting connections on `0.0.0.0:{port}`.
///
/// Each connection is handled in a separate tokio task. Binary frames
/// are dispatched through either the DAP processor (tag 40) or
/// [`handle_request`] (all other tags). Text and other frame types
/// are ignored. The server runs until the future is dropped (typically
/// via task abort on shutdown).
///
/// If `dap` is `None` and a tag-40 request arrives, an error is logged
/// and no response is sent.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind.
pub async fn run(
    buses: Arc<Mutex<CombinedBusSet>>,
    dap: Option<Arc<Mutex<Box<dyn DapProcessor + Send>>>>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    log::info!("WebSocket server listening on port {port}");

    loop {
        let (stream, addr) = listener.accept().await?;
        log::debug!("New connection from {addr}");
        let buses = buses.clone();
        let dap = dap.clone();

        tokio::spawn(async move {
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    log::error!("WebSocket handshake failed for {addr}: {e}");
                    return;
                }
            };

            let (mut sink, mut stream) = ws.split();

            while let Some(msg) = stream.next().await {
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        log::debug!("WebSocket error from {addr}: {e}");
                        break;
                    }
                };

                match msg {
                    Message::Binary(data) => {
                        let mut resp_buf = [0u8; 4096];
                        let result = dispatch_request(
                            &buses,
                            &dap,
                            &data,
                            &mut resp_buf,
                        );
                        match result {
                            Ok(n) => {
                                if sink
                                    .send(Message::Binary(resp_buf[..n].to_vec().into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(()) => {
                                log::error!("Failed to handle request from {addr}");
                            }
                        }
                    }
                    Message::Ping(data) => {
                        if sink.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            log::debug!("Connection closed: {addr}");
        });
    }
}

/// Dispatches a binary WebSocket frame to either the DAP processor or
/// the I2C bus handler based on the CBOR tag.
///
/// Tag 40 frames are routed to the DAP processor; all other tags go
/// to the I2C bus handler via [`handle_request`].
///
/// # Errors
///
/// Returns `Err(())` if the request cannot be processed.
fn dispatch_request(
    buses: &Arc<Mutex<CombinedBusSet>>,
    dap: &Option<Arc<Mutex<Box<dyn DapProcessor + Send>>>>,
    data: &[u8],
    resp_buf: &mut [u8],
) -> Result<usize, ()> {
    if let Some(tag) = cbor_dispatch::peek_cbor_tag(data) {
        if cbor_dispatch::is_dap_tag(tag) {
            return match dap {
                Some(dap_mutex) => {
                    let mut guard = dap_mutex.lock().unwrap_or_else(|e| e.into_inner());
                    cbor_dispatch::handle_dap_request(&mut **guard, data, resp_buf)
                }
                None => {
                    log::error!("DAP not enabled; ignoring tag-40 request");
                    Err(())
                }
            };
        }
    }

    let mut guard = buses.lock().unwrap_or_else(|e| e.into_inner());
    handle_request(&mut *guard, data, resp_buf)
}
