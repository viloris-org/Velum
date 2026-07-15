# ADR-0014: Android TUN Data Plane

- **Status:** Proposed
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Stakeholders:** Android, transport, protocol, server, security, and release maintainers
- **Supersedes:** ADR-0013 only for the first Android traffic-adapter selection

## Context

The Android Flutter runner is currently a control application. It creates a
`velum-client-runtime` through the native ABI but installs no VPN service or
system traffic adapter. `velum-client-api` exposes authenticated reliable
streams to exact `SocketAddr` targets and bounded UDP datagrams; it does not
accept IP packets. The relay admission crate owns authentication and exact
destination policy, but it is not an Android exit implementation.

An Android VPN needs an OS-owned TUN interface and a packet engine that turns
captured IPv4 TCP, UDP, and DNS traffic into the existing direct flow APIs.
Installing a default route before that engine can forward and receive packets
would silently blackhole device traffic and is not acceptable.

## Decision Drivers

1. System traffic must fail closed rather than claim protection without a
   forwarding path.
2. Packet payloads, credentials, and destinations must not enter Flutter or
   diagnostics.
3. The Android foreground-service lifecycle must not leak into Rust protocol
   or carrier crates.
4. The first production-shaped slice needs TCP, UDP, and DNS rather than a
   UI-only VPN indicator.
5. The data plane must reuse the existing authenticated QUIC client path and
   server authorization boundary.

## Decision

Use Android `VpnService` as the platform traffic adapter. Its foreground
service owns user consent, TUN file-descriptor lifetime, route/DNS installation,
network-change handling, and notification lifecycle. It must call
`VpnService.protect` for relay sockets so the QUIC carrier never re-enters the
TUN interface.

Introduce an Android-native packet engine below the Flutter boundary. It owns
the TUN descriptor and converts:

- IPv4 TCP flows into generation-bound `ClientRuntime::open_stream` calls;
- IPv4 UDP associations, including configured DNS resolvers, into
  generation-bound `send_datagram` and `receive_datagram` calls;
- relay responses into packets written back to the TUN descriptor.

The engine is responsible for TCP/IP state, UDP association expiry, packet
limits, MTU handling, and all packet buffers. It may use a maintained
userspace IP-stack dependency, but that dependency must be pinned, license
reviewed, and covered by packet-vector tests before it is admitted. It must
not implement an ad-hoc TCP state machine in Kotlin or Flutter.

The existing direct stream and datagram protocols remain the first transport
mapping. The relay deployment owns external socket creation and exact
destination authorization. A VPN profile has an explicit resolver list and
must not infer hostnames from packet payloads; DNS is forwarded as UDP or TCP
to the configured resolver address under the same server policy.

## Runtime View

```text
Android applications
        |
        v
VpnService TUN <-> Android packet engine <-> client-runtime <-> client-api <-> QUIC relay
        ^                                      |                                  |
        |                                      +-- lifecycle snapshots            v
        +----------- response IP packets                              authorized TCP/UDP egress

Flutter UI ---- commands/status only ---- Android host / native control ABI
```

## Invariants

- The Android service does not call `Builder.establish()` with a default route
  until the packet engine reports that the current runtime generation is online
  and able to accept packets.
- Loss of the packet engine, runtime generation, TUN descriptor, or relay
  connection closes the TUN interface and removes installed routes.
- Every relay socket is protected before connection; failure to protect is a
  start failure.
- A packet flow is bound to one runtime generation. It cannot publish data
  after stop, reconnect, or TUN replacement.
- Flutter receives redacted lifecycle and aggregate counter snapshots only;
  no packet bytes, destination addresses, credentials, certificate bytes, or
  raw tokens cross its platform channel.
- The initial supported profile is IPv4. IPv6, per-app selection, always-on,
  and kill-switch mode each require their own release evidence and are not
  implied by this ADR.

## Options Considered

| Option | Result |
|---|---|
| Add a `VpnService` and forward no packets | Rejected: establishes a blackhole TUN and produces a false connected state. |
| Process packets in Flutter/Dart | Rejected: exposes payloads to presentation code and does not fit the foreground-service lifecycle. |
| Use a local SOCKS/CONNECT proxy | Rejected: Android system traffic still requires a TUN-to-proxy engine and the selected direct API deliberately removed CONNECT. |
| Android TUN service plus native packet engine | Selected: matches Android lifecycle and keeps packet handling below the UI while reusing the QUIC flow APIs. |

## Delivery And Gates

1. Add the Android host service, explicit consent bridge, foreground
   notification, and fail-closed lifecycle. It must not establish a routed TUN
   until the packet engine exists.
2. Add a packet-engine crate/boundary with deterministic IPv4 TCP, UDP, DNS,
   checksum, MTU, expiry, and generation-cancellation tests. The first bounded
   IPv4 UDP engine now exists in `velum-adapter-tun`: it owns packet queues,
   datagram associations, response-source validation, and response packet
   encoding. The same crate now exposes generation-bound TCP-to-runtime stream
   relays for a userspace TCP stack and checksum-tested IPv4 TCP address
   translation for transparent stack ingress and egress. Neither path is yet
   wired to an Android file descriptor.
3. Wire the UDP and TCP engine flows to `client-runtime` and add a test relay with authorized
   TCP, UDP, and DNS egress.
4. Retain device evidence for install, permission denial, connect, DNS,
   TCP/UDP transfer, suspend/resume, network switch, relay loss, disconnect,
   and uninstall. A platform is not supported until every case passes.

The blocking validation commands are:

```text
cargo test -p velum-client-runtime -p velum-client-ffi
cargo xtask architecture
cargo xtask docs
flutter test
```

Android device tests become blocking when the host and engine land. No release
may state that it provides a VPN solely because the runtime phase is `Online`.

## Consequences

This is a cross-module delivery: Android host code, a bounded packet engine,
native runtime bindings, and an operated relay exit must be delivered together.
The current Android control application remains experimental until the end to
end device evidence gate passes. The decision is reversible before a default
route is installed; rollback removes the inactive host and packet-engine
integration without altering the direct client wire contract.
