# ADR-0008: Desktop Client Process Boundary

- **Status:** Accepted for the experimental Stage 2 slice
- **Date:** 2026-07-13
- **Owner:** Client maintainers

## Context

The desktop client needs a cross-platform configuration UI and a local CONNECT
adapter for the existing experimental QUIC relay. The Rust workspace forbids
`unsafe`, while a direct Dart FFI surface would require an unsafe native ABI
boundary and a lifecycle contract that has not yet been reviewed.

## Decision

The Flutter application starts a separately packaged `velum-client-core`
process with a private configuration file and consumes only its payload-free
line-oriented lifecycle output. The Rust process owns local sockets, QUIC,
certificate verification, credential loading, and CONNECT parsing. Flutter
owns form validation, configuration-file selection, process lifecycle, and
presentation.

The core speaks only the application-owned Stage 2 control record already
accepted by `velum-node`. It is not a v0 protocol implementation and must not
be represented as an interoperable or production VPN client.

## Consequences

- The UI never receives a credential value from the core and does not log
  destinations or payloads.
- Core and UI can be released independently behind a documented command-line
  and status-output contract.
- A future reviewed FFI boundary may supersede this process boundary once
  native ABI ownership, cancellation, and secret-memory handling are defined.

## Removal Condition

Replace the Stage 2 control record and line protocol when the versioned client
session API is implemented and covered by protocol conformance vectors.
