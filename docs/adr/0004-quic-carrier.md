# ADR-0004: QUIC Carrier Library

- **Status:** Accepted for Stage 2 slice
- **Date:** 2026-07-11
- **Owner:** Transport maintainers
- **Stakeholders:** Protocol, security, server, and client implementers

## Context

Stage 2 needs a maintained QUIC and TLS 1.3 implementation while preserving
the carrier-independent session boundary established by ADR-0002. The project
requires an implementation compatible with Rust 1.97 and a Tokio-based server
slice without introducing a custom cryptographic transport.

## Options Considered

### Quinn

Rust-native QUIC with a maintained Tokio runtime integration and a rustls
backend. It exposes bidirectional streams and the peer datagram payload limit
needed by the Stage 2 contract.

### s2n-quic

A credible Rust QUIC implementation, but its TLS and runtime integration would
be a separate decision for the first Tokio and rustls-oriented slice. It stays
an alternative if the selected dependency no longer meets the resource or
interoperability gates.

### quiche

Not selected for this slice because its integration and TLS configuration would
need additional adapter work before the project can validate the narrow carrier
boundary. It remains an alternative for a later implementation review.

## Decision

Use `quinn` version `0.11.11`, pinned exactly for the first Stage 2 slice.
Enable only `runtime-tokio` and `rustls-ring`; do not enable platform verifier,
logging, qlog, or multiple cryptographic backends by default. The
`velum-carrier-quic` crate implements the narrow `velum-carrier-api` contract.

The carrier maps QUIC bidirectional streams to reliable carrier streams and
QUIC datagrams to explicit unreliable operations. Before an application sends
a datagram, the carrier exposes the peer's current maximum payload. Oversize
datagrams fail locally with `DatagramTooLarge`; they are never reclassified as
reliable stream bytes.

## Threat Review

- QUIC/TLS implementation and AEAD selection are delegated to Quinn, rustls,
  and ring; Velum does not define a new cipher suite or handshake.
- Carrier encryption does not authenticate a Velum logical session. Stage 2
  server work must bind an authenticated principal before allowing targets.
- QUIC stream completion is transport evidence only. Logical acknowledgement,
  replay windows, and epochs remain session-owned.
- QUIC configuration must set bounded connection, stream, datagram, and idle
  limits before listener exposure. The carrier crate supplies no permissive
  listener defaults.
- Error values intentionally contain no peer address, destination, payload,
  credential, or TLS diagnostic text.

## Version Policy

`quinn` is exact-pinned during Stage 2. Upgrades require a dependency review,
an integration run against the supported Rust version, and a retained direct
QUIC comparison result. A security update may be expedited, but must retain
those checks before release.

## Invalidation Trigger

Revisit this decision if Quinn loses maintained TLS 1.3 support, cannot expose
the required stream or datagram limits, introduces an unresolved critical
advisory, or prevents the Stage 2 resource and interoperability gates from
passing.
