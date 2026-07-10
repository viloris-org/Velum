# ADR-0005: Stage 2 Server Admission Boundary

- **Status:** Accepted for Stage 2 slice
- **Date:** 2026-07-11
- **Owner:** Server maintainers

## Evidence

- The architecture assigns authentication, destination authorization, and
  quotas to the server, not the carrier or session.
- No local adapter has completed the Stage 0 operator-evidence decision.
- The protocol has no stable wire grammar, so a listener must not freeze one
  accidentally in a shared crate.

## Decision

Create `velum-server` as a pure admission boundary. It provides
constant-time comparison of configured non-empty shared secrets, exact socket
address allowlists that deny by default, and in-memory per-principal session
and flow quotas. It has no sockets, carrier dependency, payload logging, or
wire encoding.

The first listener and selected local adapter will compose this module after
the Stage 0 adapter decision. Their experimental framing remains application
owned and must not enter `velum-protocol` before Stage 5 validation.

## Consequences

- Security-sensitive admission behavior has deterministic unit tests before
  network integration.
- Exact addresses are deliberately restrictive; hostname resolution, CIDR
  policy, persistent quotas, and credential rotation are deferred.
- Process-local quotas reset on restart. A deployment cannot claim durable
  rate limiting from this slice.

## Invalidation Trigger

Supersede this decision when an authentication backend, hostname/CIDR policy,
or multi-process quota store is required by retained deployment evidence.
