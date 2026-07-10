# ADR-0007: Stage 2 Runtime Composition

- **Status:** Accepted for Stage 2 slice
- **Date:** 2026-07-11
- **Owner:** Server and release maintainers

## Context

ADR-0005 keeps authentication, exact destination authorization, and quota
accounting in `velum-server` without sockets or a wire format. The first QUIC
listener must consume those decisions, but the architecture contract did not
allow the application composition root to depend on `server`.

## Decision

Allow `applications -> server`. `velum-node` owns listener lifecycle,
configuration validation, local socket relay, and the experimental Stage 2
control record. `velum-server` remains a pure admission module and gains no
carrier, socket, or framing dependency.

## Consequences

- The network boundary has one composition root, while admission remains
  deterministic and separately testable.
- The Stage 2 control record is explicitly application-owned and cannot become
  a protocol dependency by accident.
- Future listeners use the same pure admission surface or add a new ADR for a
  different authority boundary.

## Invalidation Trigger

Supersede this decision when a stable protocol listener moves into a dedicated
runtime crate or deployment evidence requires a separate server process.
