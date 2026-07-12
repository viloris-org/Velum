# ADR-0011: Stage 2 Local Datagram Adapter

- **Status:** Accepted for the experimental client core
- **Date:** 2026-07-13
- **Owner:** Client and protocol maintainers
- **Related:** ADR-0006, ADR-0009, ADR-0010

## Decision

`velum-client-core` optionally binds `local_datagram_listen` and accepts a
local UDP request of `session_id: u64`, address kind (`0x04` or `0x06`), IP
address, port, and remaining payload. Integers are big-endian. This local
format has no direction or credential field: the socket direction determines
request versus response, and client-core injects the configured credential
into the encrypted QUIC DATAGRAM envelope.

The client binds each nonzero session ID to one local UDP peer and exact target.
It permits at most 256 bindings and expires idle bindings after 60 seconds.
Responses are emitted in the same local format using the source address from
the server envelope. Invalid, over-limit, repurposed, or unbound packets are
dropped without a response.

## Consequences

This is not SOCKS5 UDP ASSOCIATE and is not advertised as SOCKS-compatible.
It gives the desktop controller a bounded datagram transport now while leaving
SOCKS compatibility and its authentication/address semantics for a separate
decision. A local peer must use the documented binary format.
