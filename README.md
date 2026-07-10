# Velum

Velum is a research-stage encrypted tunneling protocol for restricted,
unstable, and heterogeneous networks.

Its intended differentiator is session continuity across multiple carriers:
the same logical session can adapt between QUIC/UDP and TLS/TCP without making
applications choose a protocol up front. Velum also treats camouflage as
native coexistence with real Internet services, not as a packet-obfuscation
toggle.

> Project status: positioning and architecture discovery. No wire protocol or
> security claim is stable yet.

## Design Direction

- Preserve logical flows while network paths and carriers change.
- Give streams, messages, and datagrams distinct delivery semantics.
- Use standard cryptographic transports; do not invent cryptography.
- Make unauthenticated endpoints behave as real services.
- Measure performance, degradation, and detectability claims.
- Keep the Rust implementation split by responsibility and protocol layer.

Start with the [documentation index](docs/README.md) and the
[roadmap](docs/roadmap.md).

## Current Non-Goals

- Claiming to be undetectable or unblockable.
- Designing a new cipher suite or TLS replacement.
- Replacing MASQUE, WireGuard, or every application proxy.
- Shipping multi-hop anonymity in the first protocol version.
- Freezing a wire format before the tracer experiments succeed.

