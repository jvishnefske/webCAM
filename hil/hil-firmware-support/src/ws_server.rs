//! Generic HTTP and WebSocket server loop for the HIL dashboard.
//!
//! Accepts TCP connections on port 8080 and routes them based on the
//! initial HTTP request. WebSocket upgrade requests enter a CBOR frame
//! processing loop for I2C bus control and firmware updates. Regular
//! HTTP GET requests are served gzip-compressed static frontend assets.
//!
//! Board binaries provide their bus implementation via [`I2cBusSet`],
//! static assets via [`StaticAssets`], and DFU flash access via
//! [`DfuFlashWriter`].

use embassy_net::Stack;

use crate::fw_update::{self, DfuFlashWriter, FwUpdateState};
use crate::http_static::StaticAssets;
use crate::ws_dispatch::I2cBusSet;

/// Handler for HTTP API requests (POST endpoints, etc.).
///
/// Board binaries implement this to handle REST-style API calls
/// alongside the WebSocket server. The default implementation
/// returns 404 for all requests.
pub trait ApiHandler {
    /// Handle an HTTP request that is not a WebSocket upgrade or static file.
    ///
    /// `method` is "POST", "PUT", etc. `path` is the URL path.
    /// `body` is the request body (may be empty for GET).
    /// Returns the HTTP response bytes to send back.
    fn handle(&mut self, method: &str, path: &str, body: &[u8]) -> Option<heapless::Vec<u8, 512>>;
}

/// No-op API handler that rejects all requests.
pub struct NullApiHandler;

impl ApiHandler for NullApiHandler {
    fn handle(
        &mut self,
        _method: &str,
        _path: &str,
        _body: &[u8],
    ) -> Option<heapless::Vec<u8, 512>> {
        None
    }
}

/// Runs the HTTP and WebSocket server on port 8080.
///
/// Accepts one TCP connection at a time. Each connection is routed
/// by its HTTP request:
///
/// - **WebSocket upgrade**: performs the RFC 6455 handshake, then enters
///   a binary frame loop where CBOR-encoded requests are dispatched.
///   Tags 20-23 are routed to the firmware update handler; all other
///   tags go to the I2C bus dispatcher.
/// - **HTTP GET**: serves gzip-compressed static frontend assets from
///   the provided [`StaticAssets`], matched by file extension.
///
/// This function never returns (`-> !`). When a client disconnects, the
/// server waits briefly then accepts the next connection. Firmware
/// update state is reset on each new connection.
pub async fn run<B: I2cBusSet>(
    stack: Stack<'static>,
    buses: &mut B,
    assets: &StaticAssets,
    fw: &mut impl DfuFlashWriter,
) -> ! {
    run_with_api(stack, buses, assets, fw, &mut NullApiHandler).await
}

/// Like [`run`], but with an [`ApiHandler`] for POST/PUT endpoints.
pub async fn run_with_api<B: I2cBusSet>(
    stack: Stack<'static>,
    buses: &mut B,
    assets: &StaticAssets,
    fw: &mut impl DfuFlashWriter,
    api: &mut impl ApiHandler,
) -> ! {
    // Wait for network link
    stack.wait_config_up().await;
    defmt::info!("WS server: network up, starting on port 8080");

    loop {
        // Static buffers for TCP socket
        let mut rx_buf = [0u8; 2048];
        let mut tx_buf = [0u8; 2048];
        let mut socket = embassy_net::tcp::TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);

        if socket.accept(8080).await.is_err() {
            defmt::warn!("WS: accept failed");
            continue;
        }
        defmt::info!("WS: client connected");

        // Read HTTP upgrade request into a fixed buffer (1KB for modern browser headers)
        let mut http_buf = [0u8; 1024];
        let mut http_len = 0;
        let upgrade_result = 'upgrade: {
            // Read until we see \r\n\r\n (end of HTTP headers)
            loop {
                if http_len >= http_buf.len() {
                    defmt::warn!("WS: HTTP request too large (>{=usize}B)", http_buf.len());
                    break 'upgrade Err(());
                }
                let n = match socket.read(&mut http_buf[http_len..]).await {
                    Ok(0) | Err(_) => break 'upgrade Err(()),
                    Ok(n) => n,
                };
                http_len += n;

                // Check for end of headers (\r\n\r\n)
                if http_len >= 4 {
                    let mut i = 0;
                    while i <= http_len - 4 {
                        if http_buf[i] == b'\r'
                            && http_buf[i + 1] == b'\n'
                            && http_buf[i + 2] == b'\r'
                            && http_buf[i + 3] == b'\n'
                        {
                            break 'upgrade Ok(());
                        }
                        i += 1;
                    }
                }
            }
        };

        if upgrade_result.is_err() {
            defmt::warn!("WS: failed to read HTTP request headers");
            let _ = socket.flush().await;
            socket.close();
            continue;
        }

        // Route: WebSocket upgrade, API handler, or static file
        let key = match crate::ws_framing::parse_upgrade_key(&http_buf[..http_len]) {
            Some(k) => k,
            None => {
                let method =
                    crate::ws_framing::parse_request_method(&http_buf[..http_len]).unwrap_or("GET");
                let path =
                    crate::ws_framing::parse_request_path(&http_buf[..http_len]).unwrap_or("/");

                // Try API handler first (for /api/* paths or non-GET methods)
                let is_api = method != "GET" || path.starts_with("/api/");
                if is_api {
                    // API request — extract body and delegate
                    let header_end = {
                        let mut pos = None;
                        let mut i = 0;
                        while i + 3 < http_len {
                            if http_buf[i] == b'\r'
                                && http_buf[i + 1] == b'\n'
                                && http_buf[i + 2] == b'\r'
                                && http_buf[i + 3] == b'\n'
                            {
                                pos = Some(i + 4);
                                break;
                            }
                            i += 1;
                        }
                        pos.unwrap_or(http_len)
                    };
                    let body = &http_buf[header_end..http_len];

                    defmt::info!(
                        "HTTP: {=str} {=str} body={=usize}B",
                        method,
                        path,
                        body.len()
                    );

                    if let Some(resp) = api.handle(method, path, body) {
                        if crate::ws_framing::write_all_to_socket(&mut socket, &resp)
                            .await
                            .is_err()
                        {
                            defmt::warn!("HTTP: failed to send API response");
                        }
                    } else {
                        let not_found = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ =
                            crate::ws_framing::write_all_to_socket(&mut socket, not_found).await;
                    }
                } else {
                    defmt::info!("HTTP: GET {=str}", path);
                    if crate::http_static::serve_static(&mut socket, path, assets)
                        .await
                        .is_err()
                    {
                        defmt::warn!("HTTP: failed to serve static file");
                    }
                }
                let _ = socket.flush().await;
                socket.close();
                embassy_time::Timer::after_millis(10).await;
                continue;
            }
        };

        let mut resp_buf = [0u8; 256];
        let resp_len = crate::ws_framing::build_upgrade_response(&key, &mut resp_buf);
        if crate::ws_framing::write_all_to_socket(&mut socket, &resp_buf[..resp_len])
            .await
            .is_err()
        {
            defmt::warn!("WS: failed to send upgrade response");
            socket.close();
            continue;
        }
        defmt::info!("WS: upgrade complete");

        // Reset firmware update state for each new connection
        let mut fw_state = FwUpdateState::Idle;

        // WebSocket frame loop
        let mut frame_buf = [0u8; 1500];
        let mut resp_cbor_buf = [0u8; 1024];
        loop {
            let frame = match crate::ws_framing::read_frame(&mut socket, &mut frame_buf).await {
                Ok(f) => f,
                Err(crate::ws_framing::WsError::Closed) => {
                    defmt::info!("WS: client disconnected");
                    break;
                }
                Err(_) => {
                    defmt::warn!("WS: frame read error");
                    break;
                }
            };

            match frame.opcode {
                // Binary frame -- CBOR request
                0x02 => {
                    // Route by CBOR tag: firmware update (20-23) or I2C dispatch
                    let result = match fw_update::peek_tag(frame.payload) {
                        Some(tag) if fw_update::is_fw_tag(tag) => {
                            match fw_update::handle_fw_request(
                                fw_state.clone(),
                                fw,
                                frame.payload,
                                &mut resp_cbor_buf,
                            ) {
                                Ok((new_state, n)) => {
                                    // Check if we need to reset after FwFinish
                                    let needs_reset =
                                        matches!(new_state, FwUpdateState::Complete) && tag == 22;
                                    fw_state = new_state;

                                    if needs_reset {
                                        // Send ack, then reset
                                        let _ = crate::ws_framing::write_frame(
                                            &mut socket,
                                            0x02,
                                            &resp_cbor_buf[..n],
                                        )
                                        .await;
                                        // Brief delay to let the ack flush
                                        embassy_time::Timer::after_millis(100).await;
                                        fw.system_reset();
                                    }
                                    Ok(n)
                                }
                                Err(()) => {
                                    defmt::warn!("WS: fw update error");
                                    crate::ws_dispatch::encode_error(
                                        &mut resp_cbor_buf,
                                        "fw update error",
                                    )
                                }
                            }
                        }
                        _ => crate::ws_dispatch::handle_request::<B>(
                            buses,
                            frame.payload,
                            &mut resp_cbor_buf,
                        ),
                    };

                    match result {
                        Ok(n) => {
                            if crate::ws_framing::write_frame(
                                &mut socket,
                                0x02,
                                &resp_cbor_buf[..n],
                            )
                            .await
                            .is_err()
                            {
                                defmt::warn!("WS: write error");
                                break;
                            }
                        }
                        Err(()) => {
                            defmt::warn!("WS: dispatch error");
                        }
                    }
                }
                // Ping -- reply with pong
                0x09 => {
                    if crate::ws_framing::write_frame(&mut socket, 0x0A, frame.payload)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                // Close
                0x08 => {
                    let _ = crate::ws_framing::write_frame(&mut socket, 0x08, &[]).await;
                    break;
                }
                // Text frame or unknown -- ignore
                _ => {}
            }
        }

        socket.close();
        // Small delay before accepting next connection
        embassy_time::Timer::after_millis(100).await;
    }
}
