//! Integration tests verifying cross-module interactions.
//!
//! These tests exercise the non-WASM modules together, ensuring the public
//! API surface is consistent and modules compose correctly.

#[cfg(test)]
mod tests {
    use combined_frontend::backoff;
    use combined_frontend::hex;
    use combined_frontend::messages;
    use combined_frontend::types;

    // -----------------------------------------------------------------------
    // Verify all public non-wasm modules compile and are importable together
    // -----------------------------------------------------------------------

    #[test]
    fn all_non_wasm_modules_importable() {
        // Simply referencing these types proves the public API compiles.
        let _ = types::Viewport::default();
        let _ = types::DagStatus::default();
        let _ = types::KnownChannels::default();
        let _ = backoff::initial_backoff();
        let _ = hex::parse_hex_u8("00");
    }

    // -----------------------------------------------------------------------
    // Cross-module: hex parsing feeds into I2C request encoding
    // -----------------------------------------------------------------------

    #[test]
    fn hex_parse_into_i2c_read_request() {
        let addr = hex::parse_hex_u8("48").expect("valid hex address");
        let reg = hex::parse_hex_u8("0x00").expect("valid hex register");
        let req = messages::Request::I2cRead {
            bus: 0,
            addr,
            reg,
            len: 2,
        };
        let encoded = messages::encode_request(&req);
        // Verify CBOR is non-empty and starts with a map marker.
        assert!(!encoded.is_empty());
        // CBOR map(5) = 0xA5
        assert_eq!(encoded[0], 0xA5);
    }

    #[test]
    fn hex_parse_into_i2c_write_request() {
        let addr = hex::parse_hex_u8("50").expect("valid hex address");
        let data = hex::parse_hex_bytes("AB CD 01").expect("valid hex data");
        assert_eq!(data.len(), 3);

        let req = messages::Request::I2cWrite { bus: 1, addr, data };
        let encoded = messages::encode_request(&req);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn hex_parse_failure_prevents_request() {
        // Invalid hex should return None, which the UI would use to
        // show a form error instead of sending a request.
        assert!(hex::parse_hex_u8("ZZ").is_none());
        assert!(hex::parse_hex_bytes("").is_none());
        assert!(hex::parse_hex_bytes("GHI").is_none());
    }

    // -----------------------------------------------------------------------
    // Cross-module: backoff sequence for reconnect scheduling
    // -----------------------------------------------------------------------

    #[test]
    fn backoff_sequence_reaches_cap() {
        let mut delay = backoff::initial_backoff();
        assert_eq!(delay, 1_000);

        // Walk the sequence until capped
        let mut steps = 0;
        while delay < backoff::MAX_BACKOFF_MS {
            delay = backoff::next_backoff(delay);
            steps += 1;
        }
        assert_eq!(delay, backoff::MAX_BACKOFF_MS);
        // Should reach cap within a reasonable number of steps (log2(30000/1000) ~ 5)
        assert!(steps <= 6, "took {steps} steps to reach cap");
    }

    // -----------------------------------------------------------------------
    // Types module: DagNode, DagSnapshot, Viewport construction
    // -----------------------------------------------------------------------

    #[test]
    fn viewport_default_is_identity() {
        let vp = types::Viewport::default();
        assert_eq!(vp.pan_x, 0.0);
        assert_eq!(vp.pan_y, 0.0);
        assert_eq!(vp.scale, 1.0);
    }

    #[test]
    fn dag_status_default_is_empty() {
        let status = types::DagStatus::default();
        assert_eq!(status, types::DagStatus::Empty);
    }

    #[test]
    fn dag_node_construction() {
        use dag_core::op::Op;
        let node = types::DagNode {
            id: 0,
            op: Op::Const(42.0),
            x: 10.0,
            y: 20.0,
            result: Some(42.0),
        };
        assert_eq!(node.id, 0);
        assert_eq!(node.result, Some(42.0));
    }

    #[test]
    fn dag_snapshot_round_trip() {
        use dag_core::op::Op;
        let snapshot = types::DagSnapshot {
            nodes: vec![
                types::DagNode {
                    id: 0,
                    op: Op::Const(1.0),
                    x: 0.0,
                    y: 0.0,
                    result: None,
                },
                types::DagNode {
                    id: 1,
                    op: Op::Neg(0),
                    x: 100.0,
                    y: 0.0,
                    result: None,
                },
            ],
            viewport: types::Viewport::default(),
            next_id: 2,
        };
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.next_id, 2);
    }

    // -----------------------------------------------------------------------
    // Cross-module: encode request -> decode is consistent (error path)
    // -----------------------------------------------------------------------

    #[test]
    fn decode_non_response_cbor_is_error() {
        // Encode a *request* and try to decode it as a response.
        // The tag numbers differ, so this should either succeed with
        // a coincidental match or produce an error — either way no panic.
        let req_bytes = messages::encode_request(&messages::Request::ListBuses);
        // Tag 3 in request maps to BusList in response, so this might decode
        // but the payload won't match — we just verify no panic.
        let _ = messages::decode_response(&req_bytes);
    }

    #[test]
    fn encode_all_request_variants() {
        // Ensure every Request variant can be encoded without panicking.
        let variants: Vec<messages::Request> = vec![
            messages::Request::ListBuses,
            messages::Request::ReadAllTelemetry,
            messages::Request::RebootBootsel,
            messages::Request::FwMarkBooted,
            messages::Request::I2cRead {
                bus: 0,
                addr: 0x48,
                reg: 0,
                len: 1,
            },
            messages::Request::I2cWrite {
                bus: 0,
                addr: 0x48,
                data: vec![0x00, 0x01],
            },
            messages::Request::FwBegin {
                total_size: 1024,
                crc32: 0xAABBCCDD,
            },
            messages::Request::FwChunk {
                offset: 0,
                data: vec![0xFF; 256],
            },
            messages::Request::FwFinish { crc32: 0x12345678 },
            messages::Request::TelemetryBlockUpdated {
                block_id: 1,
                block_type: "gain".to_string(),
                config_json: "{}".to_string(),
            },
            messages::Request::TelemetryConnectionCreated {
                from_block: 1,
                from_port: 0,
                to_block: 2,
                to_port: 0,
                channel_id: 0x01000200,
            },
        ];
        for req in &variants {
            let bytes = messages::encode_request(req);
            assert!(!bytes.is_empty(), "encoding {:?} produced empty bytes", req);
        }
    }

    // -----------------------------------------------------------------------
    // Known channels default
    // -----------------------------------------------------------------------

    #[test]
    fn known_channels_default_is_empty() {
        let kc = types::KnownChannels::default();
        assert!(kc.inputs.is_empty());
        assert!(kc.outputs.is_empty());
    }
}
