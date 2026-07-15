# ADR-0018: Desktop Privileged Traffic Host

- **Status:** Accepted
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Stakeholders:** Security, release, Windows, Linux, and macOS maintainers

## Context

ADR-0016 deliberately deferred desktop TUN because interface creation, route
and DNS mutation, socket exclusion, installation, and crash recovery require
platform-specific privilege. Running the complete client as administrator, a
setuid core, or a loopback HTTP helper would expose credentials and a broad
command surface at elevated privilege.

## Decision

The Flutter controller and ordinary Rust client remain unprivileged. A minimal
platform traffic host owns only TUN lifecycle, transactional route and DNS
changes, socket protection, operating-system handle transfer, and recovery of
its root-owned journal. It cannot receive credentials, certificates, arbitrary
paths, shell commands, process requests, or traffic payloads.

`velum-helper-protocol` defines the common bounded control contract. Frames use
a four-byte big-endian length and at most 64 KiB of strict JSON. Version 1
permits only `hello`, `status`, `start`, `stop`, and `recover`; every request is
correlated by request ID and profile generation and declares capabilities.
Platform transports authenticate and authorize peers before decoding commands.

- Windows uses a signed service and Wintun. Its local named pipe has an
  explicit DACL for the current interactive SID, denies network access, and
  verifies the connecting client identity.
- Linux uses a root system service over system D-Bus. Polkit authorizes the
  requesting subject; TUN descriptors use D-Bus Unix FD passing.
- macOS 13 and later uses a signed Network Extension Packet Tunnel system
  extension with the Rust data plane embedded. Provider messages control its
  lifecycle; no root utun helper is installed.

Route and DNS changes are journaled before mutation and committed as one
logical transaction. On restart, recovery completes before a new `start` is
accepted. Request IDs are idempotent and an older profile generation cannot
replace a newer one. Platform failures return stable categories rather than
raw error strings.

Desktop TUN remains feature-gated until all three hosts pass signed install,
upgrade, crash recovery, stop, and uninstall validation on real systems.

## Consequences

The privileged attack surface is small and independently auditable, while the
shared Rust packet and routing engine remains reusable. Packaging is more
complex and requires Windows service signing, Linux polkit policy review, and
Apple Network Extension entitlements. Host integration cannot be proven by
ordinary cross-platform unit tests.

## Fitness Functions

- Protocol tests reject oversized frames, unknown fields, unknown commands,
  unsupported versions, and parameter injection.
- Platform tests reject unauthorized peers and verify request idempotency,
  generation ordering, partial-mutation rollback, and journal recovery.
- Release evidence covers clean install, upgrade, crash, recovery, stop, and
  uninstall on every supported desktop platform.
- `cargo xtask architecture`, `cargo xtask docs`, and workspace Clippy remain
  required once the crate and platform hosts join the workspace.

## Review Triggers

Revisit this decision when a host needs a new privileged operation, the wire
protocol changes version, a platform deprecates its selected extension or
service API, or desktop TUN becomes eligible to leave its feature gate.
