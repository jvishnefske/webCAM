//! Minimal DHCPv4 responder for point-to-point links.
//!
//! Pure functions that parse DHCP requests and build fixed responses.
//! No I/O, no async — only `core` types. The imperative shell (firmware
//! or an async helper) handles UDP socket I/O and calls these functions.
//!
//! [`DhcpConfig`] makes server/client IPs configurable per board.
//! Only DHCPDISCOVER → DHCPOFFER and DHCPREQUEST → DHCPACK are handled.

/// Maximum size of a DHCP response packet.
pub const DHCP_RESPONSE_MAX: usize = 300;

/// DHCP message types recognised from the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpMessageType {
    /// DHCPDISCOVER (option 53 value 1).
    Discover,
    /// DHCPREQUEST (option 53 value 3).
    Request,
}

/// Fields extracted from a valid DHCP client request.
#[derive(Debug)]
pub struct ParsedRequest {
    /// Transaction ID from the client.
    pub xid: [u8; 4],
    /// Client hardware address (16 bytes, padded).
    pub chaddr: [u8; 16],
    /// Flags field (broadcast flag in bit 15).
    pub flags: [u8; 2],
    /// Parsed DHCP message type.
    pub message_type: DhcpMessageType,
}

/// Configuration for the DHCP responder.
///
/// Holds the server, client, subnet, and lease parameters.
/// [`Default`] provides link-local values suitable for CDC NCM:
/// server 169.254.1.61, client 169.254.1.62, mask 255.255.0.0,
/// lease 3600 seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhcpConfig {
    /// IPv4 address of the DHCP server (this device).
    pub server_ip: [u8; 4],
    /// IPv4 address to offer the client.
    pub client_ip: [u8; 4],
    /// Subnet mask to include in the offer.
    pub subnet_mask: [u8; 4],
    /// Lease duration in seconds.
    pub lease_time_secs: u32,
}

impl Default for DhcpConfig {
    fn default() -> Self {
        Self {
            server_ip: [169, 254, 1, 61],
            client_ip: [169, 254, 1, 62],
            subnet_mask: [255, 255, 0, 0],
            lease_time_secs: 3600,
        }
    }
}

const DHCP_MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];

/// Parse a DHCP client message from raw UDP payload.
///
/// Returns `None` if the packet is too short, not a BOOTREQUEST (op=1),
/// has a wrong magic cookie, or lacks a DHCP message type option (53)
/// with value 1 (Discover) or 3 (Request).
pub fn parse_request(data: &[u8]) -> Option<ParsedRequest> {
    // Minimum: 240 bytes (BOOTP fixed fields + 4-byte magic cookie)
    if data.len() < 240 {
        return None;
    }

    // op must be BOOTREQUEST (1)
    if data[0] != 1 {
        return None;
    }

    // Magic cookie at offset 236
    if data[236..240] != DHCP_MAGIC_COOKIE {
        return None;
    }

    // Extract fixed fields
    let mut xid = [0u8; 4];
    xid.copy_from_slice(&data[4..8]);

    let mut flags = [0u8; 2];
    flags.copy_from_slice(&data[10..12]);

    let mut chaddr = [0u8; 16];
    chaddr.copy_from_slice(&data[28..44]);

    // Scan TLV options starting at offset 240 for option 53 (message type)
    let mut pos = 240;
    let mut message_type = None;

    while pos < data.len() {
        let option_code = data[pos];

        // Pad option
        if option_code == 0 {
            pos += 1;
            continue;
        }

        // End option
        if option_code == 255 {
            break;
        }

        // Need at least one more byte for length
        if pos + 1 >= data.len() {
            break;
        }

        let option_len = data[pos + 1] as usize;

        // Option 53: DHCP Message Type (length must be 1)
        if option_code == 53 && option_len == 1 && pos + 2 < data.len() {
            message_type = match data[pos + 2] {
                1 => Some(DhcpMessageType::Discover),
                3 => Some(DhcpMessageType::Request),
                _ => None,
            };
        }

        pos += 2 + option_len;
    }

    let message_type = message_type?;

    Some(ParsedRequest {
        xid,
        chaddr,
        flags,
        message_type,
    })
}

/// Build a DHCP response (OFFER or ACK) into the provided buffer.
///
/// Returns the number of bytes written. The response type is determined
/// by the request type: Discover → Offer (2), Request → Ack (5).
pub fn build_response(
    request: &ParsedRequest,
    config: &DhcpConfig,
    buf: &mut [u8; DHCP_RESPONSE_MAX],
) -> usize {
    // Zero the buffer
    *buf = [0u8; DHCP_RESPONSE_MAX];

    // BOOTP fixed fields (236 bytes)
    buf[0] = 2; // op: BOOTREPLY
    buf[1] = 1; // htype: Ethernet
    buf[2] = 6; // hlen: 6 bytes MAC

    // xid (offset 4)
    buf[4..8].copy_from_slice(&request.xid);

    // flags (offset 10)
    buf[10..12].copy_from_slice(&request.flags);

    // yiaddr: offered client address (offset 16)
    buf[16..20].copy_from_slice(&config.client_ip);

    // siaddr: server address (offset 20)
    buf[20..24].copy_from_slice(&config.server_ip);

    // chaddr (offset 28)
    buf[28..44].copy_from_slice(&request.chaddr);

    // Magic cookie (offset 236)
    buf[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    // DHCP options start at offset 240
    let mut pos = 240;

    // Option 53: DHCP Message Type (1 byte)
    buf[pos] = 53;
    buf[pos + 1] = 1;
    buf[pos + 2] = match request.message_type {
        DhcpMessageType::Discover => 2, // DHCPOFFER
        DhcpMessageType::Request => 5,  // DHCPACK
    };
    pos += 3;

    // Option 1: Subnet Mask (4 bytes)
    buf[pos] = 1;
    buf[pos + 1] = 4;
    buf[pos + 2..pos + 6].copy_from_slice(&config.subnet_mask);
    pos += 6;

    // Option 51: Lease Time (4 bytes)
    buf[pos] = 51;
    buf[pos + 1] = 4;
    buf[pos + 2..pos + 6].copy_from_slice(&config.lease_time_secs.to_be_bytes());
    pos += 6;

    // Option 54: Server Identifier (4 bytes)
    buf[pos] = 54;
    buf[pos + 1] = 4;
    buf[pos + 2..pos + 6].copy_from_slice(&config.server_ip);
    pos += 6;

    // End option
    buf[pos] = 255;
    pos += 1;

    pos
}

/// Async DHCP responder loop for embassy-net.
///
/// Listens on the given UDP socket (which must already be bound to port 67)
/// and replies to DHCPDISCOVER with DHCPOFFER and DHCPREQUEST with DHCPACK,
/// sending responses to the broadcast address 255.255.255.255:68.
///
/// This function never returns.
#[cfg(feature = "embassy")]
pub async fn run_dhcp_responder(
    socket: &mut embassy_net::udp::UdpSocket<'_>,
    config: &DhcpConfig,
) -> ! {
    use embassy_net::{IpAddress, IpEndpoint};

    let mut recv_buf = [0u8; 576];

    loop {
        let (n, _from) = match socket.recv_from(&mut recv_buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(request) = parse_request(&recv_buf[..n]) {
            let mut resp_buf = [0u8; DHCP_RESPONSE_MAX];
            let resp_len = build_response(&request, config, &mut resp_buf);

            let dest = IpEndpoint::new(IpAddress::v4(255, 255, 255, 255), 68);
            let _ = socket.send_to(&resp_buf[..resp_len], dest).await;
        }
    }
}
