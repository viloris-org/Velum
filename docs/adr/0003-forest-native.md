# ADR-0003: Make Forest Native a System Invariant

- **Status:** Proposed
- **Date:** 2026-07-11
- **Owner:** Security maintainers
- **Stakeholders:** Operators, protocol implementers, cover-service owners

## Context and Evidence

Fixed protocol mimicry can still expose unique endpoint behavior, timing,
packet distributions, and deployment characteristics. AnyTLS explicitly
documents limitations beyond packet length, while Hysteria 2 requires HTTP/3
server behavior and optionally adds packet obfuscation. Neither fact supports a
claim that any system is indistinguishable.

## Decision Drivers

1. Avoid false security claims.
2. Resist simple passive fingerprints and active-probe oracles.
3. Keep camouflage changes from breaking delivery correctness.
4. Allow inexpensive profile evolution.

## Options Considered

### No camouflage

Operationally honest and simple, but does not serve restricted-network use
cases where dedicated tunnel endpoints are cheaply classified.

### Optional packet obfuscation layer

Easy to toggle, but encourages a false boolean security model and usually
ignores endpoint and deployment behavior.

### Native coexistence with a real service

Broadens operational scope, but provides a coherent threat model and removes
fixed pre-authentication behavior from the tunnel protocol.

## Decision

Treat real service coexistence, absence of pre-authentication markers, and
independent profile evolution as system invariants. Forest profiles cannot own
authentication or delivery semantics.

Do not claim undetectability. Report probe results and deployment assumptions.

## Consequences

Positive:

- Enabled and disabled endpoints can be compared directly.
- Cover behavior remains useful when Velum is unavailable.
- Traffic-profile work is isolated from the session state machine.

Negative:

- Operators must run and monitor a real service.
- Realistic distributions may add latency, bandwidth, and cost.
- A low-population endpoint can remain statistically distinctive.

## Fitness Functions

- Differential pre-authentication probe suite.
- Profile-specific latency, bandwidth, CPU, and cover-byte budgets.
- Architecture rule forbidding `forest` from depending on session or crypto.

## Review or Invalidation Trigger

Supersede if coexistence creates unacceptable security coupling, if probes find
a structural oracle that requires session-layer changes, or if deployment
trials show the operational requirement defeats adoption.

## Supersedes

None.

