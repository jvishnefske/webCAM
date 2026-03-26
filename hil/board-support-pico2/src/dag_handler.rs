//! DAG executor and HTTP API handler for the Pico 2.
//!
//! Handles POST /api/dag (CBOR upload), GET /api/status, POST /api/tick,
//! POST /api/debug, GET /api/pubsub, and GET /api/channels.

use dag_core::cbor;
use dag_core::eval::{NullChannels, PubSubReader};
use dag_core::op::Dag;
use heapless::FnvIndexMap;
use hil_firmware_support::ws_server::ApiHandler;

/// PubSub adapter backed by an `FnvIndexMap`.
///
/// Implements `PubSubReader` so the DAG evaluator can read published topics.
pub struct MapPubSub<'a> {
    map: &'a FnvIndexMap<heapless::String<32>, f64, 64>,
}

impl<'a> PubSubReader for MapPubSub<'a> {
    fn read(&self, topic: &str) -> f64 {
        let key = if let Ok(k) = heapless::String::<32>::try_from(topic) {
            k
        } else {
            return 0.0;
        };
        self.map.get(&key).copied().unwrap_or(0.0)
    }
}

/// DAG executor state held in the firmware.
pub struct DagApiHandler {
    dag: Option<Dag>,
    values: [f64; 128], // Fixed-size evaluation buffer (max 128 nodes)
    tick_count: u64,
    debug_mode: bool,
    pubsub_topics: FnvIndexMap<heapless::String<32>, f64, 64>,
    known_inputs: heapless::Vec<heapless::String<32>, 16>,
    known_outputs: heapless::Vec<heapless::String<32>, 16>,
}

impl DagApiHandler {
    pub fn new() -> Self {
        DagApiHandler {
            dag: None,
            values: [0.0; 128],
            tick_count: 0,
            debug_mode: false,
            pubsub_topics: FnvIndexMap::new(),
            known_inputs: heapless::Vec::new(),
            known_outputs: heapless::Vec::new(),
        }
    }

    /// Register a channel name as an input (e.g. "adc0").
    pub fn register_input(&mut self, name: &str) {
        if let Ok(s) = heapless::String::<32>::try_from(name) {
            let _ = self.known_inputs.push(s);
        }
    }

    /// Register a channel name as an output (e.g. "pwm0").
    pub fn register_output(&mut self, name: &str) {
        if let Ok(s) = heapless::String::<32>::try_from(name) {
            let _ = self.known_outputs.push(s);
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
                        let pubsub = MapPubSub {
                            map: &self.pubsub_topics,
                        };
                        let result = dag.evaluate(
                            &NullChannels,
                            &pubsub,
                            &mut self.values[..len],
                        );
                        // Store published topics
                        for (topic, value) in &result.publishes {
                            if let Ok(key) = heapless::String::<32>::try_from(topic.as_str()) {
                                let _ = self.pubsub_topics.insert(key, *value);
                            }
                        }
                        // In debug mode, write _dbg/<index> topics for each node value
                        if self.debug_mode {
                            for i in 0..len {
                                let mut key = heapless::String::<32>::new();
                                let _ = key.push_str("_dbg/");
                                write_usize_to_heapless_string(&mut key, i);
                                let _ = self.pubsub_topics.insert(key, self.values[i]);
                            }
                        }
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
            ("POST", "/api/debug") => {
                self.debug_mode = !self.debug_mode;
                let mut resp = heapless::Vec::new();
                let body_str = if self.debug_mode {
                    b"{\"debug\":true}" as &[u8]
                } else {
                    b"{\"debug\":false}" as &[u8]
                };
                let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
                write_usize_to_vec(&mut resp, body_str.len());
                let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                let _ = resp.extend_from_slice(body_str);
                Some(resp)
            }
            ("GET", "/api/pubsub") => {
                let mut resp = heapless::Vec::new();
                let mut json_buf = [0u8; 400];
                let json_len = write_pubsub_json(&mut json_buf, &self.pubsub_topics);

                let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
                write_usize_to_vec(&mut resp, json_len);
                let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                let _ = resp.extend_from_slice(&json_buf[..json_len]);
                Some(resp)
            }
            ("GET", "/api/channels") => {
                let mut resp = heapless::Vec::new();
                let mut json_buf = [0u8; 256];
                let json_len = write_channels_json(
                    &mut json_buf,
                    &self.known_inputs,
                    &self.known_outputs,
                );

                let _ = resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ");
                write_usize_to_vec(&mut resp, json_len);
                let _ = resp.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
                let _ = resp.extend_from_slice(&json_buf[..json_len]);
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

/// Write an f64 to a buffer as fixed-point with up to 4 decimal places,
/// trailing zeros trimmed. Returns the number of bytes written.
fn write_f64_to_buf(buf: &mut [u8], value: f64) -> usize {
    let mut pos = 0;

    if value < 0.0 {
        buf[pos] = b'-';
        pos += 1;
        return pos + write_f64_to_buf(&mut buf[pos..], -value);
    }

    // Split into integer and fractional parts
    let integer_part = value as u64;
    let frac = value - integer_part as f64;

    // Write integer part
    pos += write_u64_to_buf(&mut buf[pos..], integer_part);

    // Fractional part: up to 4 decimal places, trim trailing zeros
    let frac_scaled = (frac * 10000.0 + 0.5) as u32;
    if frac_scaled > 0 && frac_scaled < 10000 {
        buf[pos] = b'.';
        pos += 1;
        // Write exactly 4 digits, then trim trailing zeros
        let d3 = (frac_scaled / 1000) % 10;
        let d2 = (frac_scaled / 100) % 10;
        let d1 = (frac_scaled / 10) % 10;
        let d0 = frac_scaled % 10;
        let digits = [d3 as u8, d2 as u8, d1 as u8, d0 as u8];
        // Find last non-zero digit
        let mut last = 3;
        while last > 0 && digits[last] == 0 {
            last -= 1;
        }
        for d in &digits[..=last] {
            buf[pos] = b'0' + d;
            pos += 1;
        }
    }

    pos
}

fn write_u64_to_buf(buf: &mut [u8], mut n: u64) -> usize {
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

fn write_usize_to_heapless_string(s: &mut heapless::String<32>, mut n: usize) {
    if n == 0 {
        let _ = s.push('0');
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
        let _ = s.push(digits[i] as char);
    }
}

/// Write pubsub topics as JSON: `{"key":value,...}`
fn write_pubsub_json(
    buf: &mut [u8],
    topics: &FnvIndexMap<heapless::String<32>, f64, 64>,
) -> usize {
    let mut pos = 0;
    buf[pos] = b'{';
    pos += 1;

    let mut first = true;
    for (key, value) in topics.iter() {
        if !first {
            buf[pos] = b',';
            pos += 1;
        }
        first = false;
        buf[pos] = b'"';
        pos += 1;
        let key_bytes = key.as_bytes();
        buf[pos..pos + key_bytes.len()].copy_from_slice(key_bytes);
        pos += key_bytes.len();
        buf[pos] = b'"';
        pos += 1;
        buf[pos] = b':';
        pos += 1;
        pos += write_f64_to_buf(&mut buf[pos..], *value);
    }

    buf[pos] = b'}';
    pos += 1;
    pos
}

/// Write channels as JSON: `{"inputs":["a","b"],"outputs":["c","d"]}`
fn write_channels_json(
    buf: &mut [u8],
    inputs: &heapless::Vec<heapless::String<32>, 16>,
    outputs: &heapless::Vec<heapless::String<32>, 16>,
) -> usize {
    let mut pos = 0;
    let prefix = b"{\"inputs\":";
    buf[pos..pos + prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();
    pos += write_string_array(&mut buf[pos..], inputs);
    let mid = b",\"outputs\":";
    buf[pos..pos + mid.len()].copy_from_slice(mid);
    pos += mid.len();
    pos += write_string_array(&mut buf[pos..], outputs);
    buf[pos] = b'}';
    pos += 1;
    pos
}

/// Write a heapless Vec of strings as a JSON array: `["a","b","c"]`
fn write_string_array(
    buf: &mut [u8],
    items: &heapless::Vec<heapless::String<32>, 16>,
) -> usize {
    let mut pos = 0;
    buf[pos] = b'[';
    pos += 1;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buf[pos] = b',';
            pos += 1;
        }
        buf[pos] = b'"';
        pos += 1;
        let bytes = item.as_bytes();
        buf[pos..pos + bytes.len()].copy_from_slice(bytes);
        pos += bytes.len();
        buf[pos] = b'"';
        pos += 1;
    }
    buf[pos] = b']';
    pos += 1;
    pos
}
