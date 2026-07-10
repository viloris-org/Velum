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

## Review Evidence

The experimental tracer now assigns every segment and logical acknowledgement
to a `FlowId` and bounded current/retiring `Epoch` window. It rejects stale or
future epochs before changing a delivery cursor, terminates flows rather than
silently dropping timed-out reliable data, and keeps pending segment, byte, and
age state bounded.

`cargo xtask model-check` runs exhaustive short receive traces plus 10,000
seeded transition trials. The seeded campaign covers loss, duplication, delay,
black holes, recovery retransmission, and an epoch transition; each trial
checks byte-exact application output. The campaign's retained aggregate is
`11202198267056387872` for seeds `0..9999`.

Before this ADR can become `Accepted`, the named protocol maintainer must
confirm that the one-epoch retirement window and terminal timeout semantics are
the intended Version 1 contract. The review must also confirm that this
in-process evidence is sufficient to begin, but not replace, Stage 2 carrier
integration evidence.

## Review or Invalidation Trigger

Supersede if a simpler carrier-native mechanism meets the continuity targets or
if bounded acknowledgement state cannot be achieved within resource budgets.

## Supersedes

None.
