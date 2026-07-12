# ADR-0011: Wire the Optional Cover Service as a Separate TCP Listener

- **Status:** Proposed
- **Date:** 2026-07-13
- **Owner:** Security maintainers
- **Stakeholders:** Operators, release maintainers, cover-service owners

## Context

`velum-forest` already owns bounded HTTP cover routing and reverse-proxy
behavior, but the node did not expose it to operators. The QUIC relay accepts
UDP and cannot serve the TCP cover-service path. Adding this configuration and
lifecycle wiring spans the application and Forest module boundaries.

## Decision

The node MAY run an opt-in TCP cover listener configured with one exact socket
address, one reverse-proxy upstream, bounded request-head and upstream
timeouts, and a bounded connection count. The upstream is either a literal
socket address or a `hostname:PORT` resolved once while loading configuration;
the selected address is fixed until reload or restart. The listener delegates
every accepted connection to `velum-forest`; it has no access to credentials,
session state, destinations, or delivery acknowledgement state.

The listener accepts already decrypted HTTP/1.1. TLS termination remains an
operator-owned deployment concern, such as a standard TLS terminator in front
of this listener. TCP cover and UDP QUIC listeners may use the same IP:port.
This decision does not introduce a TLS carrier listener, a preface gate, or
TLS fallback attachment behavior.

## Consequences

Positive:

- Operators can run a real reverse-proxied cover application independently of
  Velum session correctness.
- Connection admission, request-head reads, and upstream connection attempts
  are bounded.
- The cover path is disabled by default and can be deployed without changing
  the QUIC wire behavior.

Negative:

- This does not provide same-connection TLS fallback or validate cover-service
  realism under an external TLS terminator.
- Cover listener shutdown is tied to node shutdown; drain semantics remain a
  future operational refinement.

## Review Trigger

Supersede this decision when a TLS listener can safely route a verified Velum
attachment preface and all other post-handshake traffic to the same real cover
service without creating a timing or connection-state oracle.
