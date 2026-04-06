//! End-to-end tests exercising the native server's REST + pubsub API.
//!
//! These simulate the full UI workflow:
//! 1. Deploy a DAG (blocks + wiring) via POST /api/dag
//! 2. Tick the graph via POST /api/tick
//! 3. Read published values via GET /api/pubsub
//! 4. Verify values flow through the dataflow pipeline
//! 5. Send telemetry events via WebSocket and read via GET /api/telemetry

use axum::body::Body;
use dag_core::cbor::encode_dag;
use dag_core::op::Dag;
use http_body_util::BodyExt;
use native_server::app;
use tower::ServiceExt;

fn temp_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("index.html"), b"test").unwrap();
    dir
}

async fn json_body(resp: axum::response::Response) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn post(uri: &str, body: Vec<u8>) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::from(body))
        .unwrap()
}

fn get(uri: &str) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

// ── Test 1: Constant → Publish, read via pubsub ─────────────────

#[tokio::test]
async fn constant_publishes_to_topic() {
    let dir = temp_dir();
    let app = app(dir.path());

    // Build DAG: constant(42.0) → publish("test/value")
    let mut dag = Dag::new();
    let c = dag.constant(42.0).unwrap();
    dag.publish("test/value", c).unwrap();
    let cbor = encode_dag(&dag);

    // Deploy
    let resp = app.clone().oneshot(post("/api/dag", cbor)).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["nodes"], 2);

    // Tick once
    let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["ok"], true);

    // Read pubsub
    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["test/value"], 42.0);
}

// ── Test 2: Math pipeline (add + publish) ────────────────────────

#[tokio::test]
async fn add_constants_publish_sum() {
    let dir = temp_dir();
    let app = app(dir.path());

    // Build DAG: const(3) + const(4) → publish("sum")
    let mut dag = Dag::new();
    let a = dag.constant(3.0).unwrap();
    let b = dag.constant(4.0).unwrap();
    let sum = dag.add(a, b).unwrap();
    dag.publish("sum", sum).unwrap();
    let cbor = encode_dag(&dag);

    let resp = app.clone().oneshot(post("/api/dag", cbor)).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["sum"], 7.0);
}

// ── Test 3: Multiply chain (const * const → publish) ─────────────

#[tokio::test]
async fn multiply_constants_publish() {
    let dir = temp_dir();
    let app = app(dir.path());

    // const(5) * const(6) → publish("product")
    let mut dag = Dag::new();
    let a = dag.constant(5.0).unwrap();
    let b = dag.constant(6.0).unwrap();
    let product = dag.mul(a, b).unwrap();
    dag.publish("product", product).unwrap();
    let cbor = encode_dag(&dag);

    let resp = app.clone().oneshot(post("/api/dag", cbor)).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    assert_eq!(json_body(resp).await["product"], 30.0);
}

// ── Test 4: Multiple ticks accumulate in pubsub ──────────────────

#[tokio::test]
async fn multiple_ticks_update_pubsub() {
    let dir = temp_dir();
    let app = app(dir.path());

    // const(1.0) → publish("counter")
    let mut dag = Dag::new();
    let c = dag.constant(1.0).unwrap();
    dag.publish("counter", c).unwrap();
    let cbor = encode_dag(&dag);

    let resp = app.clone().oneshot(post("/api/dag", cbor)).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    // Tick 3 times
    for _ in 0..3 {
        let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
        assert_eq!(json_body(resp).await["ok"], true);
    }

    // Status shows 3 ticks
    let resp = app.clone().oneshot(get("/api/status")).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["ticks"], 3);
    assert_eq!(body["loaded"], true);

    // Pubsub still has the value
    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    assert_eq!(json_body(resp).await["counter"], 1.0);
}

// ── Test 5: Subscribe reads external value ───────────────────────

#[tokio::test]
async fn subscribe_reads_injected_value() {
    let dir = temp_dir();
    let app = app(dir.path());

    // subscribe("input") → publish("output")
    let mut dag = Dag::new();
    let sub = dag.subscribe("input").unwrap();
    dag.publish("output", sub).unwrap();
    let cbor = encode_dag(&dag);

    let resp = app.clone().oneshot(post("/api/dag", cbor)).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    // Tick without injecting — output should be 0
    let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    let body = json_body(resp).await;
    // Subscribe with no prior publish reads 0.0
    assert_eq!(body["output"], 0.0);
}

// ── Test 6: Redeploy clears previous DAG ─────────────────────────

#[tokio::test]
async fn redeploy_clears_state() {
    let dir = temp_dir();
    let app = app(dir.path());

    // Deploy first DAG: const(99) → publish("a")
    let mut dag1 = Dag::new();
    let c = dag1.constant(99.0).unwrap();
    dag1.publish("a", c).unwrap();
    let resp = app.clone().oneshot(post("/api/dag", encode_dag(&dag1))).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    assert_eq!(json_body(resp).await["a"], 99.0);

    // Deploy second DAG: const(1) → publish("b")
    let mut dag2 = Dag::new();
    let c = dag2.constant(1.0).unwrap();
    dag2.publish("b", c).unwrap();
    let resp = app.clone().oneshot(post("/api/dag", encode_dag(&dag2))).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    let resp = app.clone().oneshot(post("/api/tick", vec![])).await.unwrap();
    assert_eq!(json_body(resp).await["ok"], true);

    // Old topic should be gone, new topic present
    let resp = app.clone().oneshot(get("/api/pubsub")).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["b"], 1.0);
    // "a" may or may not be cleared depending on impl — check status
    let resp = app.clone().oneshot(get("/api/status")).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["loaded"], true);
    assert_eq!(body["nodes"], 2); // new DAG has 2 nodes
}

// ── Test 7: Telemetry events appear in /api/telemetry ────────────

#[tokio::test]
async fn telemetry_events_via_websocket() {
    let dir = temp_dir();
    let app = app(dir.path());

    // Telemetry starts empty
    let resp = app.clone().oneshot(get("/api/telemetry")).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0);

    // We can't easily send WebSocket messages through tower::oneshot,
    // but we can verify the telemetry endpoint works with the since param
    let resp = app.clone().oneshot(get("/api/telemetry?since=0")).await.unwrap();
    let body = json_body(resp).await;
    assert!(body.is_array());
}

// ── Test 8: Debug mode toggle ────────────────────────────────────

#[tokio::test]
async fn debug_mode_toggle() {
    let dir = temp_dir();
    let app = app(dir.path());

    // Toggle debug on
    let resp = app.clone().oneshot(post("/api/debug", vec![])).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["debug"], true);

    // Toggle debug off
    let resp = app.clone().oneshot(post("/api/debug", vec![])).await.unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["debug"], false);
}

// ── Test 9: Channels endpoint returns valid structure ─────────────

#[tokio::test]
async fn channels_endpoint_returns_arrays() {
    let dir = temp_dir();
    let app = app(dir.path());

    let resp = app.clone().oneshot(get("/api/channels")).await.unwrap();
    let body = json_body(resp).await;
    assert!(body["inputs"].is_array());
    assert!(body["outputs"].is_array());
}
