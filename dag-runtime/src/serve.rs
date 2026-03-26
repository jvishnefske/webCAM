use alloc::vec::Vec;

use crate::channels::MapChannels;
use crate::executor::DagExecutor;
use crate::http;
use crate::pubsub::SimplePubSub;
use dag_core::eval::{NullChannels, NullPubSub};

#[cfg(feature = "embedded-assets")]
use crate::generated;

/// Handle an incoming HTTP request, returning the response bytes.
///
/// Routes:
/// - `GET /`              serves index.html (gzipped)
/// - `GET /index.html`    serves index.html (gzipped)
/// - `GET /dag-editor.js` serves editor JS (gzipped)
/// - `GET /*.wasm`        serves WASM module (gzipped)
/// - `GET /*.js`          serves JS glue (gzipped)
/// - `POST /api/dag`      loads CBOR DAG into executor
/// - `GET /api/status`    returns executor status as JSON
/// - `POST /api/tick`     executes one tick of the DAG
/// - everything else      returns 404
pub fn handle_request(
    request_data: &[u8],
    executor: &mut DagExecutor,
    channels: &mut MapChannels,
    pubsub: &mut SimplePubSub,
) -> Vec<u8> {
    let (method, path) = match http::parse_request_line(request_data) {
        Some((m, p)) => (m, p),
        None => return http::http_response_error(400, "Bad request"),
    };

    match (method, path) {
        ("POST", "/api/dag") => match http::extract_body(request_data) {
            Some(body) => match executor.load_cbor(body) {
                Ok(()) => {
                    let msg = alloc::format!(
                        r#"{{"ok":true,"nodes":{}}}"#,
                        executor.node_count()
                    );
                    http::http_response_ok(msg.as_bytes())
                }
                Err(e) => {
                    let msg = alloc::format!("DAG decode error: {}", e);
                    http::http_response_error(400, &msg)
                }
            },
            None => http::http_response_error(400, "No body"),
        },

        ("GET", "/api/status") => {
            let json = alloc::format!(
                r#"{{"loaded":{},"nodes":{},"ticks":{}}}"#,
                executor.is_loaded(),
                executor.node_count(),
                executor.tick_count(),
            );
            http::http_response_ok(json.as_bytes())
        }

        ("POST", "/api/tick") => {
            // Evaluate with null readers (channel/pubsub state lives in the
            // executor's value buffer between ticks). Outputs are written to
            // the real channels/pubsub after evaluation.
            let null_ch = NullChannels;
            let null_ps = NullPubSub;
            match executor.tick(&null_ch, channels, &null_ps, pubsub) {
                Some(eval_result) => {
                    let json = alloc::format!(
                        r#"{{"ok":true,"outputs":{},"publishes":{}}}"#,
                        eval_result.outputs.len(),
                        eval_result.publishes.len(),
                    );
                    http::http_response_ok(json.as_bytes())
                }
                None => http::http_response_error(400, "No DAG loaded"),
            }
        }

        #[cfg(feature = "embedded-assets")]
        ("GET", path) => match generated::lookup(path) {
            Some((data, content_type)) => http::http_response_gzipped(content_type, data),
            None => http::http_response_error(404, "Not found"),
        },

        #[cfg(not(feature = "embedded-assets"))]
        ("GET", _) => http::http_response_error(404, "No embedded assets"),

        _ => http::http_response_error(404, "Not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use dag_core::cbor::encode_dag;
    use dag_core::op::Dag;

    fn make_env() -> (DagExecutor, MapChannels, SimplePubSub) {
        (DagExecutor::new(), MapChannels::new(), SimplePubSub::new())
    }

    fn response_status(resp: &[u8]) -> u16 {
        let s = String::from_utf8_lossy(resp);
        let first_line = s.lines().next().unwrap();
        let status_str = first_line.split_whitespace().nth(1).unwrap();
        status_str.parse().unwrap()
    }

    fn response_body_str(resp: &[u8]) -> String {
        let s = core::str::from_utf8(resp).unwrap();
        let idx = s.find("\r\n\r\n").unwrap();
        s[idx + 4..].to_string()
    }

    #[test]
    fn test_handle_get_root() {
        let (mut exec, mut ch, mut ps) = make_env();
        let req = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let resp = handle_request(req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        // With placeholder (empty) assets and no embedded-assets feature,
        // we expect either a 404 or a valid response depending on feature flags.
        // Without embedded-assets feature, GET returns 404.
        assert_eq!(status, 404);
    }

    #[test]
    fn test_handle_post_dag() {
        let (mut exec, mut ch, mut ps) = make_env();
        let mut dag = Dag::new();
        dag.constant(1.0).unwrap();
        dag.constant(2.0).unwrap();
        dag.add(0, 1).unwrap();
        let cbor = encode_dag(&dag);

        let header = b"POST /api/dag HTTP/1.1\r\nContent-Type: application/cbor\r\n\r\n";
        let mut req = header.to_vec();
        req.extend_from_slice(&cbor);

        let resp = handle_request(&req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 200);

        let body = response_body_str(&resp);
        assert!(body.contains(r#""ok":true"#));
        assert!(body.contains(r#""nodes":3"#));
        assert!(exec.is_loaded());
        assert_eq!(exec.node_count(), 3);
    }

    #[test]
    fn test_handle_post_dag_invalid() {
        let (mut exec, mut ch, mut ps) = make_env();
        let req = b"POST /api/dag HTTP/1.1\r\nContent-Type: application/cbor\r\n\r\n\xff\xfe\xfd";
        let resp = handle_request(req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 400);
        let body = response_body_str(&resp);
        assert!(body.contains("DAG decode error"));
    }

    #[test]
    fn test_handle_get_status() {
        let (mut exec, mut ch, mut ps) = make_env();
        let req = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let resp = handle_request(req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 200);
        let body = response_body_str(&resp);
        assert!(body.contains(r#""loaded":false"#));
        assert!(body.contains(r#""nodes":0"#));
        assert!(body.contains(r#""ticks":0"#));
    }

    #[test]
    fn test_handle_post_tick() {
        let (mut exec, mut ch, mut ps) = make_env();

        // Load a DAG first
        let mut dag = Dag::new();
        dag.constant(42.0).unwrap();
        let cbor = encode_dag(&dag);
        let header = b"POST /api/dag HTTP/1.1\r\nContent-Type: application/cbor\r\n\r\n";
        let mut req = header.to_vec();
        req.extend_from_slice(&cbor);
        handle_request(&req, &mut exec, &mut ch, &mut ps);

        // Now tick
        let tick_req = b"POST /api/tick HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let resp = handle_request(tick_req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 200);
        let body = response_body_str(&resp);
        assert!(body.contains(r#""ok":true"#));
    }

    #[test]
    fn test_handle_post_tick_no_dag() {
        let (mut exec, mut ch, mut ps) = make_env();
        let req = b"POST /api/tick HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let resp = handle_request(req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 400);
        let body = response_body_str(&resp);
        assert!(body.contains("No DAG loaded"));
    }

    #[test]
    fn test_handle_unknown_path() {
        let (mut exec, mut ch, mut ps) = make_env();
        let req = b"GET /nonexistent HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let resp = handle_request(req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 404);
    }

    #[test]
    fn test_handle_bad_request() {
        let (mut exec, mut ch, mut ps) = make_env();
        let req = b"\xff\xfe garbage not http";
        let resp = handle_request(req, &mut exec, &mut ch, &mut ps);
        let status = response_status(&resp);
        assert_eq!(status, 400);
        let body = response_body_str(&resp);
        assert!(body.contains("Bad request"));
    }
}
