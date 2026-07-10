# Evidence-Driven Roadmap

Velum starts with no implementation and no validated demand. The roadmap
therefore advances by evidence gates, not calendar promises. A stage is complete
only when its exit criteria and retained evidence exist.

## Stage 0: Problem Validation and Baselines

**Status:** In progress since 2026-07-11. Experiment manifests, evidence
retention rules, and the locally validated workload harness are defined in
[`experiments/stage0`](../experiments/stage0/README.md). Operator interviews,
reference-server capture, and executable baseline verification remain open.

**Outcome:** confirm that cross-carrier continuity is valuable and establish
honest competitor baselines.

### Scope

- Interview at least five operators who currently maintain more than one tunnel
  protocol or transport configuration.
- Record network failure cases: UDP block, UDP rate limit, burst loss, NAT
  rebinding, path MTU, TCP-only captive or enterprise networks.
- Define reproducible workloads for interactive TCP, bulk TCP, real-time UDP,
  DNS-like UDP, and idle mobile behavior.
- Benchmark unmodified MASQUE where available, AnyTLS, Hysteria 2, and one
  conventional VPN or proxy baseline under the same conditions.
- Decide the first local adapter: SOCKS is the default hypothesis because it
  limits OS-specific scope.

### Deliverables

- Versioned network and workload manifests.
- Raw baseline results with hardware and software versions.
- Updated [evidence ledger](evidence-ledger.md).
- Accepted, rejected, or revised ADR-0001.

### Exit Gate

At least three target operators identify application-visible reconnects or
manual protocol switching as a material problem, and the fault matrix reliably
reproduces it.

### Stop or Roll Back

If target users primarily value easier configuration, peak throughput, or
client ecosystem rather than continuity, revise the position before creating a
wire protocol.

## Stage 1: In-Process Session Tracer

**Outcome:** prove the session state model without networking or camouflage.

### Scope

- Create a Rust workspace with `protocol`, `session`, `carrier-api`, `policy`,
  and `telemetry` crates only.
- Implement an in-memory carrier simulator supporting loss, duplication,
  reordering, delay, black holes, and recovery.
- Implement reliable-stream flow identity, epochs, logical acknowledgements,
  bounded replay windows, and transition state.
- Write a deterministic state-machine model before choosing final frame bytes.
- Establish `cargo xtask architecture` and `cargo xtask model-check`.

### Validation

- Exhaustively test small state spaces for duplicate delivery, missing delivery,
  invalid epoch rollback, and unbounded buffering.
- Run 10,000 seeded randomized transitions with byte-exact checksums.
- Fuzz every parser introduced in this stage.

### Exit Gate

All reliable-stream invariants hold, memory remains bounded, dependency gates
pass, and the state model survives review.

### Rollback

No compatibility burden exists. Change or discard the state model and ADR-0002
before external implementations appear.

## Stage 2: QUIC End-to-End Slice

**Outcome:** establish the preferred carrier and first real client-to-server
flow.

### Scope

- Add `carrier-quic`, a minimal server, and a SOCKS client adapter.
- Use a maintained Rust QUIC and TLS implementation; do not fork cryptography.
- Carry reliable streams on QUIC streams and UDP flows on QUIC datagrams.
- Implement datagram size discovery, oversize errors, destination policy,
  authentication, quotas, and redacted telemetry.
- Run SSH/HTTP/WebSocket and DNS/game-like UDP test workloads.

### Validation

- Compare against Hysteria 2 and a direct QUIC baseline under the Stage 0
  matrix.
- Validate normal-path overhead Q-003 and bounded overload Q-004.
- Security review authentication, destination filtering, amplification, and
  resource exhaustion.

### Exit Gate

The QUIC-only implementation is correct and operationally diagnosable. It need
not beat Hysteria 2; regressions must be understood and within stated budgets.

### Rollback

Retain the carrier-independent session tests. Replace the QUIC library or
carrier mapping without changing logical state ownership.

## Stage 3: TLS Fallback and Live Transition

**Outcome:** prove Velum's primary differentiator.

### Scope

- Add `carrier-tls` using TLS 1.3 over TCP.
- Implement authenticated carrier attachment and fresh session epochs.
- Add cold fallback, optional warm fallback, transition hysteresis, and
  recovery probing.
- Preserve reliable streams across QUIC black holes and rate limits.
- Define honest UDP behavior on the TLS carrier; do not silently relabel
  reliable encapsulation as datagram-equivalent.

### Validation

- Execute Q-001 and Q-002 across the complete network matrix.
- Test simultaneous failure, late QUIC packets, duplicate control frames,
  attach replay, server restart, and flow-control exhaustion.
- Measure idle bytes, memory, sockets, mobile wakeups, and false transitions.

### Exit Gate

- Zero missing or duplicate reliable bytes in 10,000 deterministic fault
  trials.
- Warm-transition P95 below 2 seconds in the reference environment.
- Cold fallback provides useful service before the QUIC timeout would have
  expired.
- Operators can explain every transition from structured telemetry.

### Stop or Revise

If recovery is slower or less reliable than application reconnect, simplify the
product to deterministic carrier selection and reject ADR-0001 rather than
shipping false continuity.

## Stage 4: Forest Native Coexistence

**Outcome:** integrate real service behavior without coupling it to protocol
correctness.

### Scope

- Add `forest` with real static-service and reverse-proxy modes.
- Define one encrypted application exchange for authentication that has no
  clear-text Velum marker.
- Build enabled-versus-disabled differential probes.
- Define a versioned profile schema with latency, byte, CPU, and timing budgets.
- Test profile rotation inside authenticated sessions.

### Validation

- Run Q-005 across valid, invalid, slow, replayed, and malformed probes.
- Compare handshake and endpoint configuration to common deployments.
- Conduct at least five independent operator deployments.
- Commission focused external review of the threat model and probe methodology.

### Exit Gate

No deterministic pre-authentication difference remains in the probe suite,
cover-service health is independent, and deployment cost is documented.

### Rollback

Disable traffic profiles while retaining real cover-service routing. Forest
failures must never require changes to session delivery state.

## Stage 5: Protocol Draft and Interoperability Preview

**Outcome:** turn validated behavior into a reviewable protocol draft.

### Scope

- Specify frame grammar, state transitions, capability negotiation, error
  codes, security considerations, and IANA-style registries if needed.
- Publish canonical encoding and state-machine test vectors.
- Freeze protocol version `0` for a time-bounded preview, not production.
- Build a second implementation of the codec or an independent conformance
  harness to detect implementation-defined behavior.
- Run parser fuzzing, dependency audits, denial-of-service tests, and an
  independent security review.

### Exit Gate

- Two independent codec consumers agree on all canonical vectors.
- Every normative requirement maps to a conformance test or explicit human
  review.
- No unresolved critical security finding exists.
- Upgrade and downgrade behavior is tested.

### Compatibility Window

Preview version `0` may break with release notes and migration tooling. Version
`1` is not declared until the preview has real deployments and the state model
has stopped changing. Version `0` support receives an explicit removal release
and date when version `1` is proposed.

## Stage 6: Production Candidate

**Outcome:** operate a narrowly scoped, supportable release.

### Scope

- Signed reproducible release artifacts and an upgrade/rollback procedure.
- Configuration validation, secret rotation, certificate automation, quotas,
  dashboards, and incident runbooks.
- Soak tests, compatibility tests, dependency review, and documented resource
  limits.
- Stable operator-facing metrics and privacy review of telemetry.

### Exit Gate

- Production readiness review is complete.
- Security review findings are closed or explicitly accepted with owners.
- The required CI suite meets its calibrated feedback budget.
- A rollback is rehearsed against a real deployment.

## Deferred Evolution Triggers

Add these capabilities only when their trigger is observed:

| Capability | Trigger |
|---|---|
| Reliable messages / partial reliability | At least two real workloads cannot be expressed efficiently as stream or datagram |
| Multipath striping | Single-carrier placement leaves measured aggregate capacity unused and transition correctness is already stable |
| MASQUE carrier | Interoperability with existing HTTP proxy infrastructure materially reduces deployment cost |
| TUN adapter | SOCKS/CONNECT cannot support validated target applications |
| Multi-hop privacy | A defined privacy use case, threat model, and operator model justify metadata and latency cost |
| Post-quantum session authentication | Reviewed standards and libraries are deployable, with an explicit threat and compatibility plan |

## Roadmap Maintenance

- Update stage status, evidence links, and threshold calibration in this file.
- Do not mark a stage complete based on code presence alone.
- Significant scope changes require a new or superseding ADR.
- Accepted ADRs remain immutable; later changes supersede them.
- Every compatibility path has an owner and removal condition.
