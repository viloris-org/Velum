# ADR-0015: Desktop Loopback Proxy And System Configuration

- **Status:** Proposed
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Related:** ADR-0012, ADR-0013, ADR-0014

## Context

The Flutter client previously connected a relay but did not install a local
traffic adapter or modify an operating system proxy setting. FlClash provides
the relevant platform split: a loopback proxy is a separate data-plane service;
Windows uses the per-user Internet Settings API, macOS changes each active
network service through `networksetup`, Linux uses the active desktop's proxy
settings, and Android routes through `VpnService` rather than a desktop proxy.

Velum's Stage 2 relay control record accepts only an exact `SocketAddr` target.
Resolving HTTP CONNECT or SOCKS domain names locally would expose DNS outside
the relay and would no longer preserve the current destination contract.

## Decision

Add `velum-adapter-proxy`, a bounded loopback-only listener that opens
generation-bound `ClientRuntime` streams. It supports HTTP CONNECT and SOCKS5
CONNECT only for literal IPv4 or IPv6 destination addresses. It binds
`127.0.0.1` only and starts only after the runtime is online. Runtime stop,
destroy, or proxy stop tears down the listener before invalidating its flows.

The Flutter desktop host changes system proxy settings only after the native
listener reports its bound port. It removes the system setting before closing
the listener. The first implementations are:

| Platform | Host action | Scope |
|---|---|---|
| Windows | Per-user `Internet Settings` registry values plus shell refresh | HTTPS/SOCKS consumers |
| macOS | `networksetup` for every enabled network service | Secure-web and SOCKS proxies |
| Linux GNOME-compatible desktops | `gsettings org.gnome.system.proxy` | HTTPS/SOCKS consumers |
| Android | No desktop proxy setting | TUN remains gated by ADR-0014 |

Unsupported Linux desktop environments fail without changing any setting.
The host does not establish a TUN device because the current adapter does not
yet implement a TCP packet engine. Establishing a default route before that
engine exists would blackhole traffic.

## Invariants

- No system proxy setting points to a listener that was not successfully bound.
- A listener is reachable only from local IPv4 loopback.
- Domain names are rejected, not resolved locally.
- Stopping a runtime removes its proxy listener before the authenticated client
  is closed.
- A failed system-setting update attempts to disable the setting and closes the
  listener. Existing user proxy configuration is not yet restored.
- Android does not call `VpnService.Builder.establish()` until TCP, UDP, DNS,
  generation cancellation, and relay socket protection are implemented and
  device-tested.

## Consequences

This is a real desktop system-proxy path for IP-literal HTTPS CONNECT and SOCKS
traffic, not a claim of universal web browsing or VPN support. The host does
not configure an HTTP proxy because this listener rejects plain HTTP. Supporting
proxy domain names or plain HTTP requires a versioned relay/server target
contract with DNS policy and leakage tests. A real Android TUN requires the
remaining ADR-0014 packet-engine and device-evidence gates; it cannot be
supplied by a UI switch.

## Validation

```text
cargo test -p velum-adapter-proxy -p velum-client-ffi
cargo xtask architecture
cargo xtask docs
flutter test
```
