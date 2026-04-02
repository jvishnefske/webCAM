//! Native DAG execution server with axum HTTP API and WebSocket I2C dispatch.
//!
//! Provides the same REST + WebSocket API as the Pico2 firmware:
//!
//! | Method | Path           | Purpose                        |
//! |--------|----------------|--------------------------------|
//! | POST   | /api/dag       | Deploy CBOR-encoded DAG        |
//! | POST   | /api/tick      | Evaluate DAG once              |
//! | GET    | /api/status    | DAG status (loaded/nodes/ticks) |
//! | GET    | /api/pubsub    | Snapshot all topic values       |
//! | GET    | /api/channels  | List registered I/O channels   |
//! | POST   | /api/debug     | Toggle debug topic publishing  |
//! | WS     | /ws            | CBOR-encoded I2C bus management |

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use dag_core::eval::{NullChannels, NullPubSub};
use dag_runtime::channels::MapChannels;
use dag_runtime::executor::DagExecutor;
use dag_runtime::pubsub::SimplePubSub;
use embedded_hal::i2c::I2c;
use i2c_hil_sim::{Address, RuntimeBus};
use tower_http::services::ServeDir;

/// Number of I2C buses managed by the server.
const BUS_COUNT: usize = 10;
/// Maximum devices per bus.
const MAX_DEVICES_PER_BUS: usize = 8;

/// Shared server state holding the DAG executor and I/O state.
pub struct ServerState {
    pub executor: DagExecutor,
    pub channels: MapChannels,
    pub pubsub: SimplePubSub,
    pub debug_mode: bool,
    pub known_inputs: Vec<String>,
    pub known_outputs: Vec<String>,
    /// Mirror of pubsub topics for JSON serialization, since
    /// `SimplePubSub` does not expose its internal map.
    pub pubsub_snapshot: HashMap<String, f64>,
    /// Simulated I2C buses for WebSocket dispatch.
    pub i2c_buses: Vec<RuntimeBus<MAX_DEVICES_PER_BUS>>,
}

impl ServerState {
    /// Create a new default server state with no DAG loaded.
    pub fn new() -> Self {
        ServerState {
            executor: DagExecutor::new(),
            channels: MapChannels::new(),
            pubsub: SimplePubSub::new(),
            debug_mode: false,
            known_inputs: Vec::new(),
            known_outputs: Vec::new(),
            pubsub_snapshot: HashMap::new(),
            i2c_buses: (0..BUS_COUNT).map(|_| RuntimeBus::new()).collect(),
        }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for the shared state handle passed to axum handlers.
pub type SharedState = Arc<Mutex<ServerState>>;

/// Build the axum router with DAG API routes and a static file fallback.
pub fn app(www_dir: &Path) -> Router {
    let state: SharedState = Arc::new(Mutex::new(ServerState::new()));
    Router::new()
        .route("/api/dag", post(post_dag))
        .route("/api/tick", post(post_tick))
        .route("/api/status", get(get_status))
        .route("/api/pubsub", get(get_pubsub))
        .route("/api/channels", get(get_channels))
        .route("/api/debug", post(post_debug))
        .route("/ws", any(ws_handler))
        .with_state(state)
        .fallback_service(ServeDir::new(www_dir))
}

/// POST /api/dag -- load a CBOR-encoded DAG into the executor.
async fn post_dag(State(state): State<SharedState>, body: Bytes) -> Json<serde_json::Value> {
    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
    match s.executor.load_cbor(&body) {
        Ok(()) => Json(serde_json::json!({
            "ok": true,
            "nodes": s.executor.node_count()
        })),
        Err(e) => Json(serde_json::json!({
            "error": format!("invalid CBOR DAG: {e}")
        })),
    }
}

/// POST /api/tick -- evaluate one tick of the loaded DAG.
async fn post_tick(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
    let ServerState {
        executor,
        channels,
        pubsub,
        debug_mode,
        pubsub_snapshot,
        ..
    } = &mut *s;

    let null_ch = NullChannels;
    let null_ps = NullPubSub;
    match executor.tick(&null_ch, channels, &null_ps, pubsub) {
        Some(eval_result) => {
            for (topic, value) in &eval_result.publishes {
                pubsub_snapshot.insert(topic.clone(), *value);
            }
            if *debug_mode {
                for (i, &v) in executor.values().iter().enumerate() {
                    pubsub_snapshot.insert(format!("_dbg/{i}"), v);
                }
            }
            Json(serde_json::json!({
                "ok": true,
                "outputs": eval_result.outputs.len(),
                "publishes": eval_result.publishes.len()
            }))
        }
        None => Json(serde_json::json!({
            "error": "no DAG loaded"
        })),
    }
}

/// GET /api/status -- return executor status.
async fn get_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.lock().unwrap_or_else(|e| e.into_inner());
    Json(serde_json::json!({
        "loaded": s.executor.is_loaded(),
        "nodes": s.executor.node_count(),
        "ticks": s.executor.tick_count()
    }))
}

/// GET /api/pubsub -- return all topic values as a JSON object.
async fn get_pubsub(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.lock().unwrap_or_else(|e| e.into_inner());
    let map: serde_json::Map<String, serde_json::Value> = s
        .pubsub_snapshot
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::json!(*v)))
        .collect();
    Json(serde_json::Value::Object(map))
}

/// GET /api/channels -- return known input and output channel names.
async fn get_channels(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.lock().unwrap_or_else(|e| e.into_inner());
    Json(serde_json::json!({
        "inputs": s.known_inputs,
        "outputs": s.known_outputs
    }))
}

/// POST /api/debug -- toggle debug mode and return current state.
async fn post_debug(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
    s.debug_mode = !s.debug_mode;
    Json(serde_json::json!({
        "debug": s.debug_mode
    }))
}

// ── WebSocket I2C dispatch ──────────────────────────────────────────

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: SharedState) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Binary(data) = msg {
            if let Some(resp) = dispatch_cbor(&state, &data) {
                let _ = socket.send(Message::Binary(resp.into())).await;
            }
        }
    }
}

/// Dispatch a CBOR-encoded I2C request and return the CBOR-encoded response.
pub fn dispatch_cbor(state: &SharedState, data: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = minicbor::Decoder::new(data);
    let _map_len = decoder.map().ok()??;
    let _key0 = decoder.u32().ok()?;
    let tag = decoder.u32().ok()?;

    match tag {
        3 => handle_list_buses(state),
        30 => handle_add_device(state, data),
        31 => handle_remove_device(state, data),
        1 => handle_i2c_read(state, data),
        2 => handle_i2c_write(state, data),
        _ => {
            let mut buf = Vec::new();
            let mut enc = minicbor::Encoder::new(&mut buf);
            enc.map(2).ok()?;
            enc.u32(0).ok()?.u32(255).ok()?;
            enc.u32(1).ok()?.str("unknown request tag").ok()?;
            Some(buf)
        }
    }
}

fn encode_error(message: &str) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut enc = minicbor::Encoder::new(&mut buf);
    enc.map(2).ok()?;
    enc.u32(0).ok()?.u32(255).ok()?;
    enc.u32(1).ok()?.str(message).ok()?;
    Some(buf)
}

fn encode_tag_ok(tag: u32) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut enc = minicbor::Encoder::new(&mut buf);
    enc.map(1).ok()?;
    enc.u32(0).ok()?.u32(tag).ok()?;
    Some(buf)
}

fn handle_list_buses(state: &SharedState) -> Option<Vec<u8>> {
    let st = state.lock().unwrap_or_else(|e| e.into_inner());
    let mut buf = Vec::new();
    let mut enc = minicbor::Encoder::new(&mut buf);
    enc.map(2).ok()?;
    enc.u32(0).ok()?.u32(3).ok()?;
    enc.u32(1).ok()?.array(st.i2c_buses.len() as u64).ok()?;
    for (i, bus) in st.i2c_buses.iter().enumerate() {
        let dev_count = bus.active_count();
        enc.map(2).ok()?;
        enc.u32(0).ok()?.u8(i as u8).ok()?;
        enc.u32(1).ok()?.array(dev_count as u64).ok()?;
        for j in 0..dev_count {
            if let Some((addr, name)) = bus.active_device_info(j) {
                enc.map(2).ok()?;
                enc.u32(0).ok()?.u8(addr).ok()?;
                enc.u32(1).ok()?.str(core::str::from_utf8(name).unwrap_or("?")).ok()?;
            }
        }
    }
    Some(buf)
}

fn handle_add_device(state: &SharedState, data: &[u8]) -> Option<Vec<u8>> {
    let mut dec = minicbor::Decoder::new(data);
    let _map = dec.map().ok()??;
    let _k0 = dec.u32().ok()?; let _tag = dec.u32().ok()?;
    let _k1 = dec.u32().ok()?; let bus_idx = dec.u8().ok()?;
    let _k2 = dec.u32().ok()?; let addr = dec.u8().ok()?;
    let _k3 = dec.u32().ok()?; let name = dec.str().ok()?;
    let _k4 = dec.u32().ok()?; let registers = dec.bytes().ok()?;
    let address = Address::new(addr)?;
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
    let bus = st.i2c_buses.get_mut(bus_idx as usize)?;
    match bus.add_device(address, name.as_bytes(), registers) {
        Ok(()) => encode_tag_ok(30),
        Err(()) => encode_error("add device failed"),
    }
}

fn handle_remove_device(state: &SharedState, data: &[u8]) -> Option<Vec<u8>> {
    let mut dec = minicbor::Decoder::new(data);
    let _map = dec.map().ok()??;
    let _k0 = dec.u32().ok()?; let _tag = dec.u32().ok()?;
    let _k1 = dec.u32().ok()?; let bus_idx = dec.u8().ok()?;
    let _k2 = dec.u32().ok()?; let addr = dec.u8().ok()?;
    let address = Address::new(addr)?;
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
    let bus = st.i2c_buses.get_mut(bus_idx as usize)?;
    match bus.remove_device(address) {
        Ok(()) => encode_tag_ok(31),
        Err(()) => encode_error("remove device failed"),
    }
}

fn handle_i2c_read(state: &SharedState, data: &[u8]) -> Option<Vec<u8>> {
    let mut dec = minicbor::Decoder::new(data);
    let _map = dec.map().ok()??;
    let _k0 = dec.u32().ok()?; let _tag = dec.u32().ok()?;
    let _k1 = dec.u32().ok()?; let bus_idx = dec.u8().ok()?;
    let _k2 = dec.u32().ok()?; let addr = dec.u8().ok()?;
    let _k3 = dec.u32().ok()?; let reg = dec.u8().ok()?;
    let _k4 = dec.u32().ok()?; let len = dec.u8().ok()?;
    let read_len = (len as usize).min(128);
    let mut read_buf = vec![0u8; read_len];
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
    let bus = st.i2c_buses.get_mut(bus_idx as usize)?;
    match bus.write_read(addr, &[reg], &mut read_buf) {
        Ok(()) => {
            let mut buf = Vec::new();
            let mut enc = minicbor::Encoder::new(&mut buf);
            enc.map(2).ok()?;
            enc.u32(0).ok()?.u32(1).ok()?;
            enc.u32(1).ok()?.bytes(&read_buf).ok()?;
            Some(buf)
        }
        Err(_) => encode_error("i2c read failed"),
    }
}

fn handle_i2c_write(state: &SharedState, data: &[u8]) -> Option<Vec<u8>> {
    let mut dec = minicbor::Decoder::new(data);
    let _map = dec.map().ok()??;
    let _k0 = dec.u32().ok()?; let _tag = dec.u32().ok()?;
    let _k1 = dec.u32().ok()?; let bus_idx = dec.u8().ok()?;
    let _k2 = dec.u32().ok()?; let addr = dec.u8().ok()?;
    let _k3 = dec.u32().ok()?; let write_data = dec.bytes().ok()?;
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
    let bus = st.i2c_buses.get_mut(bus_idx as usize)?;
    match bus.write(addr, write_data) {
        Ok(()) => encode_tag_ok(2),
        Err(_) => encode_error("i2c write failed"),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    use axum::body::Body;
    use dag_core::cbor::encode_dag;
    use dag_core::op::Dag;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn temp_site(filename: &str, content: &[u8]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join(filename);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, content).expect("failed to write temp file");
        dir
    }

    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = resp
            .into_body()
            .collect()
            .await
            .expect("failed to read body")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("failed to parse JSON")
    }

    fn make_cbor_dag(num_constants: usize) -> Vec<u8> {
        let mut dag = Dag::new();
        for i in 0..num_constants {
            dag.constant(i as f64).expect("failed to add constant");
        }
        encode_dag(&dag)
    }

    #[tokio::test]
    async fn test_post_dag_loads_cbor() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let cbor = make_cbor_dag(3);
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dag")
            .body(Body::from(cbor))
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body["ok"], true);
        assert_eq!(body["nodes"], 3);
    }

    #[tokio::test]
    async fn test_post_dag_invalid_cbor() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dag")
            .body(Body::from(vec![0xff, 0xfe, 0xfd]))
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        let body = json_body(resp).await;
        assert!(body["error"].as_str().unwrap().contains("invalid CBOR DAG"));
    }

    #[tokio::test]
    async fn test_get_status_empty() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        let body = json_body(resp).await;
        assert_eq!(body["loaded"], false);
        assert_eq!(body["nodes"], 0);
        assert_eq!(body["ticks"], 0);
    }

    #[tokio::test]
    async fn test_post_tick_no_dag() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/tick")
            .body(Body::empty())
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        let body = json_body(resp).await;
        assert!(body["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_post_tick_with_dag() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let cbor = make_cbor_dag(2);
        let load = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dag")
            .body(Body::from(cbor))
            .expect("request");
        router.clone().oneshot(load).await.expect("load");
        let tick = axum::http::Request::builder()
            .method("POST")
            .uri("/api/tick")
            .body(Body::empty())
            .expect("request");
        let resp = router.clone().oneshot(tick).await.expect("tick");
        let body = json_body(resp).await;
        assert_eq!(body["ok"], true);
        let status = axum::http::Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .expect("request");
        let sbox = json_body(router.oneshot(status).await.expect("status")).await;
        assert_eq!(sbox["ticks"], 1);
    }

    #[tokio::test]
    async fn test_pubsub_after_publish() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let mut dag = Dag::new();
        let c = dag.constant(42.0).expect("constant");
        dag.publish("sensor/temp", c).expect("publish");
        let cbor = encode_dag(&dag);
        let load = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dag")
            .body(Body::from(cbor))
            .expect("r");
        router.clone().oneshot(load).await.expect("load");
        let tick = axum::http::Request::builder()
            .method("POST")
            .uri("/api/tick")
            .body(Body::empty())
            .expect("r");
        router.clone().oneshot(tick).await.expect("tick");
        let ps = axum::http::Request::builder()
            .uri("/api/pubsub")
            .body(Body::empty())
            .expect("r");
        let body = json_body(router.oneshot(ps).await.expect("ps")).await;
        assert_eq!(body["sensor/temp"], 42.0);
    }

    #[tokio::test]
    async fn test_get_channels_empty() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .uri("/api/channels")
            .body(Body::empty())
            .expect("request");
        let body = json_body(router.oneshot(req).await.expect("failed")).await;
        assert_eq!(body["inputs"], serde_json::json!([]));
        assert_eq!(body["outputs"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_post_debug_toggle() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req1 = axum::http::Request::builder()
            .method("POST")
            .uri("/api/debug")
            .body(Body::empty())
            .expect("request");
        let body1 = json_body(router.clone().oneshot(req1).await.expect("r1")).await;
        assert_eq!(body1["debug"], true);
        let req2 = axum::http::Request::builder()
            .method("POST")
            .uri("/api/debug")
            .body(Body::empty())
            .expect("request");
        let body2 = json_body(router.oneshot(req2).await.expect("r2")).await;
        assert_eq!(body2["debug"], false);
    }

    #[tokio::test]
    async fn test_static_fallback() {
        let dir = temp_site("hello.txt", b"hello world");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .uri("/hello.txt")
            .body(Body::empty())
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        assert_eq!(resp.status(), 200);
        let bytes = resp
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert_eq!(&bytes[..], b"hello world");
    }

    // ── I2C WebSocket CBOR tests ────────────────────────────────────

    fn encode_cbor_request(tag: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).unwrap().u32(0).unwrap().u32(tag).unwrap();
        buf
    }

    fn decode_cbor_tag(data: &[u8]) -> u32 {
        let mut dec = minicbor::Decoder::new(data);
        let _ = dec.map().unwrap();
        let _ = dec.u32().unwrap();
        dec.u32().unwrap()
    }

    fn encode_add_device(bus: u8, addr: u8, name: &str, regs: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(5).unwrap();
        enc.u32(0).unwrap().u32(30).unwrap();
        enc.u32(1).unwrap().u8(bus).unwrap();
        enc.u32(2).unwrap().u8(addr).unwrap();
        enc.u32(3).unwrap().str(name).unwrap();
        enc.u32(4).unwrap().bytes(regs).unwrap();
        buf
    }

    fn encode_i2c_read(bus: u8, addr: u8, reg: u8, len: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(5).unwrap();
        enc.u32(0).unwrap().u32(1).unwrap();
        enc.u32(1).unwrap().u8(bus).unwrap();
        enc.u32(2).unwrap().u8(addr).unwrap();
        enc.u32(3).unwrap().u8(reg).unwrap();
        enc.u32(4).unwrap().u8(len).unwrap();
        buf
    }

    fn encode_i2c_write(bus: u8, addr: u8, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(4).unwrap();
        enc.u32(0).unwrap().u32(2).unwrap();
        enc.u32(1).unwrap().u8(bus).unwrap();
        enc.u32(2).unwrap().u8(addr).unwrap();
        enc.u32(3).unwrap().bytes(data).unwrap();
        buf
    }

    fn make_state() -> SharedState {
        Arc::new(Mutex::new(ServerState::new()))
    }

    #[test]
    fn test_list_buses_empty() {
        let state = make_state();
        let req = encode_cbor_request(3);
        let resp = dispatch_cbor(&state, &req).unwrap();
        assert_eq!(decode_cbor_tag(&resp), 3);
    }

    #[test]
    fn test_add_device_and_list() {
        let state = make_state();
        let add_req = encode_add_device(0, 0x48, "TMP1075", &[0xCA, 0xFE]);
        let add_resp = dispatch_cbor(&state, &add_req).unwrap();
        assert_eq!(decode_cbor_tag(&add_resp), 30);
        // Verify via state
        let st = state.lock().unwrap();
        assert_eq!(st.i2c_buses[0].active_count(), 1);
    }

    #[test]
    fn test_i2c_write_then_read() {
        let state = make_state();
        let add = encode_add_device(0, 0x50, "EEPROM", &[0u8; 256]);
        dispatch_cbor(&state, &add).unwrap();
        let write = encode_i2c_write(0, 0x50, &[0x10, 0xAB, 0xCD]);
        let w_resp = dispatch_cbor(&state, &write).unwrap();
        assert_eq!(decode_cbor_tag(&w_resp), 2);
        let read = encode_i2c_read(0, 0x50, 0x10, 2);
        let r_resp = dispatch_cbor(&state, &read).unwrap();
        assert_eq!(decode_cbor_tag(&r_resp), 1);
        // Decode payload
        let mut dec = minicbor::Decoder::new(&r_resp);
        let _ = dec.map().unwrap();
        let _ = dec.u32().unwrap(); let _ = dec.u32().unwrap();
        let _ = dec.u32().unwrap();
        let payload = dec.bytes().unwrap();
        assert_eq!(payload, &[0xAB, 0xCD]);
    }

    #[test]
    fn test_read_nonexistent_device() {
        let state = make_state();
        let read = encode_i2c_read(0, 0x48, 0, 2);
        let resp = dispatch_cbor(&state, &read).unwrap();
        assert_eq!(decode_cbor_tag(&resp), 255); // error
    }

    #[test]
    fn test_server_state_has_10_buses() {
        let state = ServerState::new();
        assert_eq!(state.i2c_buses.len(), BUS_COUNT);
    }
}
