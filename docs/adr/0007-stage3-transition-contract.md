# ADR-0007: Bound Stage 3 Carrier Transitions

- **Status:** Proposed
- **Date:** 2026-07-11
- **Owner:** Protocol maintainers
- **Stakeholders:** Transport, operations, and client implementers

## Context

Stage 3 changes the active carrier between QUIC and TLS without changing a
logical reliable flow. The existing session owns sequence allocation, logical
acknowledgements, and a current plus retiring epoch. Policy must not mutate
that state, and TLS must not be presented as a datagram-equivalent carrier.

## Decision

`velum-policy` returns pure, replayable decisions for cold/warm fallback,
hysteresis, recovery probing, and a transition rate limit. The caller records
only a structured reason and asks `velum-session` to advance an epoch.

After an authenticated attachment on the new carrier, the session reissues
only unacknowledged reliable segments on the current epoch. Their `FlowId` and
sequence remain unchanged, so the receiving delivery cursor suppresses late
or duplicate copies. Acknowledgements stay logical-session evidence.

TLS fallback is stream-only. Datagram flows are rejected at policy/application
placement; no reliable TLS byte stream may be called a datagram path.

Both transition peers negotiate a typed supported-version range before the
attachment is accepted. No shared version rejects the transition with a
redacted `IncompatibleVersion` event. The type is deliberately not a Stage 5
wire grammar.

## Consequences

- Transition decisions and their reasons are deterministic and payload-free.
- At most one retiring epoch remains valid, bounding late-packet state.
- Warm fallback is a policy/resource choice; its actual socket and wakeup
  budgets require retained measurements before it can be enabled by default.
- This does not establish live QUIC/TLS performance or server-restart
  persistence; those remain required Stage 3 evidence.

## Removal Condition

The pre-wire `VersionRange` contract must be replaced by the Stage 5 negotiated
frame grammar, with compatibility vectors and an explicit migration owner.
