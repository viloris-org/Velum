# ADR-0012: Flutter Direct Client API

- **Status:** Accepted
- **Date:** 2026-07-13
- **Owner:** Client maintainers
- **Stakeholders:** Transport, protocol, security, and desktop client maintainers
- **Supersedes:** ADR-0006, ADR-0008, and ADR-0011 for the client data plane

## Context

The Stage 2 desktop client currently starts `velum-client-core`, accepts a
local HTTP CONNECT socket, copies TCP bytes into QUIC, and repeats the inverse
mapping on the server. The local proxy is an additional data-plane boundary;
the direct loopback comparison already shows that each TCP-to-QUIC mapping has
material throughput and latency cost. Flutter has no QUIC implementation or
existing native binding. Replacing CONNECT with local IPC would retain a
separate data-plane copy and does not meet the performance objective.

## Decision Drivers

1. Eliminate the local TCP proxy from the application data path.
2. Keep session, carrier, credential, and certificate ownership in Rust.
3. Preserve a small, versioned, testable API for Flutter without exposing
   pointers, credentials, destinations, or payloads to logs.
4. Keep `unsafe` restricted to one reviewed native boundary.
5. Do not retain CONNECT as a compatibility protocol.

## Decision

Create two crates:

- `velum-client-api` is the safe Rust client engine. It owns QUIC connection
  lifecycle, authentication material, stream lifecycle, backpressure, and
  cancellation. Its public API exposes opaque sessions and streams with
  bounded `open_stream`, `write`, `read`, `finish`, and `close` operations.
- `velum-client-ffi` is the only native binding boundary. It exposes
  versioned opaque handles and copy-in/copy-out byte operations for Flutter.
  It retains no caller pointer after a call returns, serializes no secrets or
  payloads into diagnostics, and maps every failure to a stable public error
  code.

Flutter calls the native API directly. The client engine and Flutter run in
the same desktop process. The HTTP CONNECT listener, CONNECT parser, local
listen configuration, and `velum-adapter-connect` dependency are removed;
applications must use the direct API. The existing Stage 2 remote control
record remains explicitly experimental until the v0 session I/O path replaces
it. This decision does not claim v0 wire interoperability.

## Ownership And Invariants

| Concern | Owner | Invariant |
|---|---|---|
| Dart handle lifecycle | Flutter | A released handle is never reused. |
| Native handle table and calls | `velum-client-ffi` | Invalid or stale handles fail locally without dereference. |
| QUIC, credentials, certificates, flow I/O | `velum-client-api` | No credential or payload crosses into logs or error strings. |
| Flow identity and delivery semantics | `velum-session` when v0 I/O is attached | Only the session may advance a logical delivery cursor. |
| Destination authorization | Server | The client cannot bypass the server allowlist or quota. |

Every read has an explicit caller-provided capacity. Every write is bounded by
the negotiated stream budget and returns backpressure rather than queueing
unbounded application bytes. Cancellation closes the local stream and does
not claim remote delivery.

## Options Considered

| Option | Result |
|---|---|
| Retain CONNECT and tune buffers | Rejected: preserves the avoidable local TCP boundary. |
| Replace CONNECT with local RPC | Rejected: changes syntax but retains a separate data-plane copy. |
| Implement QUIC independently in Dart | Rejected: duplicates critical transport and credential logic without existing evidence or a maintained dependency. |
| Reviewed Rust native binding | Selected: removes the local proxy while retaining one owned transport implementation. |

## Consequences

- This is a breaking client API change. There is no CONNECT compatibility
  listener or configuration shim.
- `velum-client-ffi` requires a focused unsafe-code review, ABI tests, and
  supported-platform checks before release.
- Flutter must handle explicit stream backpressure and lifecycle errors.
- The safe client API can be used by non-Flutter Rust applications without
  the FFI crate.

## Fitness Functions And Delivery

1. `cargo xtask architecture` blocks dependencies outside the declared API
   and FFI boundaries.
2. API contract tests cover stale handles, bounded reads/writes, cancellation,
   and redacted failures.
3. Flutter integration tests open a stream, transfer bytes, apply
   backpressure, and release all handles.
4. A retained direct-API versus CONNECT baseline demonstrates the removal of
   the local proxy cost before the old path is deleted from a release.

The first vertical slice supports one authenticated reliable stream. It may
ship only after the contract and Flutter integration tests pass. Rollback is
to the previous released client binary, not an in-process CONNECT fallback.

## Review Trigger

Supersede this decision if a maintained Flutter QUIC implementation can meet
the same protocol, security, and performance gates without a native binding,
or if the FFI review finds that a safe bounded-handle ABI cannot be maintained.
