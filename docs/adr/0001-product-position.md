# ADR-0001: Position Velum Around Adaptive Continuity

- **Status:** Proposed
- **Date:** 2026-07-11
- **Owner:** Project maintainers
- **Stakeholders:** Client users, relay operators, protocol implementers

## Context and Evidence

Encryption, multiplexing, QUIC transport, padding, and HTTP camouflage already
exist across MASQUE, AnyTLS, VLESS/Xray, and Hysteria 2. Implementing the same
feature list in Rust would not establish a reason to adopt another protocol.

The uncovered user problem is operational discontinuity between those choices:
UDP-friendly and TCP-only paths require different protocols, manual switching,
and application reconnects. This is currently a hypothesis documented in the
[evidence ledger](../evidence-ledger.md), not validated demand.

## Decision Drivers

1. Solve a concrete cross-protocol pain rather than duplicate features.
2. Remain useful on both UDP-friendly and TCP-only networks.
3. Produce a falsifiable first prototype.
4. Keep future standards reuse possible.

## Options Considered

### Implement MASQUE in Rust

Strong interoperability story, but it competes on implementation quality and
does not by itself address cross-carrier session continuity.

### Optimize one carrier for maximum throughput

Simpler and likely faster to benchmark, but directly competes with mature QUIC
and TLS proxy implementations and fails outside its chosen carrier conditions.

### Build an adaptive logical session over multiple carriers

Harder state management, but directly addresses manual protocol switching and
creates a focused tracer experiment.

## Decision

Position Velum around preserving logical sessions across unlike carriers, with
intent-aware delivery and Forest Native coexistence as supporting capabilities.

## Consequences

Positive:

- Clear differentiation and measurable failure scenarios.
- QUIC, TLS, and possibly MASQUE can be reused as carriers.
- The architecture separates logical delivery from network connections.

Negative:

- Reliable migration requires acknowledgement, replay, and flow-control state.
- A multi-carrier implementation has larger attack and test surfaces.
- Peak throughput may trail a single-purpose protocol.

## Fitness Functions

- Q-001 and Q-002 in [protocol requirements](../protocol-requirements.md).
- Target-user deployment trials in roadmap Stage 0 and Stage 4.
- Network fault matrix retaining application checksums.

## Review or Invalidation Trigger

Reject or supersede this decision if application sessions cannot survive the
transition reliably, recovery is slower than application reconnect, or target
operators do not value automatic continuity.

## Supersedes

None.

