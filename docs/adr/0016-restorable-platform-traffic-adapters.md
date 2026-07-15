# ADR-0016: Restorable Platform Traffic Adapters

- **Status:** Accepted
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Stakeholders:** Android, release, security, and transport maintainers
- **Supersedes:** ADR-0014 and ADR-0015

## Context And Evidence

ADR-0014 supplied Android packet and flow primitives but did not install a TUN
interface. Its `VpnService` returned immediately without forwarding packets.
ADR-0015 installed desktop proxy settings, but the loopback proxy rejected
domain targets and disabling the feature discarded the user's prior proxy
configuration. A process crash could therefore leave a dead proxy configured.

FlClash demonstrates the required platform split. Android owns consent, a
foreground `VpnService`, the raw TUN descriptor, routes, DNS, and application
exclusion. Windows system proxy changes require a WinINet settings-change and
refresh notification. macOS proxy settings belong to individual network
services. Linux has separate GNOME/MATE GSettings and KDE KIO backends.

The maintained `ipstack` 1.0.1 crate accepts an asynchronous raw TUN stream and
provides userspace TCP and UDP flow abstractions. It is Rust 2024 and
Apache-2.0 licensed. Velum pins the version rather than implementing a TCP
state machine in application code. The broader `tun2proxy` package was rejected
because its transitive license and unmaintained-crate set failed `cargo deny`.

## Decision Drivers

1. Traffic capture must have a real forwarding path and fail closed on loss.
2. User-owned OS settings must survive stop, failure, and the next app launch.
3. Packet bytes remain below Flutter and platform channels.
4. Platform privilege and lifecycle differences remain explicit.
5. The implementation must be replaceable and continuously testable.

## Decision

### Desktop system proxy

The host captures the current platform configuration and atomically persists a
backup before changing any setting. Enable restores a backup left by an earlier
crash before capturing a new one. Disable removes the backup only after every
original value has been restored.

- Windows writes per-user Internet Settings and calls
  `InternetSetOptionW(SETTINGS_CHANGED)` followed by `REFRESH`.
- macOS captures and restores secure-web, SOCKS, and bypass settings for every
  enabled network service through `networksetup`.
- Linux selects GNOME, MATE, or KDE and restores the backend's exact values.

The loopback proxy accepts HTTP CONNECT and SOCKS5 domain targets. It currently
resolves those domains through the host resolver and sends only the resolved
`SocketAddr` through the existing exact-target relay contract. This makes
ordinary desktop applications work but does not provide encrypted desktop DNS.
A future remote-name target contract must replace this compatibility behavior
before Velum claims desktop DNS-leak prevention.

### Android TUN

After explicit user consent and an online Velum runtime, `VpnService` creates an
IPv4 TUN interface, installs the default route and DNS resolver, excludes
Velum's own package to prevent a relay routing loop, and returns the raw
descriptor. No packet bytes cross the method channel.

The `velum-adapter-tun` Android target owns the pinned packet engine. It runs on
a background isolate, converts TUN TCP flows to generation-bound runtime
streams, and maps UDP flows (including DNS) to bounded runtime datagram session
ids with response-source validation. TCP and UDP each have 256 concurrent flow
slots. Stop order is packet engine, Android route/interface, then runtime.

The current Android release slice covers IPv4 TCP, UDP, and DNS. The UI and
release notes must not describe this slice as IPv6 VPN support.

### Traffic mode control

The platform UI exposes one traffic-routing mode selected from the adapters
available on that platform. A dedicated controller owns desired mode, active
mode, installation progress, and payload-free failure state. Desired mode may
be selected while offline; it activates only after the authoritative runtime
becomes online. Runtime loss removes OS integration while retaining the mode
for the next connection. Explicit disconnect waits for an active adapter to be
removed before stopping the encrypted runtime.

The settings page renders this controller state and does not infer successful
routing from the runtime's `Online` phase.

Desktop TUN is a separate delivery stage. Linux capabilities/polkit, macOS
Network Extension or privileged helper installation, and Windows Wintun/helper
service have different installation and rollback contracts and must not be
hidden behind Android's consent API.

## Runtime And Failure Invariants

- A system setting never points at a listener that failed to bind.
- A failed desktop mutation attempts immediate restoration and retains the
  backup if restoration itself fails.
- Android establishes the default route only after runtime readiness; native
  engine failure closes the TUN service.
- The Velum Android package is excluded from its own VPN route.
- Stop or runtime generation loss removes the TUN path before the encrypted
  runtime disappears.
- Flutter receives only lifecycle results and an integer descriptor; it never
  receives packet content or destinations.

## Consequences And Risks

The pinned engine upgrades the workspace Tokio pin from 1.48.0 to 1.52.3.
Foundation tests are the rollback gate. The Android binary must retain the
engine's Apache-2.0 license notice. Android device tests remain required because
host tests cannot prove OEM VPN lifecycle, notification, suspend/resume, or
network-switch behavior.

Host DNS resolution for desktop proxy domain targets is an explicit temporary
security limitation. IPv6 applications remain outside the installed route in
this stage.

## Fitness Functions

- `cargo test -p velum-adapter-proxy -p velum-client-ffi`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo xtask architecture`
- `cargo xtask docs`
- `flutter analyze`
- `flutter test`
- Android debug build and retained device evidence for permission denial,
  TCP browsing, DNS resolution, relay loss, disconnect, process death,
  suspend/resume, and network switch.

## Review Triggers

Revisit this decision when the relay accepts authorized domain targets, an
upstream `ipstack` security advisory affects the pinned version, IPv6 is added,
or the first desktop TUN helper is proposed.
