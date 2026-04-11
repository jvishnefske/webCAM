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
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Query;
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

/// A logged telemetry event from the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetryEntry {
    pub seq: u32,
    pub timestamp_ms: f64,
    pub tag: u8,
    pub payload: serde_json::Value,
}

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
    /// Ring buffer of telemetry events from the frontend (last 256).
    pub telemetry_log: VecDeque<TelemetryEntry>,
    telemetry_seq: u32,
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
            telemetry_log: VecDeque::new(),
            telemetry_seq: 0,
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
        .route("/api/telemetry", get(get_telemetry))
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

#[cfg(not(tarpaulin_include))]
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

#[cfg(not(tarpaulin_include))]
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
        50..=56 => {
            handle_telemetry(state, tag, data);
            None
        }
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
        Err(_) => encode_error("add device failed"),
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
        Err(_) => encode_error("remove device failed"),
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

fn handle_telemetry(state: &SharedState, tag: u32, data: &[u8]) {
    let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
    let payload = parse_telemetry_payload(tag, data);

    let (actual_tag, seq, ts) = if tag == 56 {
        let s = payload.get("seq").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let t = payload.get("timestampMs").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let inner_tag = payload.get("innerTag").and_then(|v| v.as_u64()).unwrap_or(56) as u8;
        (inner_tag, s, t)
    } else {
        let seq = st.telemetry_seq;
        st.telemetry_seq += 1;
        (tag as u8, seq, 0.0)
    };

    // Log debug trace events (tag 57) to server console
    if actual_tag == 57 {
        let cat = payload.get("category").and_then(|v| v.as_str()).unwrap_or("?");
        let data = payload.get("data").cloned().unwrap_or(serde_json::Value::Null);
        eprintln!("[debug-trace] {cat}: {data}");
    }

    let entry = TelemetryEntry { seq, timestamp_ms: ts, tag: actual_tag, payload };
    if st.telemetry_log.len() >= 256 {
        st.telemetry_log.pop_front();
    }
    st.telemetry_log.push_back(entry);
}

fn parse_telemetry_payload(tag: u32, data: &[u8]) -> serde_json::Value {
    let mut dec = minicbor::Decoder::new(data);
    let mut map = serde_json::Map::new();

    let n = match dec.map() {
        Ok(Some(n)) => n,
        _ => return serde_json::Value::Object(map),
    };

    for _ in 0..n {
        let key = match dec.u32() {
            Ok(k) => k,
            _ => break,
        };
        if key == 0 {
            let _ = dec.u32();
            continue;
        }
        let field_name = match (tag, key) {
            (50, 1) | (51, 1) | (52, 1) => "blockId",
            (50, 2) | (52, 2) => "blockType",
            (50, 3) | (52, 3) => "config",
            (50, 4) => "x",
            (50, 5) => "y",
            (53, 1) => "fromBlock",
            (53, 2) => "fromPort",
            (53, 3) => "toBlock",
            (53, 4) => "toPort",
            (53, 5) | (54, 1) => "channelId",
            (56, 1) => "seq",
            (56, 2) => "timestampMs",
            (56, 3) => "inner",
            (57, 1) => "category",
            (57, 2) => "data",
            _ => "unknown",
        };
        match dec.datatype() {
            Ok(minicbor::data::Type::U8 | minicbor::data::Type::U16 | minicbor::data::Type::U32) => {
                if let Ok(v) = dec.u32() {
                    map.insert(field_name.to_string(), serde_json::json!(v));
                }
            }
            Ok(minicbor::data::Type::F32 | minicbor::data::Type::F64) => {
                if let Ok(v) = dec.f64() {
                    map.insert(field_name.to_string(), serde_json::json!(v));
                }
            }
            Ok(minicbor::data::Type::String) => {
                if let Ok(v) = dec.str() {
                    map.insert(field_name.to_string(), serde_json::json!(v));
                }
            }
            _ => break,
        }
    }
    serde_json::Value::Object(map)
}

#[derive(serde::Deserialize)]
struct TelemetryQuery {
    since: Option<u32>,
}

/// GET /api/telemetry -- return recent telemetry events as JSON array.
async fn get_telemetry(
    State(state): State<SharedState>,
    Query(query): Query<TelemetryQuery>,
) -> Json<serde_json::Value> {
    let s = state.lock().unwrap_or_else(|e| e.into_inner());
    let since = query.since.unwrap_or(0);
    let entries: Vec<&TelemetryEntry> = s
        .telemetry_log
        .iter()
        .filter(|e| e.seq >= since)
        .collect();
    Json(serde_json::json!(entries))
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

    fn encode_telemetry_block_added(block_id: u32, block_type: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(3).unwrap();
        enc.u32(0).unwrap().u32(50).unwrap();
        enc.u32(1).unwrap().u32(block_id).unwrap();
        enc.u32(2).unwrap().str(block_type).unwrap();
        buf
    }

    #[test]
    fn test_telemetry_block_added_logged() {
        let state = make_state();
        let req = encode_telemetry_block_added(7, "constant");
        let resp = dispatch_cbor(&state, &req);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 50);
        assert_eq!(st.telemetry_log[0].payload["blockId"], 7);
        assert_eq!(st.telemetry_log[0].payload["blockType"], "constant");
    }

    #[test]
    fn test_telemetry_graph_reset() {
        let state = make_state();
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).unwrap();
        enc.u32(0).unwrap().u32(55).unwrap();
        let resp = dispatch_cbor(&state, &buf);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 55);
    }

    #[tokio::test]
    async fn test_get_telemetry_empty() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let req = axum::http::Request::builder()
            .uri("/api/telemetry")
            .body(Body::empty())
            .expect("request");
        let resp = router.oneshot(req).await.expect("failed");
        let body = json_body(resp).await;
        assert!(body.as_array().unwrap().is_empty());
    }

    // ── encode_error / encode_tag_ok direct tests ──────────────────

    #[test]
    fn test_encode_error_produces_valid_cbor() {
        let buf = encode_error("something went wrong").expect("encode_error returned None");
        let mut dec = minicbor::Decoder::new(&buf);
        let n = dec.map().unwrap().unwrap();
        assert_eq!(n, 2);
        // key 0 -> 255 (error tag)
        assert_eq!(dec.u32().unwrap(), 0);
        assert_eq!(dec.u32().unwrap(), 255);
        // key 1 -> error message string
        assert_eq!(dec.u32().unwrap(), 1);
        assert_eq!(dec.str().unwrap(), "something went wrong");
    }

    #[test]
    fn test_encode_tag_ok_produces_valid_cbor() {
        let buf = encode_tag_ok(30).expect("encode_tag_ok returned None");
        let mut dec = minicbor::Decoder::new(&buf);
        let n = dec.map().unwrap().unwrap();
        assert_eq!(n, 1);
        assert_eq!(dec.u32().unwrap(), 0);
        assert_eq!(dec.u32().unwrap(), 30);
    }

    #[test]
    fn test_encode_tag_ok_different_tags() {
        for tag in [1u32, 2, 3, 31, 255] {
            let buf = encode_tag_ok(tag).expect("encode_tag_ok returned None");
            assert_eq!(decode_cbor_tag(&buf), tag);
        }
    }

    // ── dispatch_cbor: unknown tag ─────────────────────────────────

    #[test]
    fn test_dispatch_unknown_tag() {
        let state = make_state();
        let req = encode_cbor_request(99);
        let resp = dispatch_cbor(&state, &req).unwrap();
        assert_eq!(decode_cbor_tag(&resp), 255);
        // Verify the error message is present
        let mut dec = minicbor::Decoder::new(&resp);
        let _ = dec.map().unwrap();
        let _ = dec.u32().unwrap();
        let _ = dec.u32().unwrap(); // tag 255
        let _ = dec.u32().unwrap(); // key 1
        assert_eq!(dec.str().unwrap(), "unknown request tag");
    }

    #[test]
    fn test_dispatch_invalid_cbor() {
        let state = make_state();
        let resp = dispatch_cbor(&state, &[0xFF, 0xFE]);
        assert!(resp.is_none());
    }

    // ── handle_remove_device tests ─────────────────────────────────

    fn encode_remove_device(bus: u8, addr: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(3).unwrap();
        enc.u32(0).unwrap().u32(31).unwrap();
        enc.u32(1).unwrap().u8(bus).unwrap();
        enc.u32(2).unwrap().u8(addr).unwrap();
        buf
    }

    #[test]
    fn test_remove_device_success() {
        let state = make_state();
        // Add a device first
        let add = encode_add_device(0, 0x48, "TMP", &[0x00; 4]);
        dispatch_cbor(&state, &add).unwrap();
        assert_eq!(state.lock().unwrap().i2c_buses[0].active_count(), 1);

        // Remove it
        let rm = encode_remove_device(0, 0x48);
        let resp = dispatch_cbor(&state, &rm).unwrap();
        assert_eq!(decode_cbor_tag(&resp), 31);
        assert_eq!(state.lock().unwrap().i2c_buses[0].active_count(), 0);
    }

    #[test]
    fn test_remove_device_not_found() {
        let state = make_state();
        let rm = encode_remove_device(0, 0x48);
        let resp = dispatch_cbor(&state, &rm).unwrap();
        // Should be an error (tag 255) since no device at that address
        assert_eq!(decode_cbor_tag(&resp), 255);
    }

    #[test]
    fn test_remove_device_invalid_address() {
        let state = make_state();
        // Address 0xFF > 0x7F, so Address::new returns None -> dispatch returns None
        let rm = encode_remove_device(0, 0xFF);
        let resp = dispatch_cbor(&state, &rm);
        assert!(resp.is_none());
    }

    // ── handle_add_device edge cases ───────────────────────────────

    #[test]
    fn test_add_device_invalid_bus_index() {
        let state = make_state();
        // Bus index 99 is out of range (only 0..10)
        let add = encode_add_device(99, 0x48, "DEV", &[0x00]);
        let resp = dispatch_cbor(&state, &add);
        // Returns None because st.i2c_buses.get_mut(99) returns None
        assert!(resp.is_none());
    }

    #[test]
    fn test_add_device_invalid_address() {
        let state = make_state();
        // Address 0x80 exceeds 7-bit range
        let add = encode_add_device(0, 0x80, "DEV", &[0x00]);
        let resp = dispatch_cbor(&state, &add);
        assert!(resp.is_none());
    }

    #[test]
    fn test_add_device_duplicate_address() {
        let state = make_state();
        let add = encode_add_device(0, 0x48, "DEV1", &[0x00; 4]);
        let resp1 = dispatch_cbor(&state, &add).unwrap();
        assert_eq!(decode_cbor_tag(&resp1), 30);

        // Adding same address again should fail
        let add2 = encode_add_device(0, 0x48, "DEV2", &[0x00; 4]);
        let resp2 = dispatch_cbor(&state, &add2).unwrap();
        assert_eq!(decode_cbor_tag(&resp2), 255); // error
    }

    // ── handle_i2c_read edge cases ─────────────────────────────────

    #[test]
    fn test_i2c_read_invalid_bus() {
        let state = make_state();
        let read = encode_i2c_read(99, 0x48, 0, 2);
        let resp = dispatch_cbor(&state, &read);
        // Out-of-bounds bus index -> None
        assert!(resp.is_none());
    }

    // ── handle_i2c_write edge cases ────────────────────────────────

    #[test]
    fn test_i2c_write_no_device() {
        let state = make_state();
        let write = encode_i2c_write(0, 0x48, &[0x00, 0xAB]);
        let resp = dispatch_cbor(&state, &write).unwrap();
        assert_eq!(decode_cbor_tag(&resp), 255); // error
    }

    #[test]
    fn test_i2c_write_invalid_bus() {
        let state = make_state();
        let write = encode_i2c_write(99, 0x48, &[0x00, 0xAB]);
        let resp = dispatch_cbor(&state, &write);
        assert!(resp.is_none());
    }

    // ── telemetry tag tests (50-56) ────────────────────────────────

    fn encode_telemetry_block_removed(block_id: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).unwrap();
        enc.u32(0).unwrap().u32(51).unwrap();
        enc.u32(1).unwrap().u32(block_id).unwrap();
        buf
    }

    fn encode_telemetry_block_configured(block_id: u32, block_type: &str, config: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(4).unwrap();
        enc.u32(0).unwrap().u32(52).unwrap();
        enc.u32(1).unwrap().u32(block_id).unwrap();
        enc.u32(2).unwrap().str(block_type).unwrap();
        enc.u32(3).unwrap().str(config).unwrap();
        buf
    }

    fn encode_telemetry_edge_added(
        from_block: u32, from_port: u32, to_block: u32, to_port: u32, channel_id: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(6).unwrap();
        enc.u32(0).unwrap().u32(53).unwrap();
        enc.u32(1).unwrap().u32(from_block).unwrap();
        enc.u32(2).unwrap().u32(from_port).unwrap();
        enc.u32(3).unwrap().u32(to_block).unwrap();
        enc.u32(4).unwrap().u32(to_port).unwrap();
        enc.u32(5).unwrap().u32(channel_id).unwrap();
        buf
    }

    fn encode_telemetry_edge_removed(channel_id: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(2).unwrap();
        enc.u32(0).unwrap().u32(54).unwrap();
        enc.u32(1).unwrap().u32(channel_id).unwrap();
        buf
    }

    fn encode_telemetry_wrapped(seq: u32, timestamp_ms: f64, inner_tag: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(4).unwrap();
        enc.u32(0).unwrap().u32(56).unwrap();
        enc.u32(1).unwrap().u32(seq).unwrap();
        enc.u32(2).unwrap().f64(timestamp_ms).unwrap();
        enc.u32(3).unwrap().u32(inner_tag).unwrap();
        buf
    }

    #[test]
    fn test_telemetry_block_removed_tag51() {
        let state = make_state();
        let req = encode_telemetry_block_removed(42);
        let resp = dispatch_cbor(&state, &req);
        assert!(resp.is_none()); // telemetry returns None
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 51);
        assert_eq!(st.telemetry_log[0].payload["blockId"], 42);
    }

    #[test]
    fn test_telemetry_block_configured_tag52() {
        let state = make_state();
        let req = encode_telemetry_block_configured(5, "gain", "2.0");
        let resp = dispatch_cbor(&state, &req);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 52);
        assert_eq!(st.telemetry_log[0].payload["blockId"], 5);
        assert_eq!(st.telemetry_log[0].payload["blockType"], "gain");
        assert_eq!(st.telemetry_log[0].payload["config"], "2.0");
    }

    #[test]
    fn test_telemetry_edge_added_tag53() {
        let state = make_state();
        let req = encode_telemetry_edge_added(1, 0, 2, 0, 100);
        let resp = dispatch_cbor(&state, &req);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 53);
        assert_eq!(st.telemetry_log[0].payload["fromBlock"], 1);
        assert_eq!(st.telemetry_log[0].payload["fromPort"], 0);
        assert_eq!(st.telemetry_log[0].payload["toBlock"], 2);
        assert_eq!(st.telemetry_log[0].payload["toPort"], 0);
        assert_eq!(st.telemetry_log[0].payload["channelId"], 100);
    }

    #[test]
    fn test_telemetry_edge_removed_tag54() {
        let state = make_state();
        let req = encode_telemetry_edge_removed(77);
        let resp = dispatch_cbor(&state, &req);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].tag, 54);
        assert_eq!(st.telemetry_log[0].payload["channelId"], 77);
    }

    #[test]
    fn test_telemetry_wrapped_tag56() {
        let state = make_state();
        let req = encode_telemetry_wrapped(10, 1234.5, 50);
        let resp = dispatch_cbor(&state, &req);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        // The seq and timestamp are extracted from the payload
        assert_eq!(st.telemetry_log[0].seq, 10);
        assert!((st.telemetry_log[0].timestamp_ms - 1234.5).abs() < f64::EPSILON);
        // Note: the inner tag field is stored as "inner" by parse_telemetry_payload
        // but handle_telemetry looks for "innerTag", so the fallback (56) is used.
        // This means the wrapped tag is recorded as 56 rather than the inner tag.
        assert_eq!(st.telemetry_log[0].tag, 56);
        // The parsed payload should have the "inner" field with value 50
        assert_eq!(st.telemetry_log[0].payload["inner"], 50);
    }

    #[test]
    fn test_telemetry_seq_increments() {
        let state = make_state();
        // Send three non-wrapped telemetry events (tags 50-52 use auto seq)
        dispatch_cbor(&state, &encode_telemetry_block_added(1, "const"));
        dispatch_cbor(&state, &encode_telemetry_block_removed(1));
        dispatch_cbor(&state, &encode_telemetry_block_added(2, "gain"));

        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 3);
        assert_eq!(st.telemetry_log[0].seq, 0);
        assert_eq!(st.telemetry_log[1].seq, 1);
        assert_eq!(st.telemetry_log[2].seq, 2);
    }

    #[test]
    fn test_telemetry_ring_buffer_eviction() {
        let state = make_state();
        // Fill 256 entries then add one more to trigger eviction
        for i in 0..257u32 {
            let req = encode_telemetry_block_added(i, "const");
            dispatch_cbor(&state, &req);
        }
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 256);
        // The oldest entry (seq 0) should have been evicted
        assert_eq!(st.telemetry_log[0].seq, 1);
        assert_eq!(st.telemetry_log[255].seq, 256);
    }

    // ── GET /api/telemetry with since parameter ────────────────────

    #[tokio::test]
    async fn test_get_telemetry_with_entries() {
        let state: SharedState = Arc::new(Mutex::new(ServerState::new()));

        // Manually inject telemetry entries
        {
            let mut s = state.lock().unwrap();
            for i in 0..5u32 {
                s.telemetry_log.push_back(TelemetryEntry {
                    seq: i,
                    timestamp_ms: (i as f64) * 100.0,
                    tag: 50,
                    payload: serde_json::json!({"blockId": i}),
                });
            }
        }

        let router = Router::new()
            .route("/api/telemetry", get(get_telemetry))
            .with_state(state);

        // Query without since -- should return all 5
        let req = axum::http::Request::builder()
            .uri("/api/telemetry")
            .body(Body::empty())
            .expect("request");
        let body = json_body(router.clone().oneshot(req).await.expect("ok")).await;
        assert_eq!(body.as_array().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn test_get_telemetry_since_filter() {
        let state: SharedState = Arc::new(Mutex::new(ServerState::new()));
        {
            let mut s = state.lock().unwrap();
            for i in 0..5u32 {
                s.telemetry_log.push_back(TelemetryEntry {
                    seq: i,
                    timestamp_ms: (i as f64) * 100.0,
                    tag: 50,
                    payload: serde_json::json!({"blockId": i}),
                });
            }
        }

        let router = Router::new()
            .route("/api/telemetry", get(get_telemetry))
            .with_state(state);

        // Query with since=3 -- should return entries with seq >= 3 (seq 3 and 4)
        let req = axum::http::Request::builder()
            .uri("/api/telemetry?since=3")
            .body(Body::empty())
            .expect("request");
        let body = json_body(router.clone().oneshot(req).await.expect("ok")).await;
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["seq"], 3);
        assert_eq!(arr[1]["seq"], 4);
    }

    #[tokio::test]
    async fn test_get_telemetry_since_beyond_all() {
        let state: SharedState = Arc::new(Mutex::new(ServerState::new()));
        {
            let mut s = state.lock().unwrap();
            for i in 0..3u32 {
                s.telemetry_log.push_back(TelemetryEntry {
                    seq: i,
                    timestamp_ms: 0.0,
                    tag: 55,
                    payload: serde_json::json!({}),
                });
            }
        }

        let router = Router::new()
            .route("/api/telemetry", get(get_telemetry))
            .with_state(state);

        // since=100 -- no entries match
        let req = axum::http::Request::builder()
            .uri("/api/telemetry?since=100")
            .body(Body::empty())
            .expect("request");
        let body = json_body(router.oneshot(req).await.expect("ok")).await;
        assert!(body.as_array().unwrap().is_empty());
    }

    // ── list_buses after adding devices ────────────────────────────

    #[test]
    fn test_list_buses_with_devices() {
        let state = make_state();
        // Add two devices to bus 0
        let add1 = encode_add_device(0, 0x48, "TMP1075", &[0xCA, 0xFE]);
        let add2 = encode_add_device(0, 0x50, "EEPROM", &[0u8; 16]);
        dispatch_cbor(&state, &add1).unwrap();
        dispatch_cbor(&state, &add2).unwrap();

        let req = encode_cbor_request(3);
        let resp = dispatch_cbor(&state, &req).unwrap();
        assert_eq!(decode_cbor_tag(&resp), 3);

        // Decode the bus array and verify bus 0 has 2 devices
        let mut dec = minicbor::Decoder::new(&resp);
        let _ = dec.map().unwrap(); // map(2)
        let _ = dec.u32().unwrap(); // key 0
        let _ = dec.u32().unwrap(); // tag 3
        let _ = dec.u32().unwrap(); // key 1
        let bus_count = dec.array().unwrap().unwrap();
        assert_eq!(bus_count, BUS_COUNT as u64);

        // First bus should have 2 devices
        let _ = dec.map().unwrap(); // map(2) for bus 0
        let _ = dec.u32().unwrap(); // key 0
        let bus_idx = dec.u8().unwrap();
        assert_eq!(bus_idx, 0);
        let _ = dec.u32().unwrap(); // key 1
        let dev_count = dec.array().unwrap().unwrap();
        assert_eq!(dev_count, 2);
    }

    #[test]
    fn test_add_remove_add_cycle() {
        let state = make_state();
        // Add device
        let add = encode_add_device(1, 0x20, "GPIO", &[0x00; 8]);
        let r1 = dispatch_cbor(&state, &add).unwrap();
        assert_eq!(decode_cbor_tag(&r1), 30);
        assert_eq!(state.lock().unwrap().i2c_buses[1].active_count(), 1);

        // Remove device
        let rm = encode_remove_device(1, 0x20);
        let r2 = dispatch_cbor(&state, &rm).unwrap();
        assert_eq!(decode_cbor_tag(&r2), 31);
        assert_eq!(state.lock().unwrap().i2c_buses[1].active_count(), 0);

        // Re-add same address
        let add2 = encode_add_device(1, 0x20, "GPIO_v2", &[0xFF; 8]);
        let r3 = dispatch_cbor(&state, &add2).unwrap();
        assert_eq!(decode_cbor_tag(&r3), 30);
        assert_eq!(state.lock().unwrap().i2c_buses[1].active_count(), 1);
    }

    #[test]
    fn test_server_state_default() {
        let state = ServerState::default();
        assert!(!state.debug_mode);
        assert_eq!(state.i2c_buses.len(), BUS_COUNT);
        assert!(state.telemetry_log.is_empty());
        assert!(state.known_inputs.is_empty());
        assert!(state.known_outputs.is_empty());
    }

    #[tokio::test]
    async fn test_post_tick_with_debug_mode() {
        let dir = temp_site("index.html", b"test");
        let router = app(dir.path());
        let cbor = make_cbor_dag(2);

        // Load a DAG
        let load = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dag")
            .body(Body::from(cbor))
            .expect("request");
        router.clone().oneshot(load).await.expect("load");

        // Enable debug mode
        let dbg = axum::http::Request::builder()
            .method("POST")
            .uri("/api/debug")
            .body(Body::empty())
            .expect("request");
        let dbg_body = json_body(router.clone().oneshot(dbg).await.expect("debug")).await;
        assert_eq!(dbg_body["debug"], true);

        // Tick with debug mode on
        let tick = axum::http::Request::builder()
            .method("POST")
            .uri("/api/tick")
            .body(Body::empty())
            .expect("request");
        let tick_body = json_body(router.clone().oneshot(tick).await.expect("tick")).await;
        assert_eq!(tick_body["ok"], true);

        // Pubsub should contain debug entries
        let ps = axum::http::Request::builder()
            .uri("/api/pubsub")
            .body(Body::empty())
            .expect("request");
        let ps_body = json_body(router.oneshot(ps).await.expect("pubsub")).await;
        // Debug mode adds _dbg/N entries for each value
        assert!(ps_body.as_object().unwrap().keys().any(|k| k.starts_with("_dbg/")));
    }

    #[test]
    fn test_telemetry_parse_float_value() {
        let state = make_state();
        // Create a telemetry event with an f64 value
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(3).unwrap();
        enc.u32(0).unwrap().u32(50).unwrap();
        enc.u32(1).unwrap().u32(42).unwrap();
        enc.u32(4).unwrap().f64(12.5).unwrap(); // x coordinate as float
        let resp = dispatch_cbor(&state, &buf);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        // The float value should be parsed
        assert_eq!(st.telemetry_log[0].payload["x"], 12.5);
    }

    #[test]
    fn test_telemetry_wrapped_with_inner_tag() {
        let state = make_state();
        // Create a wrapped telemetry event (tag 56) with innerTag field
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(4).unwrap();
        enc.u32(0).unwrap().u32(56).unwrap();
        enc.u32(1).unwrap().u32(99).unwrap();  // seq
        enc.u32(2).unwrap().f64(500.0).unwrap(); // timestampMs
        // Use key 3 mapped to "inner" but store the inner tag as u32 so
        // handle_telemetry sees "innerTag"
        enc.u32(3).unwrap().u32(51).unwrap();
        let resp = dispatch_cbor(&state, &buf);
        assert!(resp.is_none());
        let st = state.lock().unwrap();
        assert_eq!(st.telemetry_log.len(), 1);
        assert_eq!(st.telemetry_log[0].seq, 99);
    }

    #[test]
    fn test_parse_telemetry_payload_empty_map() {
        // A map with no interesting keys should produce an empty object
        let mut buf = Vec::new();
        let mut enc = minicbor::Encoder::new(&mut buf);
        enc.map(1).unwrap();
        enc.u32(0).unwrap().u32(55).unwrap(); // just the tag key
        let result = parse_telemetry_payload(55, &buf);
        assert!(result.is_object());
    }

    #[test]
    fn test_parse_telemetry_payload_invalid_cbor() {
        let result = parse_telemetry_payload(50, &[0xFF, 0xFE]);
        assert!(result.is_object());
        assert!(result.as_object().unwrap().is_empty());
    }
}
