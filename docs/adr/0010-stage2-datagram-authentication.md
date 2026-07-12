# ADR-0010: Stage 2 Datagram Authentication

- **Status:** Accepted for the experimental Stage 2 relay
- **Date:** 2026-07-13
- **Owner:** Protocol and server maintainers
- **Related:** ADR-0009 v0 Unreliable Datagram Envelope

## Context

ADR-0009 defined a bounded native QUIC DATAGRAM envelope and the server can
create destination-authorized UDP associations. Its first runtime slice only
accepted datagrams after a reliable control stream authenticated the same QUIC
connection. That made a datagram-only client unable to establish an
association, even though QUIC DATAGRAM has no ordered control prerequisite.

Stage 2 credentials are already bounded to 1 through 128 bytes and the QUIC
carrier encrypts every application datagram. The current relay has no logical
v0 attachment handshake on its experimental control path.

## Decision

Every client-to-server datagram envelope carries a one-octet credential length
and the credential immediately after the direction octet. The credential MUST
be 1 through 128 bytes. Server-to-client envelopes never contain a credential.
The server authenticates this value before destination policy evaluation or
UDP association creation.

The successful principal is bound to the connection-owned `ConnectionAdmission`
session lease. A later reliable stream must authenticate as the same principal,
and a later datagram with a different credential is rejected. Datagram traffic
does not consume the reliable-flow quota; it does consume the connection's
principal session quota. Credentials are not retained by the association,
telemetry, errors, or logs.

## Alternatives

- A datagram authentication handshake: reduces repeated overhead but requires
  nonce generation, acknowledgement, expiry, retry, and replay state.
- Requiring a reliable stream first: keeps envelopes smaller but prevents
  datagram-only clients.
- Transport identity alone: does not authenticate a Velum principal.

## Consequences

- The maximum client-to-server envelope grows by at most 129 bytes.
- Datagram loss can include a lost credential-bearing packet; clients retry
  only according to their application UDP semantics.
- This is experimental Stage 2 behavior, not the v0 logical attachment
  protocol. A future authenticated v0 attachment can supersede this envelope
  credential without weakening the principal-binding invariant.

## Validation And Removal

Codec tests reject empty credentials and preserve canonical round trips.
Admission tests cover datagram-first authentication, shared principal binding,
and conflicting credentials. Socket relay tests cover that association creation
happens only after successful admission. Supersede this ADR when Stage 2 is
replaced by a versioned logical attachment that authenticates QUIC DATAGRAM.
