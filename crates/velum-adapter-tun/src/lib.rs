//! Bounded, generation-aware flow ownership for platform TUN adapters.
//!
//! This crate deliberately contains no Android, file-descriptor, JNI, or UI
//! code. A platform host supplies packets and owns the TUN descriptor; this
//! crate rejects stale flows before they can reach `velum-client-runtime`.

#[cfg(target_os = "android")]
mod android;
mod fake_dns;

#[cfg(target_os = "android")]
pub use android::run_android_tun;
pub use fake_dns::{FakeDnsError, FakeDnsMapping, FakeDnsTable};

use std::{
    collections::{BTreeMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
};

use smoltcp::{
    iface::{Config as InterfaceConfig, Interface, PollResult, SocketSet},
    phy::{ChecksumCapabilities, Device, DeviceCapabilities, Medium, RxToken, TxToken},
    time::Instant,
    wire::{
        HardwareAddress, IpAddress, IpCidr, IpProtocol, Ipv4Packet, Ipv4Repr, Ipv6Packet, Ipv6Repr,
        TcpPacket, UdpPacket, UdpRepr,
    },
};
use velum_client_routing::{RouteContext, RoutingAction, RoutingPolicy};
use velum_client_runtime::{
    ClientError, ClientRuntime, DatagramSessionId, RuntimeError, RuntimeStream,
};

/// The transport protocol captured by a TUN adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FlowProtocol {
    Tcp,
    Udp,
}

/// A direction-independent IPv4 five-tuple identifying one captured flow.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlowKey {
    pub protocol: FlowProtocol,
    pub source: SocketAddr,
    pub destination: SocketAddr,
}

/// One policy decision after fake-IP restoration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TunRouteDecision {
    pub action: RoutingAction,
    pub destination: SocketAddr,
    pub domain: Option<String>,
}

/// Shared routing and DNS identity state used by every platform TUN host.
pub struct TunPolicyEngine {
    policy: RoutingPolicy,
    dns: FakeDnsTable,
}

impl TunPolicyEngine {
    pub fn new(
        generation: u64,
        policy: RoutingPolicy,
        dns_capacity: usize,
    ) -> Result<Self, FakeDnsError> {
        Ok(Self {
            policy,
            dns: FakeDnsTable::new(generation, dns_capacity)?,
        })
    }

    pub fn allocate_dns(
        &mut self,
        generation: u64,
        domain: &str,
        real_address: std::net::IpAddr,
        ttl: std::time::Duration,
        now: std::time::Instant,
    ) -> Result<std::net::IpAddr, FakeDnsError> {
        self.dns
            .allocate(generation, domain, real_address, ttl, now)
    }

    pub fn decide(
        &mut self,
        generation: u64,
        flow: FlowKey,
        now: std::time::Instant,
    ) -> Result<TunRouteDecision, FakeDnsError> {
        let mapping = self.dns.resolve(generation, flow.destination.ip(), now)?;
        let (destination, domain) = mapping.map_or_else(
            || (flow.destination, None),
            |mapping| {
                (
                    SocketAddr::new(mapping.real_address, flow.destination.port()),
                    Some(mapping.domain),
                )
            },
        );
        let action = self.policy.decide(RouteContext {
            domain: domain.as_deref(),
            destination: destination.ip(),
            destination_port: destination.port(),
        });
        Ok(TunRouteDecision {
            action,
            destination,
            domain,
        })
    }

    pub fn replace_generation(&mut self, generation: u64, policy: RoutingPolicy) {
        self.policy = policy;
        self.dns.replace_generation(generation);
    }
}

/// A packet that cannot safely enter the adapter flow table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketError {
    MalformedIpv4,
    MalformedIpv6,
    FragmentedIpv4,
    UnsupportedProtocol,
    MalformedTransport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketEncodeError {
    AddressFamilyMismatch,
    MtuExceeded,
}

/// Which IP endpoint a transparent TCP translation changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TcpAddressSide {
    Source,
    Destination,
}

/// Rewrites one IPv4 TCP endpoint and recalculates both affected checksums.
///
/// A transparent userspace TCP stack receives packets whose destination was
/// changed to an internal virtual address. Before a response returns to the
/// platform TUN fd, its source is restored to the original destination. The
/// relay target is never derived from the translated packet.
pub fn rewrite_tcp_ipv4_address(
    frame: &[u8],
    side: TcpAddressSide,
    address: std::net::Ipv4Addr,
) -> Result<Vec<u8>, PacketError> {
    if classify_ipv4_packet(frame)?.protocol != FlowProtocol::Tcp {
        return Err(PacketError::UnsupportedProtocol);
    }
    let original = Ipv4Packet::new_checked(frame).map_err(|_| PacketError::MalformedIpv4)?;
    let original_source: IpAddress = original.src_addr().into();
    let original_destination: IpAddress = original.dst_addr().into();
    let translated: IpAddress = address.into();
    let (source, destination) = match side {
        TcpAddressSide::Source => (translated, original_destination),
        TcpAddressSide::Destination => (original_source, translated),
    };
    let mut rewritten = frame.to_vec();
    let mut ipv4 =
        Ipv4Packet::new_checked(&mut rewritten).map_err(|_| PacketError::MalformedIpv4)?;
    match side {
        TcpAddressSide::Source => ipv4.set_src_addr(address),
        TcpAddressSide::Destination => ipv4.set_dst_addr(address),
    }
    ipv4.fill_checksum();
    let mut tcp =
        TcpPacket::new_checked(ipv4.payload_mut()).map_err(|_| PacketError::MalformedTransport)?;
    tcp.fill_checksum(&source, &destination);
    Ok(rewritten)
}

/// Extracts the payload from one validated IPv4 UDP TUN frame.
pub fn udp_payload(frame: &[u8]) -> Result<Vec<u8>, PacketError> {
    let key = classify_ipv4_packet(frame)?;
    if key.protocol != FlowProtocol::Udp {
        return Err(PacketError::UnsupportedProtocol);
    }
    let ipv4 = Ipv4Packet::new_checked(frame).map_err(|_| PacketError::MalformedIpv4)?;
    let transport =
        UdpPacket::new_checked(ipv4.payload()).map_err(|_| PacketError::MalformedTransport)?;
    Ok(transport.payload().to_vec())
}

/// Encodes a relay UDP response for delivery to its original TUN flow.
pub fn encode_udp_response(
    flow: FlowKey,
    payload: &[u8],
    mtu: usize,
) -> Result<Vec<u8>, PacketEncodeError> {
    let (source_ip, destination_ip) = match (flow.destination.ip(), flow.source.ip()) {
        (std::net::IpAddr::V4(source), std::net::IpAddr::V4(destination)) => (source, destination),
        (std::net::IpAddr::V6(source), std::net::IpAddr::V6(destination)) => {
            return encode_udp_response_ipv6(flow, source, destination, payload, mtu);
        }
        _ => return Err(PacketEncodeError::AddressFamilyMismatch),
    };
    let udp = UdpRepr {
        src_port: flow.destination.port(),
        dst_port: flow.source.port(),
    };
    let length = Ipv4Repr {
        src_addr: source_ip,
        dst_addr: destination_ip,
        next_header: IpProtocol::Udp,
        payload_len: udp.header_len() + payload.len(),
        hop_limit: 64,
    }
    .buffer_len()
        + udp.header_len()
        + payload.len();
    if length > mtu {
        return Err(PacketEncodeError::MtuExceeded);
    }
    let ipv4 = Ipv4Repr {
        src_addr: source_ip,
        dst_addr: destination_ip,
        next_header: IpProtocol::Udp,
        payload_len: udp.header_len() + payload.len(),
        hop_limit: 64,
    };
    let mut frame = vec![0; length];
    {
        let mut packet = Ipv4Packet::new_unchecked(&mut frame);
        ipv4.emit(&mut packet, &ChecksumCapabilities::default());
    }
    let source: IpAddress = source_ip.into();
    let destination: IpAddress = destination_ip.into();
    let mut packet = UdpPacket::new_unchecked(&mut frame[ipv4.buffer_len()..]);
    udp.emit(
        &mut packet,
        &source,
        &destination,
        payload.len(),
        |body| body.copy_from_slice(payload),
        &ChecksumCapabilities::default(),
    );
    Ok(frame)
}

fn encode_udp_response_ipv6(
    flow: FlowKey,
    source_ip: std::net::Ipv6Addr,
    destination_ip: std::net::Ipv6Addr,
    payload: &[u8],
    mtu: usize,
) -> Result<Vec<u8>, PacketEncodeError> {
    let udp = UdpRepr {
        src_port: flow.destination.port(),
        dst_port: flow.source.port(),
    };
    let ipv6 = Ipv6Repr {
        src_addr: source_ip,
        dst_addr: destination_ip,
        next_header: IpProtocol::Udp,
        payload_len: udp.header_len() + payload.len(),
        hop_limit: 64,
    };
    let length = ipv6.buffer_len() + udp.header_len() + payload.len();
    if length > mtu {
        return Err(PacketEncodeError::MtuExceeded);
    }
    let mut frame = vec![0; length];
    {
        let mut packet = Ipv6Packet::new_unchecked(&mut frame);
        ipv6.emit(&mut packet);
    }
    let source: IpAddress = source_ip.into();
    let destination: IpAddress = destination_ip.into();
    let mut packet = UdpPacket::new_unchecked(&mut frame[ipv6.buffer_len()..]);
    udp.emit(
        &mut packet,
        &source,
        &destination,
        payload.len(),
        |body| body.copy_from_slice(payload),
        &ChecksumCapabilities::default(),
    );
    Ok(frame)
}

/// Bounded layer-3 device between a platform TUN descriptor and `smoltcp`.
///
/// The Android/JNI boundary calls `push_inbound` after reading the TUN file
/// descriptor and drains `pop_outbound` to write generated packets back. The
/// device itself is entirely safe Rust and does not own an operating-system
/// descriptor.
pub struct TunFrameQueue {
    mtu: usize,
    capacity: usize,
    inbound: VecDeque<Vec<u8>>,
    outbound: VecDeque<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameQueueError {
    Oversized,
    Full,
}

impl TunFrameQueue {
    pub fn new(mtu: usize, capacity: usize) -> Result<Self, TunAdapterConfigError> {
        if mtu < 576 || capacity == 0 {
            return Err(TunAdapterConfigError::FrameQueue);
        }
        Ok(Self {
            mtu,
            capacity,
            inbound: VecDeque::with_capacity(capacity),
            outbound: VecDeque::with_capacity(capacity),
        })
    }

    pub fn push_inbound(&mut self, frame: Vec<u8>) -> Result<(), FrameQueueError> {
        if frame.len() > self.mtu {
            return Err(FrameQueueError::Oversized);
        }
        if self.inbound.len() == self.capacity {
            return Err(FrameQueueError::Full);
        }
        self.inbound.push_back(frame);
        Ok(())
    }

    pub fn pop_outbound(&mut self) -> Option<Vec<u8>> {
        self.outbound.pop_front()
    }

    fn pop_inbound(&mut self) -> Option<Vec<u8>> {
        self.inbound.pop_front()
    }

    fn push_outbound(&mut self, frame: Vec<u8>) -> Result<(), FrameQueueError> {
        if frame.len() > self.mtu {
            return Err(FrameQueueError::Oversized);
        }
        if self.outbound.len() == self.capacity {
            return Err(FrameQueueError::Full);
        }
        self.outbound.push_back(frame);
        Ok(())
    }
}

impl Device for TunFrameQueue {
    type RxToken<'a>
        = TunRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = TunTxToken<'a>
    where
        Self: 'a;

    fn receive(&mut self, _: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let frame = self.inbound.pop_front()?;
        Some((TunRxToken(frame), TunTxToken(self)))
    }

    fn transmit(&mut self, _: Instant) -> Option<Self::TxToken<'_>> {
        (self.outbound.len() < self.capacity).then_some(TunTxToken(self))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ip;
        capabilities.max_transmission_unit = self.mtu;
        capabilities.max_burst_size = Some(1);
        capabilities.checksum = ChecksumCapabilities::default();
        capabilities
    }
}

pub struct TunRxToken(Vec<u8>);

impl RxToken for TunRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}

pub struct TunTxToken<'a>(&'a mut TunFrameQueue);

impl TxToken for TunTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut frame = vec![0; len];
        let result = f(&mut frame);
        if frame.len() <= self.0.mtu && self.0.outbound.len() < self.0.capacity {
            self.0.outbound.push_back(frame);
        }
        result
    }
}

/// Configuration owned by one installed IPv4 TUN interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TunStackConfig {
    pub address: [u8; 4],
    pub prefix_len: u8,
    pub mtu: usize,
    pub frame_queue_capacity: usize,
}

impl TunStackConfig {
    pub const fn validate(self) -> Result<Self, TunAdapterConfigError> {
        if self.prefix_len > 32 {
            return Err(TunAdapterConfigError::Ipv4Prefix);
        }
        if self.mtu < 576 || self.frame_queue_capacity == 0 {
            return Err(TunAdapterConfigError::FrameQueue);
        }
        Ok(self)
    }
}

/// The safe polling core for one Android TUN installation.
pub struct TunStack {
    device: TunFrameQueue,
    interface: Interface,
    sockets: SocketSet<'static>,
}

impl TunStack {
    pub fn new(config: TunStackConfig, now: Instant) -> Result<Self, TunAdapterConfigError> {
        let config = config.validate()?;
        let mut device = TunFrameQueue::new(config.mtu, config.frame_queue_capacity)?;
        let mut interface =
            Interface::new(InterfaceConfig::new(HardwareAddress::Ip), &mut device, now);
        interface.update_ip_addrs(|addresses| {
            addresses
                .push(IpCidr::new(
                    IpAddress::v4(
                        config.address[0],
                        config.address[1],
                        config.address[2],
                        config.address[3],
                    ),
                    config.prefix_len,
                ))
                .expect("one IPv4 TUN address fits the interface address budget");
        });
        Ok(Self {
            device,
            interface,
            sockets: SocketSet::new(Vec::new()),
        })
    }

    pub fn push_inbound(&mut self, frame: Vec<u8>) -> Result<(), FrameQueueError> {
        self.device.push_inbound(frame)
    }

    /// Drives all currently queued input and scheduled protocol output once.
    pub fn poll(&mut self, now: Instant) -> PollResult {
        self.interface
            .poll(now, &mut self.device, &mut self.sockets)
    }

    pub fn pop_outbound(&mut self) -> Option<Vec<u8>> {
        self.device.pop_outbound()
    }
}

/// Classifies one raw TUN frame without retaining its payload.
///
/// Android TUN interfaces deliver layer-3 packets.
pub fn classify_ip_packet(frame: &[u8]) -> Result<FlowKey, PacketError> {
    match frame.first().map(|byte| byte >> 4) {
        Some(4) => classify_ipv4_packet(frame),
        Some(6) => classify_ipv6_packet(frame),
        _ => Err(PacketError::UnsupportedProtocol),
    }
}

/// Classifies one IPv4 TCP or UDP packet.
pub fn classify_ipv4_packet(frame: &[u8]) -> Result<FlowKey, PacketError> {
    let packet = Ipv4Packet::new_checked(frame).map_err(|_| PacketError::MalformedIpv4)?;
    let ipv4 = Ipv4Repr::parse(&packet, &ChecksumCapabilities::ignored())
        .map_err(|_| PacketError::FragmentedIpv4)?;
    let source_ip = ipv4.src_addr.into();
    let destination_ip = ipv4.dst_addr.into();
    let payload = packet.payload();
    let (protocol, source_port, destination_port) = match ipv4.next_header {
        IpProtocol::Tcp => {
            let transport =
                TcpPacket::new_checked(payload).map_err(|_| PacketError::MalformedTransport)?;
            (
                FlowProtocol::Tcp,
                transport.src_port(),
                transport.dst_port(),
            )
        }
        IpProtocol::Udp => {
            let transport =
                UdpPacket::new_checked(payload).map_err(|_| PacketError::MalformedTransport)?;
            (
                FlowProtocol::Udp,
                transport.src_port(),
                transport.dst_port(),
            )
        }
        _ => return Err(PacketError::UnsupportedProtocol),
    };
    if destination_port == 0 {
        return Err(PacketError::MalformedTransport);
    }
    Ok(FlowKey {
        protocol,
        source: SocketAddr::new(source_ip, source_port),
        destination: SocketAddr::new(destination_ip, destination_port),
    })
}

/// Classifies one IPv6 TCP or UDP packet without extension headers.
pub fn classify_ipv6_packet(frame: &[u8]) -> Result<FlowKey, PacketError> {
    let packet = Ipv6Packet::new_checked(frame).map_err(|_| PacketError::MalformedIpv6)?;
    let ipv6 = Ipv6Repr::parse(&packet).map_err(|_| PacketError::MalformedIpv6)?;
    let source_ip = ipv6.src_addr.into();
    let destination_ip = ipv6.dst_addr.into();
    let payload = packet.payload();
    let (protocol, source_port, destination_port) = match ipv6.next_header {
        IpProtocol::Tcp => {
            let transport =
                TcpPacket::new_checked(payload).map_err(|_| PacketError::MalformedTransport)?;
            (
                FlowProtocol::Tcp,
                transport.src_port(),
                transport.dst_port(),
            )
        }
        IpProtocol::Udp => {
            let transport =
                UdpPacket::new_checked(payload).map_err(|_| PacketError::MalformedTransport)?;
            (
                FlowProtocol::Udp,
                transport.src_port(),
                transport.dst_port(),
            )
        }
        _ => return Err(PacketError::UnsupportedProtocol),
    };
    if destination_port == 0 {
        return Err(PacketError::MalformedTransport);
    }
    Ok(FlowKey {
        protocol,
        source: SocketAddr::new(source_ip, source_port),
        destination: SocketAddr::new(destination_ip, destination_port),
    })
}

/// Limits held by one installed TUN interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TunAdapterLimits {
    pub max_tcp_flows: usize,
    pub max_udp_associations: usize,
}

impl TunAdapterLimits {
    pub const fn validate(self) -> Result<Self, TunAdapterConfigError> {
        if self.max_tcp_flows == 0 {
            return Err(TunAdapterConfigError::TcpFlowLimit);
        }
        if self.max_udp_associations == 0 {
            return Err(TunAdapterConfigError::UdpAssociationLimit);
        }
        Ok(self)
    }
}

impl Default for TunAdapterLimits {
    fn default() -> Self {
        Self {
            max_tcp_flows: 256,
            max_udp_associations: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TunAdapterConfigError {
    TcpFlowLimit,
    UdpAssociationLimit,
    FrameQueue,
    Ipv4Prefix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowAdmissionError {
    NotOnline,
    TcpFlowLimit,
    UdpAssociationLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlowLease {
    generation: u64,
}

/// Tracks flows owned by one TUN installation.
///
/// Flow leases are only valid while their captured runtime generation remains
/// online. Callers must release a lease when its platform flow closes; a
/// generation transition clears every remaining lease.
pub struct TunFlowRegistry {
    runtime: Arc<ClientRuntime>,
    limits: TunAdapterLimits,
    flows: BTreeMap<FlowKey, FlowLease>,
}

/// Bounded mapping between a captured UDP flow and one QUIC datagram session.
pub struct UdpAssociationTable {
    next_session: u64,
    by_flow: BTreeMap<FlowKey, DatagramSessionId>,
    by_session: BTreeMap<DatagramSessionId, FlowKey>,
    limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UdpAssociationError {
    NotUdp,
    Limit,
    UnknownSession,
    SourceMismatch,
}

#[derive(Debug)]
pub enum UdpForwardError {
    Packet(PacketError),
    Admission(FlowAdmissionError),
    Association(UdpAssociationError),
    Runtime(RuntimeError),
}

#[derive(Debug)]
pub enum UdpReceiveError {
    Runtime(RuntimeError),
    Association(UdpAssociationError),
    StaleFlow,
    Encode(PacketEncodeError),
}

#[derive(Debug)]
pub enum TcpRelayError {
    NotTcp,
    Admission(FlowAdmissionError),
    Runtime(RuntimeError),
    Client(ClientError),
    StaleFlow,
}

/// One reliable relay stream bound to an admitted TCP TUN flow.
///
/// The packet engine owns its TCP state machine and calls this object only for
/// application payloads. Its generation check prevents a replaced TUN or
/// runtime connection from publishing stale flow bytes.
pub struct TunTcpRelay {
    runtime: Arc<ClientRuntime>,
    flow: FlowKey,
    generation: u64,
    stream: RuntimeStream,
}

impl TunTcpRelay {
    pub fn flow(&self) -> FlowKey {
        self.flow
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub async fn write_all(&mut self, payload: &[u8]) -> Result<(), TcpRelayError> {
        self.ensure_active()?;
        self.stream
            .write_all(payload)
            .await
            .map_err(TcpRelayError::Client)?;
        self.ensure_active()
    }

    pub async fn read(&mut self, output: &mut [u8]) -> Result<Option<usize>, TcpRelayError> {
        self.ensure_active()?;
        let read = self
            .stream
            .read(output)
            .await
            .map_err(TcpRelayError::Client)?;
        self.ensure_active()?;
        Ok(read)
    }

    pub fn finish(&mut self) -> Result<(), TcpRelayError> {
        self.ensure_active()?;
        self.stream.finish().map_err(TcpRelayError::Client)
    }

    fn ensure_active(&self) -> Result<(), TcpRelayError> {
        self.runtime
            .is_generation_online(self.generation)
            .then_some(())
            .ok_or(TcpRelayError::StaleFlow)
    }
}

impl UdpAssociationTable {
    pub fn new(limit: usize) -> Result<Self, TunAdapterConfigError> {
        if limit == 0 {
            return Err(TunAdapterConfigError::UdpAssociationLimit);
        }
        Ok(Self {
            next_session: 1,
            by_flow: BTreeMap::new(),
            by_session: BTreeMap::new(),
            limit,
        })
    }

    pub fn session_for(&mut self, flow: FlowKey) -> Result<DatagramSessionId, UdpAssociationError> {
        if flow.protocol != FlowProtocol::Udp {
            return Err(UdpAssociationError::NotUdp);
        }
        if let Some(session) = self.by_flow.get(&flow) {
            return Ok(*session);
        }
        if self.by_flow.len() == self.limit {
            return Err(UdpAssociationError::Limit);
        }
        let session = self.allocate_session();
        self.by_flow.insert(flow, session);
        self.by_session.insert(session, flow);
        Ok(session)
    }

    /// Resolves a relay response only when its claimed source is the flow target.
    pub fn flow_for_response(
        &self,
        session: DatagramSessionId,
        source: SocketAddr,
    ) -> Result<FlowKey, UdpAssociationError> {
        let flow = *self
            .by_session
            .get(&session)
            .ok_or(UdpAssociationError::UnknownSession)?;
        if flow.destination != source {
            return Err(UdpAssociationError::SourceMismatch);
        }
        Ok(flow)
    }

    pub fn clear(&mut self) {
        self.by_flow.clear();
        self.by_session.clear();
    }

    fn allocate_session(&mut self) -> DatagramSessionId {
        loop {
            let candidate = self.next_session;
            self.next_session = self.next_session.wrapping_add(1);
            if self.next_session == 0 {
                self.next_session = 1;
            }
            let session = DatagramSessionId::new(candidate).expect("session id never zero");
            if !self.by_session.contains_key(&session) {
                return session;
            }
        }
    }
}

impl TunFlowRegistry {
    pub fn new(
        runtime: Arc<ClientRuntime>,
        limits: TunAdapterLimits,
    ) -> Result<Self, TunAdapterConfigError> {
        Ok(Self {
            runtime,
            limits: limits.validate()?,
            flows: BTreeMap::new(),
        })
    }

    /// Admits a captured flow for the currently online runtime generation.
    pub fn admit(&mut self, key: FlowKey) -> Result<u64, FlowAdmissionError> {
        let generation = self.current_generation()?;
        if let Some(lease) = self.flows.get(&key) {
            return if lease.generation == generation {
                Ok(generation)
            } else {
                self.flows.remove(&key);
                self.admit_new(key, generation)
            };
        }
        self.admit_new(key, generation)
    }

    /// Returns whether a previously admitted flow may still exchange bytes.
    pub fn is_active(&self, key: FlowKey, generation: u64) -> bool {
        self.runtime.is_generation_online(generation)
            && self
                .flows
                .get(&key)
                .is_some_and(|lease| lease.generation == generation)
    }

    /// Releases one platform flow and its bounded admission slot.
    pub fn release(&mut self, key: FlowKey) -> bool {
        self.flows.remove(&key).is_some()
    }

    /// Invalidates all leases after TUN replacement, disconnect, or reconnect.
    pub fn clear(&mut self) {
        self.flows.clear();
    }

    pub fn flow_count(&self) -> usize {
        self.flows.len()
    }

    /// Opens one reliable relay stream for an admitted TCP packet flow.
    pub async fn open_tcp(&mut self, flow: FlowKey) -> Result<TunTcpRelay, TcpRelayError> {
        if flow.protocol != FlowProtocol::Tcp {
            return Err(TcpRelayError::NotTcp);
        }
        let generation = self.admit(flow).map_err(TcpRelayError::Admission)?;
        let stream = self
            .runtime
            .open_stream(flow.destination)
            .await
            .map_err(TcpRelayError::Runtime)?;
        if !self.is_active(flow, generation) {
            return Err(TcpRelayError::StaleFlow);
        }
        Ok(TunTcpRelay {
            runtime: Arc::clone(&self.runtime),
            flow,
            generation,
            stream,
        })
    }

    /// Sends one TUN UDP payload through the active QUIC datagram path.
    pub async fn forward_udp(
        &mut self,
        frame: &[u8],
        associations: &mut UdpAssociationTable,
    ) -> Result<u64, UdpForwardError> {
        let flow = classify_ipv4_packet(frame).map_err(UdpForwardError::Packet)?;
        let payload = udp_payload(frame).map_err(UdpForwardError::Packet)?;
        let generation = self.admit(flow).map_err(UdpForwardError::Admission)?;
        let session = associations
            .session_for(flow)
            .map_err(UdpForwardError::Association)?;
        self.runtime
            .send_datagram(session, flow.destination, &payload)
            .await
            .map_err(UdpForwardError::Runtime)?;
        if !self.is_active(flow, generation) {
            return Err(UdpForwardError::Admission(FlowAdmissionError::NotOnline));
        }
        Ok(generation)
    }

    /// Receives one relay datagram and encodes it for the original TUN flow.
    pub async fn receive_udp(
        &self,
        associations: &UdpAssociationTable,
        mtu: usize,
    ) -> Result<Vec<u8>, UdpReceiveError> {
        let response = self
            .runtime
            .receive_datagram()
            .await
            .map_err(UdpReceiveError::Runtime)?;
        let flow = associations
            .flow_for_response(response.session_id, response.source)
            .map_err(UdpReceiveError::Association)?;
        let generation = self.runtime.snapshot().generation;
        if !self.is_active(flow, generation) {
            return Err(UdpReceiveError::StaleFlow);
        }
        encode_udp_response(flow, &response.payload, mtu).map_err(UdpReceiveError::Encode)
    }

    fn current_generation(&self) -> Result<u64, FlowAdmissionError> {
        let snapshot = self.runtime.snapshot();
        if self.runtime.is_generation_online(snapshot.generation) {
            Ok(snapshot.generation)
        } else {
            Err(FlowAdmissionError::NotOnline)
        }
    }

    fn admit_new(&mut self, key: FlowKey, generation: u64) -> Result<u64, FlowAdmissionError> {
        let used = self
            .flows
            .keys()
            .filter(|candidate| candidate.protocol == key.protocol)
            .count();
        let limit = match key.protocol {
            FlowProtocol::Tcp => self.limits.max_tcp_flows,
            FlowProtocol::Udp => self.limits.max_udp_associations,
        };
        if used >= limit {
            return Err(match key.protocol {
                FlowProtocol::Tcp => FlowAdmissionError::TcpFlowLimit,
                FlowProtocol::Udp => FlowAdmissionError::UdpAssociationLimit,
            });
        }
        self.flows.insert(key, FlowLease { generation });
        Ok(generation)
    }
}

/// Drives bounded IPv4 UDP traffic between a platform TUN descriptor and the
/// active runtime. The platform owns descriptor I/O; this engine owns neither
/// file descriptors nor Android lifecycle state.
pub struct TunUdpEngine {
    queue: TunFrameQueue,
    registry: TunFlowRegistry,
    associations: UdpAssociationTable,
}

#[derive(Debug)]
pub enum TunUdpEngineError {
    Queue(FrameQueueError),
    Forward(UdpForwardError),
    Receive(UdpReceiveError),
}

impl TunUdpEngine {
    pub fn new(
        runtime: Arc<ClientRuntime>,
        limits: TunAdapterLimits,
        mtu: usize,
        queue_capacity: usize,
    ) -> Result<Self, TunAdapterConfigError> {
        Ok(Self {
            queue: TunFrameQueue::new(mtu, queue_capacity)?,
            associations: UdpAssociationTable::new(limits.max_udp_associations)?,
            registry: TunFlowRegistry::new(runtime, limits)?,
        })
    }

    /// Takes one packet read from a platform TUN descriptor.
    pub fn push_inbound(&mut self, frame: Vec<u8>) -> Result<(), FrameQueueError> {
        self.queue.push_inbound(frame)
    }

    /// Forwards one queued UDP packet, if any, through the authenticated relay.
    pub async fn forward_next(&mut self) -> Result<Option<u64>, TunUdpEngineError> {
        let Some(frame) = self.queue.pop_inbound() else {
            return Ok(None);
        };
        self.registry
            .forward_udp(&frame, &mut self.associations)
            .await
            .map(Some)
            .map_err(TunUdpEngineError::Forward)
    }

    /// Receives one relay datagram and queues its authenticated IP response.
    pub async fn receive_next(&mut self) -> Result<(), TunUdpEngineError> {
        let frame = self
            .registry
            .receive_udp(&self.associations, self.queue.mtu)
            .await
            .map_err(TunUdpEngineError::Receive)?;
        self.queue
            .push_outbound(frame)
            .map_err(TunUdpEngineError::Queue)
    }

    /// Returns the next packet that the platform host must write to its TUN fd.
    pub fn pop_outbound(&mut self) -> Option<Vec<u8>> {
        self.queue.pop_outbound()
    }

    /// Invalidates all packet-to-runtime associations after stop or replacement.
    pub fn clear(&mut self) {
        self.registry.clear();
        self.associations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(protocol: FlowProtocol, source_port: u16) -> FlowKey {
        FlowKey {
            protocol,
            source: SocketAddr::from(([10, 8, 0, 2], source_port)),
            destination: SocketAddr::from(([192, 0, 2, 10], 443)),
        }
    }

    fn ipv4_packet(protocol: u8, transport: &[u8]) -> Vec<u8> {
        let total = 20 + transport.len();
        let mut packet = vec![0; total];
        packet[0] = 0x45;
        packet[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        packet[8] = 64;
        packet[9] = protocol;
        packet[12..16].copy_from_slice(&[10, 8, 0, 2]);
        packet[16..20].copy_from_slice(&[192, 0, 2, 10]);
        packet[20..].copy_from_slice(transport);
        packet
    }

    fn ipv6_packet(protocol: u8, transport: &[u8]) -> Vec<u8> {
        let mut packet = vec![0; 40 + transport.len()];
        packet[0] = 0x60;
        packet[4..6].copy_from_slice(&(transport.len() as u16).to_be_bytes());
        packet[6] = protocol;
        packet[7] = 64;
        packet[8..24].copy_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
        packet[24..40].copy_from_slice(
            &"2001:db8::1"
                .parse::<std::net::Ipv6Addr>()
                .expect("IPv6")
                .octets(),
        );
        packet[40..].copy_from_slice(transport);
        packet
    }

    fn tcp_packet() -> Vec<u8> {
        let mut tcp = [0_u8; 20];
        tcp[..2].copy_from_slice(&50_000_u16.to_be_bytes());
        tcp[2..4].copy_from_slice(&443_u16.to_be_bytes());
        tcp[12] = 0x50;
        ipv4_packet(6, &tcp)
    }

    #[test]
    fn classifies_ipv4_udp_without_retaining_payload() {
        let packet = ipv4_packet(17, &[0xc3, 0x50, 0x00, 0x35, 0x00, 0x08, 0x00, 0x00]);

        assert_eq!(
            classify_ipv4_packet(&packet),
            Ok(FlowKey {
                protocol: FlowProtocol::Udp,
                source: SocketAddr::from(([10, 8, 0, 2], 50_000)),
                destination: SocketAddr::from(([192, 0, 2, 10], 53)),
            })
        );
    }

    #[test]
    fn classifies_and_encodes_ipv6_udp() {
        let packet = ipv6_packet(17, &[0xc3, 0x50, 0x00, 0x35, 0x00, 0x08, 0x00, 0x00]);
        let flow = classify_ip_packet(&packet).expect("IPv6 flow");
        assert_eq!(flow.protocol, FlowProtocol::Udp);
        assert_eq!(flow.source.port(), 50_000);
        assert_eq!(flow.destination.port(), 53);
        assert!(flow.destination.is_ipv6());

        let response = encode_udp_response(flow, &[1, 2, 3], 1280).expect("response");
        let response_flow = classify_ip_packet(&response).expect("response flow");
        assert_eq!(response_flow.source, flow.destination);
        assert_eq!(response_flow.destination, flow.source);
    }

    #[test]
    fn fake_dns_restores_domain_before_shared_policy_decision() {
        use velum_client_routing::{RoutingRule, RuleMatcher};

        let policy = RoutingPolicy::new(vec![
            RoutingRule::new(
                RuleMatcher::domain_suffix("example.com").expect("domain"),
                RoutingAction::Node("node-sg".into()),
            ),
            RoutingRule::new(RuleMatcher::Match, RoutingAction::Direct),
        ])
        .expect("policy");
        let now = std::time::Instant::now();
        let mut engine = TunPolicyEngine::new(3, policy, 32).expect("engine");
        let fake = engine
            .allocate_dns(
                3,
                "api.example.com",
                "2001:db8::20".parse().expect("real IP"),
                std::time::Duration::from_secs(60),
                now,
            )
            .expect("fake IP");
        let decision = engine
            .decide(
                3,
                FlowKey {
                    protocol: FlowProtocol::Tcp,
                    source: "[fd00:19::2]:50000".parse().expect("source"),
                    destination: SocketAddr::new(fake, 443),
                },
                now,
            )
            .expect("decision");
        assert_eq!(decision.action, RoutingAction::Node("node-sg".into()));
        assert_eq!(decision.domain.as_deref(), Some("api.example.com"));
        assert_eq!(decision.destination, "[2001:db8::20]:443".parse().unwrap());
    }

    #[test]
    fn rejects_unknown_or_truncated_transport_packets() {
        assert_eq!(
            classify_ipv4_packet(&ipv4_packet(1, &[])),
            Err(PacketError::UnsupportedProtocol)
        );
        assert_eq!(
            classify_ipv4_packet(&ipv4_packet(6, &[0; 12])),
            Err(PacketError::MalformedTransport)
        );
    }

    #[test]
    fn rewrites_tcp_endpoint_addresses_and_recalculates_checksums() {
        let original = tcp_packet();
        let internal = std::net::Ipv4Addr::new(10, 255, 0, 1);
        let rewritten = rewrite_tcp_ipv4_address(&original, TcpAddressSide::Destination, internal)
            .expect("rewrite destination");
        let restored = rewrite_tcp_ipv4_address(
            &rewritten,
            TcpAddressSide::Source,
            std::net::Ipv4Addr::new(192, 0, 2, 10),
        )
        .expect("rewrite source");

        let original_flow = classify_ipv4_packet(&original).expect("original flow");
        let translated_flow = classify_ipv4_packet(&rewritten).expect("translated flow");
        let restored_flow = classify_ipv4_packet(&restored).expect("restored flow");
        assert_eq!(translated_flow.source, original_flow.source);
        assert_eq!(
            translated_flow.destination.ip(),
            std::net::IpAddr::V4(internal)
        );
        assert_eq!(
            translated_flow.destination.port(),
            original_flow.destination.port()
        );
        assert_eq!(restored_flow.source.ip(), original_flow.destination.ip());
        assert_eq!(restored_flow.source.port(), original_flow.source.port());
        let packet = Ipv4Packet::new_checked(&restored).expect("IPv4");
        let tcp = TcpPacket::new_checked(packet.payload()).expect("TCP");
        assert!(packet.verify_checksum());
        assert!(tcp.verify_checksum(&packet.src_addr().into(), &packet.dst_addr().into()));
    }

    #[test]
    fn frame_queue_enforces_mtu_and_backpressure() {
        let mut queue = TunFrameQueue::new(576, 1).expect("queue");
        assert_eq!(
            queue.push_inbound(vec![0; 577]),
            Err(FrameQueueError::Oversized)
        );
        queue.push_inbound(vec![1]).expect("first frame");
        assert_eq!(queue.push_inbound(vec![2]), Err(FrameQueueError::Full));

        let tx = Device::transmit(&mut queue, Instant::from_millis(0)).expect("tx token");
        tx.consume(2, |frame| frame.copy_from_slice(&[7, 8]));
        assert_eq!(queue.pop_outbound(), Some(vec![7, 8]));
    }

    #[test]
    fn stack_validates_its_address_and_queue_configuration() {
        assert_eq!(
            TunStackConfig {
                address: [10, 8, 0, 1],
                prefix_len: 33,
                mtu: 1280,
                frame_queue_capacity: 4,
            }
            .validate(),
            Err(TunAdapterConfigError::Ipv4Prefix)
        );
        assert!(
            TunStack::new(
                TunStackConfig {
                    address: [10, 8, 0, 1],
                    prefix_len: 24,
                    mtu: 1280,
                    frame_queue_capacity: 4,
                },
                Instant::from_millis(0),
            )
            .is_ok()
        );
    }

    #[test]
    fn udp_associations_bind_responses_to_the_original_destination() {
        let flow = FlowKey {
            protocol: FlowProtocol::Udp,
            source: SocketAddr::from(([10, 8, 0, 2], 50_000)),
            destination: SocketAddr::from(([192, 0, 2, 53], 53)),
        };
        let mut associations = UdpAssociationTable::new(1).expect("limit");
        let session = associations.session_for(flow).expect("session");

        assert_eq!(associations.session_for(flow), Ok(session));
        assert_eq!(
            associations.flow_for_response(session, flow.destination),
            Ok(flow)
        );
        assert_eq!(
            associations.flow_for_response(session, SocketAddr::from(([192, 0, 2, 54], 53))),
            Err(UdpAssociationError::SourceMismatch)
        );
    }

    #[test]
    fn encodes_udp_responses_back_to_the_originating_tun_flow() {
        let original = FlowKey {
            protocol: FlowProtocol::Udp,
            source: SocketAddr::from(([10, 8, 0, 2], 50_000)),
            destination: SocketAddr::from(([192, 0, 2, 53], 53)),
        };
        let frame = encode_udp_response(original, &[1, 2, 3], 1280).expect("frame");

        assert_eq!(
            classify_ipv4_packet(&frame),
            Ok(FlowKey {
                protocol: FlowProtocol::Udp,
                source: original.destination,
                destination: original.source,
            })
        );
        assert_eq!(
            encode_udp_response(original, &[0; 2_000], 576),
            Err(PacketEncodeError::MtuExceeded)
        );
    }

    #[test]
    fn limits_require_positive_values() {
        assert_eq!(
            TunAdapterLimits {
                max_tcp_flows: 0,
                max_udp_associations: 1,
            }
            .validate(),
            Err(TunAdapterConfigError::TcpFlowLimit)
        );
        assert_eq!(
            TunAdapterLimits {
                max_tcp_flows: 1,
                max_udp_associations: 0,
            }
            .validate(),
            Err(TunAdapterConfigError::UdpAssociationLimit)
        );
    }

    #[test]
    fn offline_runtime_cannot_admit_packet_flows() {
        let runtime = Arc::new(ClientRuntime::new());
        let mut registry =
            TunFlowRegistry::new(runtime, TunAdapterLimits::default()).expect("limits");

        assert_eq!(
            registry.admit(flow(FlowProtocol::Tcp, 50_000)),
            Err(FlowAdmissionError::NotOnline)
        );
    }

    #[test]
    fn clear_releases_every_flow_slot() {
        let runtime = Arc::new(ClientRuntime::new());
        let mut registry = TunFlowRegistry::new(
            runtime,
            TunAdapterLimits {
                max_tcp_flows: 1,
                max_udp_associations: 1,
            },
        )
        .expect("limits");

        registry
            .flows
            .insert(flow(FlowProtocol::Tcp, 50_000), FlowLease { generation: 1 });
        registry
            .flows
            .insert(flow(FlowProtocol::Udp, 50_001), FlowLease { generation: 1 });
        registry.clear();

        assert_eq!(registry.flow_count(), 0);
    }

    #[tokio::test]
    async fn udp_engine_preserves_queue_backpressure_and_fails_closed_offline() {
        let runtime = Arc::new(ClientRuntime::new());
        let mut engine =
            TunUdpEngine::new(runtime, TunAdapterLimits::default(), 1280, 1).expect("engine");
        let packet = ipv4_packet(17, &[0xc3, 0x50, 0x00, 0x35, 0x00, 0x08, 0x00, 0x00]);

        engine.push_inbound(packet.clone()).expect("first packet");
        assert_eq!(engine.push_inbound(packet), Err(FrameQueueError::Full));
        assert!(matches!(
            engine.forward_next().await,
            Err(TunUdpEngineError::Forward(UdpForwardError::Admission(
                FlowAdmissionError::NotOnline
            )))
        ));
        assert_eq!(engine.forward_next().await.expect("empty queue"), None);
    }

    #[tokio::test]
    async fn tcp_relay_rejects_non_tcp_flows_before_opening_runtime_stream() {
        let runtime = Arc::new(ClientRuntime::new());
        let mut registry =
            TunFlowRegistry::new(runtime, TunAdapterLimits::default()).expect("registry");

        assert!(matches!(
            registry.open_tcp(flow(FlowProtocol::Udp, 50_000)).await,
            Err(TcpRelayError::NotTcp)
        ));
    }
}
