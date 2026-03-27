#![allow(clippy::expect_used)]
use hil_backplane::dhcp::{
    build_response, parse_request, DhcpConfig, DhcpMessageType, DHCP_RESPONSE_MAX,
};

const DHCP_MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];

/// Build a minimal valid DHCPDISCOVER packet.
fn make_discover() -> [u8; 300] {
    let mut pkt = [0u8; 300];
    pkt[0] = 1; // op: BOOTREQUEST
    pkt[1] = 1; // htype: Ethernet
    pkt[2] = 6; // hlen

    // xid
    pkt[4] = 0xDE;
    pkt[5] = 0xAD;
    pkt[6] = 0xBE;
    pkt[7] = 0xEF;

    // flags: broadcast
    pkt[10] = 0x80;
    pkt[11] = 0x00;

    // chaddr: fake MAC
    pkt[28] = 0x02;
    pkt[29] = 0x03;
    pkt[30] = 0x04;
    pkt[31] = 0x05;
    pkt[32] = 0x06;
    pkt[33] = 0x08;

    // Magic cookie
    pkt[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    // Option 53: DHCP Message Type = 1 (Discover)
    pkt[240] = 53;
    pkt[241] = 1;
    pkt[242] = 1;

    // End option
    pkt[243] = 255;

    pkt
}

/// Build a minimal valid DHCPREQUEST packet.
fn make_request() -> [u8; 300] {
    let mut pkt = make_discover();
    // Change message type to Request (3)
    pkt[242] = 3;
    pkt
}

#[test]
fn parse_discover() {
    let pkt = make_discover();
    let parsed = parse_request(&pkt).expect("should parse discover");
    assert_eq!(parsed.message_type, DhcpMessageType::Discover);
    assert_eq!(parsed.xid, [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(parsed.flags, [0x80, 0x00]);
    assert_eq!(parsed.chaddr[0..6], [0x02, 0x03, 0x04, 0x05, 0x06, 0x08]);
}

#[test]
fn parse_request_type() {
    let pkt = make_request();
    let parsed = parse_request(&pkt).expect("should parse request");
    assert_eq!(parsed.message_type, DhcpMessageType::Request);
}

#[test]
fn reject_too_short() {
    let pkt = [0u8; 100];
    assert!(parse_request(&pkt).is_none());
}

#[test]
fn reject_bootreply() {
    let mut pkt = make_discover();
    pkt[0] = 2; // op: BOOTREPLY — not a client message
    assert!(parse_request(&pkt).is_none());
}

#[test]
fn reject_bad_cookie() {
    let mut pkt = make_discover();
    pkt[236] = 0xFF; // corrupt magic cookie
    assert!(parse_request(&pkt).is_none());
}

#[test]
fn reject_unknown_message_type() {
    let mut pkt = make_discover();
    pkt[242] = 7; // DHCPRELEASE — not handled
    assert!(parse_request(&pkt).is_none());
}

#[test]
fn build_offer_response() {
    let config = DhcpConfig::default();
    let pkt = make_discover();
    let parsed = parse_request(&pkt).unwrap();
    let mut buf = [0u8; DHCP_RESPONSE_MAX];
    let len = build_response(&parsed, &config, &mut buf);

    // BOOTREPLY
    assert_eq!(buf[0], 2);
    // xid copied
    assert_eq!(&buf[4..8], &[0xDE, 0xAD, 0xBE, 0xEF]);
    // flags copied
    assert_eq!(&buf[10..12], &[0x80, 0x00]);
    // yiaddr = 169.254.1.62
    assert_eq!(&buf[16..20], &config.client_ip);
    // siaddr = 169.254.1.61
    assert_eq!(&buf[20..24], &config.server_ip);
    // chaddr copied
    assert_eq!(buf[28], 0x02);
    // Magic cookie
    assert_eq!(&buf[236..240], &DHCP_MAGIC_COOKIE);
    // Option 53 = 2 (DHCPOFFER)
    assert_eq!(&buf[240..243], &[53, 1, 2]);
    // Option 1 = subnet mask
    assert_eq!(&buf[243..249], &[1, 4, 255, 255, 0, 0]);
    // Option 51 = lease time 3600
    assert_eq!(&buf[249..255], &[51, 4, 0, 0, 14, 16]);
    // Option 54 = server id
    assert_eq!(&buf[255..261], &[54, 4, 169, 254, 1, 61]);
    // End option
    assert_eq!(buf[261], 255);
    assert_eq!(len, 262);
}

#[test]
fn build_ack_response() {
    let config = DhcpConfig::default();
    let pkt = make_request();
    let parsed = parse_request(&pkt).unwrap();
    let mut buf = [0u8; DHCP_RESPONSE_MAX];
    let _len = build_response(&parsed, &config, &mut buf);

    // Option 53 = 5 (DHCPACK)
    assert_eq!(&buf[240..243], &[53, 1, 5]);
}

#[test]
fn parse_with_pad_options() {
    let mut pkt = make_discover();
    // Insert pad options (0) before option 53
    pkt[240] = 0; // pad
    pkt[241] = 0; // pad
    pkt[242] = 53;
    pkt[243] = 1;
    pkt[244] = 1; // Discover
    pkt[245] = 255; // End
    let parsed = parse_request(&pkt).expect("should parse with pads");
    assert_eq!(parsed.message_type, DhcpMessageType::Discover);
}

#[test]
fn parse_skips_other_options() {
    let mut pkt = make_discover();
    // Option 12 (hostname) with length 4 before option 53
    pkt[240] = 12;
    pkt[241] = 4;
    pkt[242] = b't';
    pkt[243] = b'e';
    pkt[244] = b's';
    pkt[245] = b't';
    // Then option 53
    pkt[246] = 53;
    pkt[247] = 1;
    pkt[248] = 1; // Discover
    pkt[249] = 255;
    let parsed = parse_request(&pkt).expect("should parse after skipping option 12");
    assert_eq!(parsed.message_type, DhcpMessageType::Discover);
}

#[test]
fn default_config_values() {
    let config = DhcpConfig::default();
    assert_eq!(config.server_ip, [169, 254, 1, 61]);
    assert_eq!(config.client_ip, [169, 254, 1, 62]);
    assert_eq!(config.subnet_mask, [255, 255, 0, 0]);
    assert_eq!(config.lease_time_secs, 3600);
}

#[test]
fn custom_config_in_response() {
    let config = DhcpConfig {
        server_ip: [10, 0, 0, 1],
        client_ip: [10, 0, 0, 2],
        subnet_mask: [255, 255, 255, 0],
        lease_time_secs: 7200,
    };
    let pkt = make_discover();
    let parsed = parse_request(&pkt).unwrap();
    let mut buf = [0u8; DHCP_RESPONSE_MAX];
    let len = build_response(&parsed, &config, &mut buf);

    // yiaddr = 10.0.0.2
    assert_eq!(&buf[16..20], &[10, 0, 0, 2]);
    // siaddr = 10.0.0.1
    assert_eq!(&buf[20..24], &[10, 0, 0, 1]);
    // Option 1 = subnet mask 255.255.255.0
    assert_eq!(&buf[243..249], &[1, 4, 255, 255, 255, 0]);
    // Option 51 = lease time 7200 (0x00001C20)
    assert_eq!(&buf[249..255], &[51, 4, 0, 0, 0x1C, 0x20]);
    // Option 54 = server id 10.0.0.1
    assert_eq!(&buf[255..261], &[54, 4, 10, 0, 0, 1]);
    assert_eq!(len, 262);
}
