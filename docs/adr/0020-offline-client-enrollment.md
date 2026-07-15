# ADR-0020: Offline Client Enrollment

- **Status:** Proposed
- **Date:** 2026-07-15
- **Owner:** Client and release maintainers
- **Stakeholders:** Security, node operations, client platform, and protocol maintainers
- **Related:** ADR-0005, ADR-0013, ADR-0014, and ADR-0019

## Context And Evidence

The Stage 2 relay authenticates every reliable flow and native datagram with a
configured shared secret. Guided node setup creates one 32-byte credential,
while client profiles deliberately persist only `secret://velum/` references.
Operators therefore lacked a versioned way to transfer relay coordinates,
trust material, and a per-device credential into platform secure storage.

Mobile clients need camera-based enrollment. Desktop clients cannot assume a
camera and need a file workflow. The installer and command line must not accept
secret bytes as arguments because process listings and shell history are not
secret stores.

## Decision Drivers

1. Long-lived credential bytes are exposed only during an explicit enrollment.
2. Each device receives an independently revocable principal credential.
3. Mobile QR and desktop file delivery share one validated contract.
4. Enrollment does not add a remotely reachable management service.
5. Schema ownership and compatibility remain machine enforceable.

## Options Considered

| Option | Decision |
|---|---|
| Put credentials into normal client profiles | Rejected: profiles are durable and intentionally redacted |
| Add an online pairing endpoint and short code | Deferred: expiry state, abuse controls, and a new public attack surface are not justified for the first slice |
| Versioned offline bundle delivered as QR or owner-only file | Selected: one bounded contract and no new network service |

## Decision

`velum-client-profile` owns canonical enrollment JSON v1 in addition to the
redacted profile schema. The enrollment contains a fixed kind and version,
client-reachable relay socket, TLS server name, principal ID, exactly 32 bytes
of hexadecimal credential material, and either system trust or a bounded PEM
CA certificate.

The node application owns issuance and revocation orchestration. `velum client
issue` generates the credential internally, writes its secret file with owner
permissions, atomically updates the node configuration, and then renders the
same canonical JSON as a terminal QR code or a new `.velum-enroll` file.
`velum client revoke` removes the principal before deleting its secret file and
refuses to remove the final configured credential. The Stage 2 process keeps
admission state in memory, so issuance and revocation require a relay restart or
redeploy before taking effect.

The native client FFI validates and canonicalizes enrollment input before the
Flutter host projects it. Flutter writes credential and optional CA bytes into
platform secure storage, creates only `secret://velum/` references, clears
mutable byte buffers, and attempts to remove an imported source file. The
current Android host exposes QR scanning; every shipped host exposes native
file selection.

## Invariants

- Normal profiles never contain credential or certificate bytes.
- Enrollment credential material is exactly 32 random bytes.
- Relay addresses in enrollment cannot be unspecified listener addresses.
- Unknown fields, oversized input, and unsupported versions fail closed.
- CLI output files use create-new semantics and owner-only permissions.
- Issuance rollback removes the new credential if configuration or delivery
  activation fails.
- One client credential can be revoked without changing other clients.
- CLI output explicitly reports the restart required to activate credential changes.
- Enrollment bytes and credentials are excluded from logs and error text.

## Consequences

Offline files contain a long-lived credential until successfully imported and
removed. Copies made by file synchronization, backups, screenshots, or terminal
recording cannot be remotely invalidated except by revoking that principal.
QR capacity can also reject large custom certificate chains; file delivery is
the supported fallback.

The selected slice avoids an online administration boundary but cannot provide
single-use or time-limited transfer semantics. Those properties require a new
ADR covering a PAKE or high-entropy token protocol, expiry ownership, rate
limits, and service exposure.

## Fitness Functions

1. `cargo test -p velum-client-profile -p velum-client-ffi -p velum-node`
   covers schema bounds, native validation, QR generation, CLI parsing, and
   credential rejection.
2. `cargo xtask architecture` enforces applications and FFI depending inward
   on the client-profile schema owner.
3. `cargo xtask client-test` runs Flutter analysis, enrollment projection
   tests, widget tests, and native ABI integration.
4. `cargo xtask docs`, `cargo fmt --all --check`, and workspace Clippy remain
   blocking.

## Delivery And Rollback

This first slice is additive: existing credential files and profile import
remain valid. Rollback removes enrollment commands, ABI symbols, and UI entry
points without changing the relay authentication record. Enrollment v1 remains
supported while any released client consumes it; incompatible evolution uses a
new version rather than changing v1 interpretation.

## Review And Invalidation Triggers

Review this decision when operators require remote enrollment, transfer expiry,
fleet-scale issuance, or auditable invitation consumption. Review QR payload
limits if retained custom-CA enrollment evidence shows routine scan failures.
