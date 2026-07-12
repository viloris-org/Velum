# ADR-0009: v0 Unreliable Datagram Envelope

- **Status:** Accepted for the protocol codec slice
- **Date:** 2026-07-13
- **Owner:** Protocol maintainers
- **Stakeholders:** Transport, server, session, and client implementers

## Context

QUIC exposes native unreliable datagrams, but the v0 draft previously defined
only reliable carrier frames. The existing destination policy authorizes exact
IP socket addresses, and TLS fallback must not claim equivalent datagram
delivery. A UDP application relay needs a bounded association identity and an
unambiguous address representation before it can own sockets.

## Decision

Define a carrier-independent, encrypted datagram envelope in
`velum-protocol`. It has a direction octet, a non-zero `u64` session ID, an
exact IPv4 or IPv6 socket address, and the remaining application payload.
Integers are big-endian. Hostnames, port-zero special handling, protocol-level
fragmentation, retries, ordering, acknowledgements, and retransmission are out
of scope.

The codec accepts at most 60 KiB of application payload and 60 KiB plus the
largest envelope header overall. The QUIC carrier applies the stricter
negotiated path MTU and returns `DatagramTooLarge`; applications may drop or
apply their own retry policy. A receiver must reject malformed, unknown,
zero-session, and over-limit envelopes without creating a UDP socket.

## Alternatives

- Reuse reliable `Frame`: violates datagram ordering and retransmission
  semantics.
- Copy Hysteria's UDP framing: imports an unrelated compatibility surface.
- Fragment in v0: requires reassembly memory, expiry, abuse limits, and loss
  semantics that have not been validated.

## Consequences

- `velum-protocol` owns only codec and canonical vectors.
- The future server UDP association manager owns destination authorization,
  sockets, idle cleanup, and per-principal limits.
- The future client UDP adapter owns local socket association and bounded
  queues.
- Datagram sessions terminate on a carrier transition; they are never replayed
  over TLS.

## Validation And Removal

The codec has round-trip and malformed-input tests. Before enabling runtime
UDP relay, add socket-level loopback, overload, idle-expiry, MTU, loss, and
carrier-transition tests with retained netem evidence. Supersede this ADR if a
versioned protocol review finds that application fragmentation is required.
