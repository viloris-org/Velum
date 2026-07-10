# ADR-0002: Separate Logical Sessions from Carriers

- **Status:** Proposed
- **Date:** 2026-07-11
- **Owner:** Protocol maintainers
- **Stakeholders:** Transport, security, and client implementers

## Context and Evidence

QUIC supports connection migration within QUIC, but Velum's target failure is a
change between unlike transports such as QUIC/UDP and TLS/TCP. Binding logical
flow identity to either connection makes that transition an application-visible
reconnect.

## Decision Drivers

1. Preserve reliable flow correctness through carrier failure.
2. Keep carrier implementations independently replaceable.
3. Prevent transport acknowledgements from becoming ambiguous session truth.
4. Make degraded semantics explicit.

## Options Considered

### Reconnect and let applications recover

Lowest protocol complexity, but does not satisfy the product position.

### One preferred carrier with a byte-level fallback tunnel

Can hide some outages, but couples fallback behavior to one transport and risks
silent semantic changes for datagrams.

### Carrier-independent session and flow state

Adds epochs, logical acknowledgements, replay defense, and transition logic, but
allows carriers to expose honest capabilities behind one contract.

## Decision

Define a logical session above carriers. The session owns flow identifiers,
delivery cursors, epochs, and migration correctness. Carriers own transport I/O,
capabilities, and health measurements only.

Version 1 supports one active placement per flow. Multipath striping is out of
scope.

## Consequences

Positive:

- Carrier failures can be contained below application adapters.
- QUIC and TLS implementations remain independent.
- Datagram degradation cannot be hidden behind a generic byte pipe.

Negative:

- Logical acknowledgement adds overhead and state.
- Transition correctness needs model checking, fuzzing, and fault injection.
- Server memory use grows with resumable unacknowledged data.

## Fitness Functions

- Deterministic model check for duplicate, reordered, and replayed frames.
- 10,000 transition trials with byte-exact application checksums.
- Architecture dependency gate against
  [`architecture-contract.yaml`](../architecture-contract.yaml).

## Review or Invalidation Trigger

Supersede if a simpler carrier-native mechanism meets the continuity targets or
if bounded acknowledgement state cannot be achieved within resource budgets.

## Supersedes

None.

