# ADR-0006: Stage 2 Experimental CONNECT Adapter

- **Status:** Accepted for Stage 2 slice
- **Date:** 2026-07-11
- **Owner:** Client maintainers

## Evidence

- Stage 2 needs one local adapter to validate a reliable QUIC byte flow.
- The session model currently validates reliable streams, while datagram and
  TUN semantics remain separate Stage 2 work.
- The server admission boundary authorizes exact socket addresses only.

## Decision

Use a minimal HTTP CONNECT adapter for the experimental slice. It accepts only
`CONNECT <IP:port> HTTP/1.1`, retains bytes read after the header, and emits a
standard successful CONNECT response after the future server approves the
target. It rejects hostnames, malformed requests, other HTTP methods, and
headers larger than 8 KiB.

The adapter owns only local HTTP parsing. The session owns flow identity, the
server owns destination authorization, and the carrier owns QUIC I/O.

## Alternatives

- SOCKS5: requires a separate UDP-associate decision and protocol surface.
- TUN: requires OS privileges and routes traffic beyond the initial reliable
  stream proof.

## Invalidation Trigger

Supersede if retained operator evidence favors SOCKS5 or TUN, or if hostname
support is needed with a verified server-side resolution policy.
