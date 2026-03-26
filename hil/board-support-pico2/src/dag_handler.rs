//! DAG executor and HTTP API handler for the Pico 2.
//!
//! Handles POST /api/dag (CBOR upload), GET /api/status, and POST /api/tick.

use dag_core::cbor;
use dag_core::eval::{NullChannels, NullPubSub};
use dag_core::op::Dag;
use hil_firmware_support::ws_server::ApiHandler;

/// DAG executor state held in the firmware.
pub struct DagApiHandler {
    dag: Option<Dag>,
    values: [f64; 128], // Fixed-size evaluation buffer (max 128 nodes)
    tick_count: u64,
}

impl DagApiHandler {
    pub const fn new() -> Self {
        DagApiHandler {
            dag: None,
            values: [0.0; 128],
            tick_count: 0,
        }
    }
}

impl ApiHandler for DagApiHandler {
    fn handle(
        &mut self,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Option<heapless::Vec<u8, 512>> {
        match (method, path) {
            ("POST", "/api/dag") => {
                let mut resp = heapless::Vec::new();
                match cbor::decode_dag(body) {
                    Ok(dag) => {
                        let node_count = dag.len();
                        // Clear values buffer
                        for v in self.values.iter_mut() {
                            *v = 0.0;
                        }
                        self.dag = Some(dag);
                        self.tick_count = 0;
                        defmt::info!("DAG loaded: {} nodes", node_count);

                        let msg = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ";
                        let _ = resp.extend_from_slice(msg);
                        // Build JSON body
                        let mut json_buf = [0u8; 64];
                        let json_len = write_json_ok(&mut json_buf, node_count);
                        write_usize_to_vec(&mut resp, json_len);
                        let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                        let _ = resp.extend_from_slice(&json_buf[..json_len]);
                    }
                    Err(_) => {
                        let body_str = b"{\"error\":\"invalid CBOR DAG\"}";
                        let _ = resp.extend_from_slice(b"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: ");
                        write_usize_to_vec(&mut resp, body_str.len());
                        let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                        let _ = resp.extend_from_slice(body_str);
                    }
                }
                Some(resp)
            }
            ("GET", "/api/status") => {
                let mut resp = heapless::Vec::new();
                let loaded = self.dag.is_some();
                let nodes = self.dag.as_ref().map_or(0, |d| d.len());
                let ticks = self.tick_count;

                let mut json_buf = [0u8; 128];
                let json_len = write_json_status(&mut json_buf, loaded, nodes, ticks);

                let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
                write_usize_to_vec(&mut resp, json_len);
                let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                let _ = resp.extend_from_slice(&json_buf[..json_len]);
                Some(resp)
            }
            ("POST", "/api/tick") => {
                let mut resp = heapless::Vec::new();
                if let Some(dag) = &self.dag {
                    let len = dag.len();
                    if len <= 128 {
                        dag.evaluate(
                            &NullChannels,
                            &NullPubSub,
                            &mut self.values[..len],
                        );
                        self.tick_count += 1;
                    }
                    let body_str = b"{\"ok\":true}";
                    let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
                    write_usize_to_vec(&mut resp, body_str.len());
                    let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                    let _ = resp.extend_from_slice(body_str);
                } else {
                    let body_str = b"{\"error\":\"no DAG loaded\"}";
                    let _ = resp.extend_from_slice(b"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: ");
                    write_usize_to_vec(&mut resp, body_str.len());
                    let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                    let _ = resp.extend_from_slice(body_str);
                }
                Some(resp)
            }
            _ => None,
        }
    }
}

fn write_usize_to_vec(v: &mut heapless::Vec<u8, 512>, mut n: usize) {
    if n == 0 {
        let _ = v.push(b'0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut len = 0;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in (0..len).rev() {
        let _ = v.push(digits[i]);
    }
}

fn write_json_ok(buf: &mut [u8], nodes: usize) -> usize {
    // {"ok":true,"nodes":NNN}
    let prefix = b"{\"ok\":true,\"nodes\":";
    let mut pos = prefix.len();
    buf[..pos].copy_from_slice(prefix);
    pos += write_usize_to_buf(&mut buf[pos..], nodes);
    buf[pos] = b'}';
    pos + 1
}

fn write_json_status(buf: &mut [u8], loaded: bool, nodes: usize, ticks: u64) -> usize {
    let prefix = if loaded {
        b"{\"loaded\":true,\"nodes\":" as &[u8]
    } else {
        b"{\"loaded\":false,\"nodes\":" as &[u8]
    };
    let mut pos = prefix.len();
    buf[..pos].copy_from_slice(prefix);
    pos += write_usize_to_buf(&mut buf[pos..], nodes);
    let mid = b",\"ticks\":";
    buf[pos..pos + mid.len()].copy_from_slice(mid);
    pos += mid.len();
    pos += write_usize_to_buf(&mut buf[pos..], ticks as usize);
    buf[pos] = b'}';
    pos + 1
}

fn write_usize_to_buf(buf: &mut [u8], mut n: usize) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut digits = [0u8; 20];
    let mut len = 0;
    while n > 0 {
        digits[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in 0..len {
        buf[i] = digits[len - 1 - i];
    }
    len
}
