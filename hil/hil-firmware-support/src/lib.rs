//! Shared firmware support library for HIL board binaries.
//!
//! Extracts the repeating USB device configuration, descriptor buffer
//! allocation, vendor interface setup, CDC NCM class creation, and network
//! stack initialization into reusable helpers. Each new board binary only
//! needs to provide board-specific logic (interrupt bindings, pin assignments,
//! I2C buses, USB handler type).
//!
//! # Embassy task macro
//!
//! The [`define_ncm_tasks`] macro generates the four standard embassy tasks
//! (`usb_task`, `usb_ncm_task`, `net_task`, `dhcp_responder_task`) that every
//! CDC NCM board binary needs. These must be defined in the binary crate
//! because `#[embassy_executor::task]` generates statics that must live there.
//!
//! # Composing Multiple USB Functions
//!
//! embassy-usb supports composite USB devices — a single USB peripheral
//! presenting multiple independent functions to the host. Each function
//! gets its own Interface Association Descriptor (IAD) so Linux binds a
//! separate driver per function.
//!
//! ## Device-level setup
//!
//! [`usb_device_config`] enables IAD at the device descriptor level so the
//! host groups interfaces into functions:
//!
//! ```ignore
//! let mut config = embassy_usb::Config::new(VID, PID);
//! config.composite_with_iads = true;
//! config.device_class = 0xEF;      // Misc
//! config.device_sub_class = 0x02;  // Common
//! config.device_protocol = 0x01;   // IAD
//! ```
//!
//! ## Adding functions to the builder
//!
//! Each call to `builder.function(class, subclass, protocol)` opens a new
//! IAD group. Within that group, create one or more interfaces and their
//! alt settings. [`allocate_vendor_interface`] wraps the vendor-class case.
//! Scope the function/interface borrows so the builder is available again
//! for the next function:
//!
//! ```ignore
//! // Function 0: vendor interface (EP0 control transfers only)
//! let if0 = {
//!     let mut func = builder.function(0xFF, 0, 0);
//!     let mut iface = func.interface();
//!     let if_num = iface.interface_number();
//!     let _alt = iface.alt_setting(0xFF, 0, 0, None);
//!     if_num
//! };
//!
//! // Function 1: vendor interface with bulk endpoints
//! let (ep_in, ep_out) = {
//!     let mut func = builder.function(0xFF, 0xFF, 0xFF);
//!     let mut iface = func.interface();
//!     let mut alt = iface.alt_setting(0xFF, 0xFF, 0xFF, None);
//!     (alt.endpoint_bulk_in(64), alt.endpoint_bulk_out(64))
//! };
//!
//! // Function 2: CDC NCM (uses embassy-usb's built-in class)
//! let ncm_class = CdcNcmClass::new(&mut builder, ncm_state, mac, 64);
//! ```
//!
//! ## Registering handlers
//!
//! EP0 control transfers (vendor requests) are dispatched through
//! [`embassy_usb::Handler`] trait objects registered on the builder. Each
//! handler inspects the request and returns `None` to pass it to the next:
//!
//! ```ignore
//! // Handler must live in a StaticCell for 'static lifetime
//! static HANDLER: StaticCell<MyHandler> = StaticCell::new();
//! let handler = HANDLER.init(MyHandler::new());
//! handler.set_interface_numbers(if0, if1);
//! builder.handler(handler);
//! ```
//!
//! Multiple handlers can be registered. embassy-usb calls each in order
//! until one returns `Some`. The handler discriminates on interface number
//! or `bRequest` to claim only its own transfers.
//!
//! ## Bulk endpoints in async tasks
//!
//! Bulk endpoints cannot be used inside `Handler` (synchronous). Move them
//! into a dedicated `#[embassy_executor::task]`. See
//! [`gs_usb_device`](https://docs.rs/gs-usb-device) for the complete
//! CAN loopback example.
//!
//! ## Static allocation
//!
//! All handler state and USB descriptor buffers must be `'static`. Use
//! [`StaticCell`] for anything that outlives
//! `main`'s stack frame. [`create_usb_builder`] allocates the standard
//! descriptor buffers:
//!
//! ```ignore
//! static CONFIG_DESCRIPTOR: StaticCell<[u8; 512]> = StaticCell::new();
//! static GS_HANDLER: StaticCell<GsUsbHandler<'static>> = StaticCell::new();
//! ```
//!
//! Size the config descriptor buffer large enough for all functions. A
//! dual-vendor + CDC NCM composite needs ~512 bytes; simpler composites
//! fit in 256.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_in_result)]

pub mod fw_update;
#[cfg(feature = "tcp")]
pub mod http_static;
pub mod ws_dispatch;
pub mod ws_framing;
#[cfg(feature = "tcp")]
pub mod ws_server;
pub mod runtime_buses;

#[cfg(feature = "embassy")]
use embassy_net::{
    ConfigV6, Ipv4Address, Ipv4Cidr, Ipv6Address, Ipv6Cidr, Stack, StackResources, StaticConfigV4,
    StaticConfigV6,
};
#[cfg(feature = "embassy")]
use embassy_usb::class::cdc_ncm::embassy_net::Device as NcmDevice;
#[cfg(feature = "embassy")]
use embassy_usb::class::cdc_ncm::{self, CdcNcmClass};
#[cfg(feature = "embassy")]
use embassy_usb::driver::Driver;
#[cfg(feature = "embassy")]
use embassy_usb::Builder;
#[cfg(feature = "embassy")]
use static_cell::StaticCell;
#[cfg(feature = "embassy")]
use usb_composite_dispatchers::i2c_tiny_usb;

/// Creates the standard USB device configuration for composite CDC NCM boards.
///
/// Configures the device as a composite USB device with IADs (Interface
/// Association Descriptors), using the i2c-tiny-usb VID/PID. Returns a
/// fully populated [`embassy_usb::Config`].
#[cfg(feature = "embassy")]
pub fn usb_device_config() -> embassy_usb::Config<'static> {
    let mut config = embassy_usb::Config::new(i2c_tiny_usb::VID, i2c_tiny_usb::PID);
    config.manufacturer = Some("i2c-tiny-usb");
    config.product = Some("i2c-tiny-usb");
    config.serial_number = Some("0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.composite_with_iads = true;
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config
}

/// Creates a USB builder with statically allocated descriptor buffers.
///
/// Allocates static buffers for config (512 bytes), BOS (256 bytes),
/// MS OS (256 bytes), and control (128 bytes) descriptors via
/// function-scoped `StaticCell`s. These are large enough for CDC NCM
/// plus multiple vendor IADs.
#[cfg(feature = "embassy")]
pub fn create_usb_builder<'d, D: Driver<'d>>(
    driver: D,
    config: embassy_usb::Config<'d>,
) -> Builder<'d, D> {
    static CONFIG_DESCRIPTOR: StaticCell<[u8; 512]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();

    let config_descriptor = CONFIG_DESCRIPTOR.init([0; 512]);
    let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);
    let msos_descriptor = MSOS_DESCRIPTOR.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 128]);

    Builder::new(
        driver,
        config,
        config_descriptor,
        bos_descriptor,
        msos_descriptor,
        control_buf,
    )
}

/// Allocates a vendor-class USB interface (class 0xFF) on the builder.
///
/// Creates a function with a single interface and alt-setting, all set to
/// vendor-specific class (0xFF). Returns the [`embassy_usb::types::InterfaceNumber`]
/// for later handler registration.
#[cfg(feature = "embassy")]
pub fn allocate_vendor_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
) -> embassy_usb::types::InterfaceNumber {
    let mut func = builder.function(0xFF, 0, 0);
    let mut iface = func.interface();
    let if_num = iface.interface_number();
    let _alt = iface.alt_setting(0xFF, 0, 0, None);
    if_num
}

/// Adds a CDC NCM Ethernet function to the USB builder.
///
/// Allocates static state for both the CDC NCM class and the embassy-net
/// bridge layer. Returns the NCM runner (to be spawned as a task) and the
/// network device (to be passed to [`create_net_stack`]).
///
/// Requires `'static` lifetime because the CDC NCM state is allocated in a
/// `StaticCell` and `cdc_ncm::State` is invariant over its lifetime parameter.
#[cfg(feature = "embassy")]
pub fn add_cdc_ncm<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    device_mac: [u8; 6],
    host_mac: [u8; 6],
) -> (
    cdc_ncm::embassy_net::Runner<'static, D, 1514>,
    NcmDevice<'static, 1514>,
) {
    static NCM_STATE: StaticCell<cdc_ncm::State> = StaticCell::new();
    let ncm_state = NCM_STATE.init(cdc_ncm::State::new());
    let ncm_class = CdcNcmClass::new(builder, ncm_state, device_mac, 64);

    static NET_STATE: StaticCell<cdc_ncm::embassy_net::State<1514, 4, 4>> = StaticCell::new();
    let net_state = NET_STATE.init(cdc_ncm::embassy_net::State::new());
    ncm_class.into_embassy_net_device(net_state, host_mac)
}

/// Creates the embassy-net network stack with static IPv4 and IPv6 addresses.
///
/// Configures the stack with the given IPv4/IPv6 addresses and prefix lengths,
/// no gateway or DNS servers. Uses a function-scoped `StaticCell` for the
/// stack resources (4 socket slots). Returns the stack handle and runner
/// (to be spawned as a task).
#[cfg(feature = "embassy")]
pub fn create_net_stack(
    net_device: NcmDevice<'static, 1514>,
    ipv4: [u8; 4],
    ipv4_prefix: u8,
    ipv6: [u16; 8],
    ipv6_prefix: u8,
) -> (
    Stack<'static>,
    embassy_net::Runner<'static, NcmDevice<'static, 1514>>,
) {
    let [a, b, c, d] = ipv4;
    let mut net_config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(a, b, c, d), ipv4_prefix),
        gateway: None,
        dns_servers: heapless::Vec::new(),
    });
    let [s0, s1, s2, s3, s4, s5, s6, s7] = ipv6;
    net_config.ipv6 = ConfigV6::Static(StaticConfigV6 {
        address: Ipv6Cidr::new(
            Ipv6Address::new(s0, s1, s2, s3, s4, s5, s6, s7),
            ipv6_prefix,
        ),
        gateway: None,
        dns_servers: heapless::Vec::new(),
    });

    static RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());
    embassy_net::new(net_device, net_config, resources, 0x12345678)
}

/// Defines the four standard embassy tasks for a CDC NCM board binary.
///
/// Generates `usb_task`, `usb_ncm_task`, `net_task`, and `dhcp_responder_task`
/// with the correct types for the given USB driver. These must be defined in
/// the binary crate because `#[embassy_executor::task]` generates internal
/// statics that must live in the final binary.
///
/// # Usage
///
/// ```ignore
/// hil_firmware_support::define_ncm_tasks!(embassy_rp::usb::Driver<'static, USB>);
/// ```
#[cfg(feature = "embassy")]
#[macro_export]
macro_rules! define_ncm_tasks {
    ($driver:ty) => {
        /// Background task that runs the USB device stack.
        #[embassy_executor::task]
        async fn usb_task(mut device: embassy_usb::UsbDevice<'static, $driver>) -> ! {
            device.run().await
        }

        /// Background task that relays CDC NCM packets between USB and the network driver.
        #[embassy_executor::task]
        async fn usb_ncm_task(
            runner: embassy_usb::class::cdc_ncm::embassy_net::Runner<'static, $driver, 1514>,
        ) -> ! {
            runner.run().await
        }

        /// Background task that runs the embassy-net TCP/IP stack event loop.
        #[embassy_executor::task]
        async fn net_task(
            mut runner: embassy_net::Runner<
                'static,
                embassy_usb::class::cdc_ncm::embassy_net::Device<'static, 1514>,
            >,
        ) -> ! {
            runner.run().await
        }

        /// Background task that responds to DHCPv4 requests on the NCM link.
        ///
        /// Listens on UDP port 67 and replies with DHCPOFFER/DHCPACK.
        #[embassy_executor::task]
        async fn dhcp_responder_task(stack: embassy_net::Stack<'static>) -> ! {
            static RX_META: static_cell::StaticCell<[embassy_net::udp::PacketMetadata; 1]> =
                static_cell::StaticCell::new();
            static TX_META: static_cell::StaticCell<[embassy_net::udp::PacketMetadata; 1]> =
                static_cell::StaticCell::new();
            static RX_BUF: static_cell::StaticCell<[u8; 576]> = static_cell::StaticCell::new();
            static TX_BUF: static_cell::StaticCell<[u8; 576]> = static_cell::StaticCell::new();

            let rx_meta = RX_META.init([embassy_net::udp::PacketMetadata::EMPTY; 1]);
            let tx_meta = TX_META.init([embassy_net::udp::PacketMetadata::EMPTY; 1]);
            let rx_buf = RX_BUF.init([0; 576]);
            let tx_buf = TX_BUF.init([0; 576]);

            let mut socket =
                embassy_net::udp::UdpSocket::new(stack, rx_meta, rx_buf, tx_meta, tx_buf);
            socket.bind(67).expect("bind UDP port 67");

            hil_backplane::dhcp::run_dhcp_responder(
                &mut socket,
                &hil_backplane::dhcp::DhcpConfig::default(),
            )
            .await
        }
    };
}
