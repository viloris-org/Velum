# ADR-0014: Client Trust Modes And Runtime ABI v2

- **Status:** Proposed
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Related:** ADR-0013

## Context

Requiring every relay node to name a local CA file makes ordinary connections
needlessly difficult. The client must instead use the platform trust store by
default while preserving an explicit custom-CA path for private relays.

An insecure mode is also needed for development and exceptional deployments.
It removes certificate and server-name verification and is therefore vulnerable
to relay impersonation and traffic interception.

## Decision

`velum-client-api` owns three trust modes: system trust store, explicit custom
CA roots, and an insecure verifier. System trust is the default. Custom CA is
only required for that selected mode. The FFI configuration receives a trust
mode field, which changes its fixed C layout; both the synchronous and runtime
ABI versions advance to v2. Flutter rejects v1 libraries rather than reading a
configuration with an incompatible layout.

The UI requires a one-time, per-process risk acknowledgement before enabling
insecure mode. Its warning remains visible for three seconds, after which the
user must explicitly confirm understanding. The acknowledgement is not written
to disk and is cleared when the application exits.

## Consequences

- Publicly trusted relays require no CA file selection.
- Private relays retain explicit CA pinning through the custom-CA mode.
- Insecure mode is an explicit exception with a visible risk record in the UI,
  never the default or fallback after certificate failure.
- Every ABI consumer must rebuild against v2; there is no compatibility shim
  because a v1 caller cannot safely populate the added field.

## Validation

Run `cargo test -p velum-client-api -p velum-client-ffi`, `flutter test`, and
the architecture and documentation gates before release.
