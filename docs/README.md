# Velum Documentation

This directory is the design source of truth while Velum is in discovery.
Statements are divided into verified facts, hypotheses, decisions, and open
questions. Proposed ADRs are not final protocol commitments.

## Product and Evidence

- [Vision and positioning](vision.md): target users, problem statement,
  differentiators, goals, and non-goals.
- [Protocol landscape](landscape.md): MASQUE, AnyTLS, VLESS, Hysteria 2, and
  WireGuard comparison.
- [Evidence ledger](evidence-ledger.md): facts, assumptions, unknowns, and
  invalidation signals.
- [Validation evidence](../validation/README.md): versioned network and
  workload manifests, interview records, baseline pins, and raw evidence rules.

## Protocol Design

- [Architecture](architecture.md): system boundaries, protocol layers, runtime
  behavior, failure handling, and ownership.
- [Client architecture evolution](client-architecture.md): target runtime,
  platform-host, UI, traffic-adapter, and migration boundaries.
- [ACME operations](acme.md): external Lego DNS-01 issuance, renewal, and
  certificate activation.
- [`velum` research CLI](velum-node.md): guided relay setup, deployment, and
  local operator controls.
- [Forest Native](forest-native.md): camouflage philosophy, threat model, and
  deployment requirements.
- [TLS fallback evolution](tls-fallback-evolution.md): deferred shaping and
  inner-multiplexing direction, limits, and evidence gates.
- [Protocol requirements](protocol-requirements.md): delivery semantics,
  invariants, quality scenarios, and fitness functions.
- [Protocol v0 draft](protocol-v0.md): bounded frame grammar, negotiation,
  carrier attachment, reliable-stream frames, and explicit recovery limits.
- [Stage 1 session tracer](session-tracer.md): deterministic reliable-flow
  state model and transition table.
- [Architecture contract](architecture-contract.yaml): initial machine-readable
  module and dependency boundaries, enforced by `cargo xtask architecture`.

## Delivery and Decisions

- [Roadmap](roadmap.md): evidence-driven delivery stages and exit gates.
- [Contribution policy](../CONTRIBUTING.md), [security policy](../SECURITY.md),
  [support policy](../SUPPORT.md), and [release policy](../RELEASES.md).
- [ADR-0001: Product position](adr/0001-product-position.md)
- [ADR-0002: Multi-carrier sessions](adr/0002-multi-carrier-sessions.md)
- [ADR-0003: Forest Native](adr/0003-forest-native.md)
- [ADR-0004: QUIC carrier library](adr/0004-quic-carrier.md)
- [ADR-0005: Stage 2 server admission](adr/0005-stage2-server-admission.md)
- [ADR-0006: Stage 2 CONNECT adapter](adr/0006-stage2-connect-adapter.md)
- [ADR-0007: Stage 2 runtime composition](adr/0007-stage2-runtime-composition.md)
- [ADR-0012: Flutter direct client API](adr/0012-flutter-direct-client-api.md)
- [ADR-0013: Client runtime and platform host boundary](adr/0013-client-runtime-boundary.md)
- [ADR-0014: Android TUN data plane](adr/0014-android-tun-data-plane.md)
- [ADR-0015: Desktop system proxy](adr/0015-desktop-system-proxy.md)
- [ADR-0016: Restorable platform traffic adapters](adr/0016-restorable-platform-traffic-adapters.md)
- [ADR-0017: Local traffic routing policy](adr/0017-local-traffic-routing-policy.md)
- [ADR-0018: Desktop privileged traffic host](adr/0018-desktop-privileged-traffic-host.md)
- [ADR-0019: Profile and routing evolution](adr/0019-profile-and-routing-evolution.md)
- [ADR-0020: Offline client enrollment](adr/0020-offline-client-enrollment.md)
- [ADR-0021: Native client capability and UI evolution](adr/0021-native-client-capability-and-ui-evolution.md)
- [Privileged helper protocol v1](helper-protocol-v1.md)
- [ADR-0011: Cover-service listener wiring](adr/0011-cover-service-listener-wiring.md)

## Document Status

| Artifact | Status | Becomes stable when |
|---|---|---|
| Vision | Working baseline | Target-user interviews validate the problem |
| Architecture | Proposed | Tracer prototype validates session migration |
| Wire protocol | v0 draft | Vectors, a second consumer, fuzzing, and security review converge |
| Security model | Proposed | Independent review closes critical findings |
| Roadmap | Active | Updated at every stage gate |

## Repository Checks

- `cargo xtask architecture` validates runtime ownership and dependency rules.
- `cargo xtask docs` validates repository-local Markdown links.
- `cargo xtask test` runs every current Foundation gate.
- `cargo xtask client-test` builds the native client and runs Flutter analysis,
  widget tests, and the non-skipped ABI v2/v3 integration tests.
- `cargo xtask model-check` runs the deterministic Stage 1 tracer checks.

Commands planned for later roadmap stages are not advertised as passing until
their implementation and blocking CI evidence exist.
