# Evidence-Driven Delivery Roadmap

Velum advances through evidence gates, not calendar promises. This document is
both the delivery plan from repository bootstrap to a production candidate and
the implementation status dashboard.

**Last verified:** 2026-07-13

## Status Rules

### Stage status

| Status | Meaning |
|---|---|
| `NOT STARTED` | No retained implementation evidence exists |
| `IN PROGRESS` | At least one deliverable has evidence, but the exit gate is open |
| `BLOCKED` | Progress depends on a named external decision or prerequisite |
| `DONE` | Every exit criterion has retained evidence |

### Item status

| Status | Meaning |
|---|---|
| `TODO` | No implementation evidence exists |
| `PARTIAL` | An artifact exists, but validation or required scope is incomplete |
| `DONE` | The linked artifact and verification command satisfy the item |
| `BLOCKED` | A named prerequisite prevents progress |

Code presence alone never means `DONE`. Every completed item must link to a
repository artifact, retained result, CI run, review, or executable command.
When evidence becomes stale or a regression is found, move the item back to
`PARTIAL`.

## Current Dashboard

| Stage | Outcome | Status | Completed evidence | Next gate |
|---|---|---|---|---|
| Foundation | Reproducible repository and engineering controls | `IN PROGRESS` | Pinned workspace, executable architecture/docs/test gates, Stage 0 harness, CI and governance artifacts | Default-branch CI evidence, policy review, CI health baseline |
| 0 | Validate the problem and competitor baselines | `IN PROGRESS` | Manifests and retained-result integrity checks pass; local harness tests pass | Five interviews, reference server pinning, executable baselines, raw runs |
| 1 | Prove the carrier-independent session state model | `IN PROGRESS` | Deterministic tracer, exhaustive receive traces, and a 10,000-seed transition campaign retained by CI | ADR-0002 maintainer review |
| 2 | Deliver a QUIC end-to-end slice | `IN PROGRESS` | QUIC stream relay, admission composition, bounded configuration, and redacted relay events | Live QUIC workload and datagram evidence |
| 3 | Preserve streams across QUIC/TLS transitions | `IN PROGRESS` | TLS 1.3/TCP carrier contract and loopback integration test | 10,000 correct fault trials and transition budgets |
| 4 | Add Forest Native service coexistence | `NOT STARTED` | Threat-model proposal only | Differential probes and independent deployments |
| 5 | Publish an interoperable protocol preview | `IN PROGRESS` | v0 draft, bounded codec, and authenticated attachment contract | Canonical vectors, two codec consumers, fuzzing, and security review |
| 6 | Operate a production candidate | `NOT STARTED` | None | Readiness review and rehearsed rollback |

The current critical path is:

```text
Foundation controls
  -> Stage 0 demand and baseline evidence
  -> Stage 1 session tracer
  -> Stage 2 QUIC slice
  -> Stage 3 live carrier transition
  -> Stage 4 Forest Native coexistence
  -> Stage 5 interoperable preview
  -> Stage 6 production candidate
```

## Foundation: Repository and Engineering Bootstrap

**Outcome:** make every later result reproducible and keep the implementation
inside enforceable responsibility boundaries.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| F-01 | Repository and documentation index | `DONE` | [`README.md`](../README.md), [`docs/README.md`](README.md) |
| F-02 | Vision, landscape, architecture, and protocol requirements | `DONE` | [`vision.md`](vision.md), [`landscape.md`](landscape.md), [`architecture.md`](architecture.md), [`protocol-requirements.md`](protocol-requirements.md) |
| F-03 | Decision records for product, multi-carrier sessions, and Forest Native | `PARTIAL` | ADR-0001 through ADR-0003 exist but remain `Proposed` |
| F-04 | Machine-readable module ownership and dependency contract | `DONE` | [`architecture-contract.yaml`](architecture-contract.yaml); `cargo xtask architecture` passes |
| F-05 | Versioned validation manifests and validation command | `DONE` | `node validation/validate.mjs` passes |
| F-06 | Local workload harness for all five workload classes | `DONE` | `node validation/harness/harness.test.mjs` passes when local TCP/UDP binding is permitted |
| F-07 | Rust workspace with pinned stable toolchain and dependency policy | `DONE` | [`rust-toolchain.toml`](../rust-toolchain.toml), [`deny.toml`](../deny.toml); `cargo check --workspace` and `cargo deny check` pass |
| F-08 | `xtask` entry points for architecture, tests, and model checking | `DONE` | `cargo xtask architecture`, `cargo xtask docs`, and `cargo xtask test` pass; Stage 1 adds `model-check` only when it becomes blocking |
| F-09 | Required CI for docs, manifests, tests, formatting, lint, and dependency policy | `PARTIAL` | [`ci.yml`](../.github/workflows/ci.yml) runs all current gates; default-branch run and required-check evidence await a GitHub remote |
| F-10 | Contribution, security-reporting, release, and support policies | `PARTIAL` | [`CONTRIBUTING.md`](../CONTRIBUTING.md), [`SECURITY.md`](../SECURITY.md), [`RELEASES.md`](../RELEASES.md), and [`SUPPORT.md`](../SUPPORT.md) exist; owner review and GitHub Private Vulnerability Reporting remain open |
| F-11 | CI feedback and flake baselines | `PARTIAL` | [`ci-health.yml`](../.github/workflows/ci-health.yml) retains and enforces metrics after 20 duration and 100 flake samples; no remote samples exist yet |

### Foundation Exit Gate

- A clean checkout can run every current validation command from documented
  toolchain versions.
- CI blocks structural manifest errors, formatting or lint failures, test
  failures, forbidden dependencies, dependency cycles, and unowned modules.
- Architecture-changing work requires an ADR; compatibility shims require an
  owner and removal condition.

### Rollback

No runtime compatibility exists yet. Replace tools or repository layout while
preserving evidence formats and decision history.

## Stage 0: Problem Validation and Baselines

**Outcome:** confirm that cross-carrier continuity is valuable and establish
honest competitor baselines before defining a wire protocol.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S0-01 | Versioned network failure matrix | `DONE` | [`networks.json`](../validation/manifests/networks.json); structural validator passes |
| S0-02 | Interactive TCP, bulk TCP, real-time UDP, DNS-like UDP, and idle-mobile workloads | `DONE` | [`workloads.json`](../validation/manifests/workloads.json); local harness tests pass |
| S0-03 | Candidate MASQUE, AnyTLS, Hysteria 2, and conventional baseline revisions | `PARTIAL` | [`baselines.json`](../validation/manifests/baselines.json); all remain `candidate_pinned` |
| S0-04 | Reference client and server environment pinned | `PARTIAL` | Client is recorded; server OS and kernel are unset |
| S0-05 | At least five operator interviews | `TODO` | Five redacted records under `validation/interviews/` |
| S0-06 | At least three operators confirm material reconnect or manual switching pain | `TODO` | Three interview records classified `yes` |
| S0-07 | Every baseline builds and covers its declared workloads | `TODO` | `node validation/validate.mjs --ready` passes plus retained build metadata |
| S0-08 | Raw runs execute across the failure matrix | `TODO` | Immutable result directories following [`results/README.md`](../validation/results/README.md) pass `node validation/results/validate.mjs` |
| S0-09 | Repeated trials establish variance and honest baseline comparisons | `TODO` | Sample counts, failures, environment, and raw artifacts retained |
| S0-10 | Evidence ledger updated from reviewed interviews and measurements | `TODO` | [`evidence-ledger.md`](evidence-ledger.md) links retained evidence |
| S0-11 | First adapter decision: SOCKS, CONNECT, or TUN | `PARTIAL` | [ADR-0006](adr/0006-stage2-connect-adapter.md) selects experimental IP-only CONNECT; operator evidence remains required before a production commitment |
| S0-12 | ADR-0001 accepted, rejected, or superseded | `TODO` | Decision status and evidence are recorded |

### Exit Gate

- At least three target operators identify application-visible reconnects or
  manual protocol switching as a material problem.
- The fault matrix reliably reproduces those failures.
- Baselines and toolchains have immutable versions, and raw comparison runs are
  reviewable.

### Stop or Revise

If users primarily value easier configuration, peak throughput, or client
ecosystem rather than continuity, revise the product position before creating a
wire protocol.

## Stage 1: In-Process Session Tracer

**Outcome:** prove the session state model without networking or camouflage.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S1-01 | `protocol`, `session`, `carrier-api`, `policy`, and `telemetry` crates | `DONE` | [`crates/`](../crates); `cargo check --workspace` and `cargo xtask architecture` pass |
| S1-02 | Deterministic state-machine model before final frame bytes | `DONE` | [`session-tracer.md`](session-tracer.md); `cargo xtask model-check` passes |
| S1-03 | In-memory carrier simulator for loss, duplication, reordering, delay, black holes, and recovery | `DONE` | [`simulator.rs`](../crates/velum-session/src/simulator.rs); deterministic scenarios cover every listed fault and recovery |
| S1-04 | Flow identity, epochs, logical acknowledgements, replay windows, and transition state | `DONE` | [`velum-session`](../crates/velum-session/src/lib.rs) carries `FlowId` and `Epoch` on segments and acknowledgements; [`transition.rs`](../crates/velum-session/src/transition.rs) enforces the bounded current/retiring epoch window; `cargo xtask model-check` passes |
| S1-05 | Bounded memory, queues, replay windows, and timeouts | `DONE` | Pending segment, byte, and age limits are enforced by [`velum-session`](../crates/velum-session/src/lib.rs); the bounded epoch window and receive cursor reject stale/replayed delivery; `cargo xtask model-check` passes |
| S1-06 | Duplicate-free, gap-free reliable-stream behavior | `DONE` | Exhaustive short receive traces, deterministic fault recovery, and [`campaign.rs`](../crates/velum-session/src/campaign.rs) enforce byte-exact, duplicate-free recovery; `cargo xtask model-check` passes |
| S1-07 | 10,000 seeded randomized transitions with byte-exact checksums | `DONE` | Seeds `0..9999`, varying transition positions, and aggregate checksum `4550704779471716960` are verified by [`campaign.rs`](../crates/velum-session/src/campaign.rs); required CI retains the model-check result |
| S1-08 | Parser fuzz targets and corpus retention | `DONE` | No Stage 1 frame parser exists by design; the deterministic tracer accepts typed in-memory segments only. Any introduced parser must add a fuzz target and retained corpus before this item may remain complete |
| S1-09 | `cargo xtask architecture` and `cargo xtask model-check` | `DONE` | [`ci.yml`](../.github/workflows/ci.yml) executes `cargo xtask test` and a separately retained `cargo xtask model-check` result |
| S1-10 | ADR-0002 accepted, rejected, or superseded | `PARTIAL` | [`ADR-0002`](adr/0002-multi-carrier-sessions.md) contains tracer review evidence; named protocol-maintainer decision remains required |

### Exit Gate

All reliable-stream invariants hold, memory remains bounded, dependency gates
pass, and the state model survives review.

### Rollback

No external compatibility burden exists. Change or discard the state model and
ADR-0002 before external implementations appear.

## Stage 2: QUIC End-to-End Slice

**Outcome:** establish the preferred carrier and first real client-to-server
flow.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S2-01 | Maintained QUIC/TLS library selected and threat-reviewed | `DONE` | [ADR-0004](adr/0004-quic-carrier.md) records the Quinn 0.11.11 pin, threat review, version policy, and invalidation triggers |
| S2-02 | `carrier-quic` behind the narrow carrier contract | `PARTIAL` | [`velum-carrier-quic`](../crates/velum-carrier-quic) maps QUIC streams and datagrams behind `velum-carrier-api`; live client/server contract tests remain part of S2-03 |
| S2-03 | Minimal server and direct client API | `PARTIAL` | [`velum-client-api`](../crates/velum-client-api) exposes direct QUIC streams, independent send/receive ownership, connection-closed observation, and datagrams; [`velum-client-runtime`](../crates/velum-client-runtime) owns lifecycle snapshots, managed connection and closure tasks, cancellation, and stale-generation rejection; [`velum-client-ffi`](../crates/velum-client-ffi) exposes ABI v2 synchronous stream and non-blocking runtime lifecycle control plus cancellable full-duplex stream calls. A production server runtime and live client/server workload evidence remain. |
| S2-04 | QUIC streams for reliable flows | `TODO` | A production server runtime must forward authenticated QUIC bidirectional streams to allowed TCP targets and retain SSH, HTTP, and WebSocket workload evidence. |
| S2-05 | QUIC datagrams with explicit MTU and oversize behavior | `PARTIAL` | [`velum-carrier-quic`](../crates/velum-carrier-quic) discovers the QUIC DATAGRAM maximum and rejects oversize payloads; DNS-like and real-time UDP live workload evidence remains |
| S2-06 | Authentication, destination deny-by-default policy, and quotas | `PARTIAL` | [`velum-server`](../crates/velum-server) provides constant-time configured-secret authentication, exact deny-by-default destination allowlists, and per-principal in-memory quotas. A production application must invoke them before connecting a target; abuse-load evidence remains. |
| S2-07 | Stable configuration schema and redacted telemetry vocabulary | `PARTIAL` | [`QuicRelayEvent`](../crates/velum-telemetry/src/lib.rs) contains only lifecycle classes. A production application configuration schema, compatibility tests, and export tests remain. |
| S2-08 | Graceful shutdown, overload shedding, and resource limits | `TODO` | A production relay runtime must bound control records, target connection time, and admission quotas; it must close active endpoints, drain tasks to a validated deadline, and abort stragglers. Retained load evidence is also required. |
| S2-09 | Comparison against Hysteria 2 and direct QUIC | `TODO` | Stage 0 matrix results retained |
| S2-10 | Normal-path overhead and overload budgets calibrated | `TODO` | Q-003 and Q-004 evidence retained |

### Exit Gate

The QUIC-only implementation is correct, resource-bounded, secure at its stated
boundary, and operationally diagnosable. It need not beat Hysteria 2; every
regression must be understood and within an explicit budget.

### Rollback

Retain carrier-independent session tests. Replace the QUIC library or mapping
without changing logical state ownership.

## Stage 3: TLS Fallback and Live Transition

**Outcome:** prove Velum's primary differentiator.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S3-01 | `carrier-tls` using TLS 1.3 over TCP | `DONE` | [`velum-carrier-tls`](../crates/velum-carrier-tls) maps one TLS 1.3/TCP connection to one reliable stream, rejects datagrams, and proves a certificate-validated loopback exchange with `cargo test -p velum-carrier-tls` |
| S3-02 | Authenticated carrier attachment with fresh session epochs | `DONE` | [`velum-crypto`](../crates/velum-crypto) binds an HMAC-SHA-256 proof to session, carrier, and epoch; [`velum-session`](../crates/velum-session) accepts only one authenticated current-epoch attachment and rejects forged, replayed, stale, and future attachments with `cargo test -p velum-crypto -p velum-session` |
| S3-03 | Cold fallback, optional warm fallback, and recovery probing | `PARTIAL` | [`velum-policy`](../crates/velum-policy) supplies deterministic cold/warm fallback, recovery, and rate-limit decisions; live carrier probes remain |
| S3-04 | Transition hysteresis and false-transition control | `PARTIAL` | Policy tests cover failure confirmation, recovery confirmation, and rate limiting; retained loss/jitter measurements remain |
| S3-05 | Reliable-stream resume from logical acknowledgement cursors | `PARTIAL` | [`velum-session`](../crates/velum-session/src/lib.rs) reissues only pending sequences under the new epoch; its 10,000-seed campaign checks byte-exact delivery; live transition evidence remains |
| S3-06 | Honest UDP semantics over TLS | `PARTIAL` | [`fallback_supports`](../crates/velum-policy/src/lib.rs) permits only streams over TLS fallback; application wiring and telemetry export review remain |
| S3-07 | Simultaneous failure, late packets, duplicate controls, server restart, and flow-control exhaustion | `PARTIAL` | Existing deterministic tracer rejects stale epochs and duplicates and bounds pending queues; server-restart and live carrier fault tests remain |
| S3-08 | Idle bytes, memory, sockets, and mobile wakeup budgets | `PARTIAL` | Cold/warm fallback is explicit in policy and queue/socket ownership is bounded by the tracer; calibrated retained measurements remain |
| S3-09 | Structured explanation for every transition | `PARTIAL` | [`velum-telemetry`](../crates/velum-telemetry/src/lib.rs) records redacted carrier classes and reasons; operator exercise and exporter assertions remain |
| S3-10 | Upgrade, downgrade, and mixed-version transition behavior | `PARTIAL` | [`VersionRange`](../crates/velum-protocol/src/lib.rs) chooses the newest shared version and rejects disjoint peers; wire-level compatibility matrix remains |
| S3-11 | Bounded TLS shaping and optional inner multiplexing | `TODO` | [`tls-fallback-evolution.md`](tls-fallback-evolution.md) records the deferred direction, evidence thresholds, ownership boundary, and required safety controls; no implementation begins before retained workload and probe comparisons justify it |

### Exit Gate

- Zero missing or duplicate reliable bytes in 10,000 deterministic fault
  trials.
- Warm-transition P95 is below 2 seconds in the reference environment.
- Cold fallback provides useful service before the QUIC timeout would have
  expired.
- Operators can explain every transition from structured telemetry.

### Stop or Revise

If recovery is slower or less reliable than application reconnect, simplify to
deterministic carrier selection and supersede ADR-0001 rather than shipping
false continuity.

## Stage 4: Forest Native Coexistence

**Outcome:** integrate real service behavior without coupling it to session
correctness.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S4-01 | Real static-service and reverse-proxy modes | `PARTIAL` | [`velum-forest`](../crates/velum-forest) serves bounded HTTP/1.1 static responses and reverse-proxies cover requests. A production application integration, external TLS termination, deployment evidence, and updated loopback coverage remain. [ADR-0011](adr/0011-cover-service-listener-wiring.md) records the boundary. |
| S4-02 | Encrypted authentication exchange with no clear-text Velum marker | `PARTIAL` | The existing QUIC admission record is processed only after the carrier's TLS handshake, and its application record has no clear-text Velum identifier; raw-wire capture and focused probe review remain |
| S4-03 | Enabled-versus-disabled differential probe suite | `PARTIAL` | [`velum-forest`](../crates/velum-forest) compares cover responses with traffic profiles enabled and killed for valid, invalid, slow, replayed, and malformed inputs; `cargo test -p velum-forest` passes. Live endpoints and retained probe runs remain |
| S4-04 | Versioned traffic-profile schema and rotation | `PARTIAL` | [`velum-forest`](../crates/velum-forest) validates versioned, expiring profiles and accepts rotation only through an injected authenticated verifier; `cargo test -p velum-forest` passes. Wire compatibility, deployment authentication integration, and measurements remain; initial scope and AnyTLS-derived constraints are recorded in [`anytls-design-notes.md`](anytls-design-notes.md) |
| S4-05 | Latency, byte, CPU, and timing budgets | `TODO` | Profile measurements are retained |
| S4-06 | Forest failure isolation and kill switch | `PARTIAL` | [`ForestRuntime`](../crates/velum-forest/src/lib.rs) disables only traffic-profile selection and has no session dependency; its isolation test runs with `cargo test -p velum-forest`. Application-level failure injection remains |
| S4-07 | Five independent operator deployments | `TODO` | Redacted deployment reports retained |
| S4-08 | Focused external threat-model and probe review | `TODO` | Findings are closed or explicitly accepted with owners |
| S4-09 | ADR-0003 accepted, rejected, or superseded | `TODO` | Decision reflects deployment and probe evidence |

### Exit Gate

No deterministic pre-authentication difference remains in the probe suite,
cover-service health is independent, and deployment cost is documented.

### Rollback

Disable traffic profiles while retaining real cover-service routing. Forest
failures must never require changes to session delivery state.

## Stage 5: Protocol Draft and Interoperability Preview

**Outcome:** turn validated behavior into a reviewable protocol version `0`.

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S5-01 | Frame grammar, state transitions, negotiation, and error registry | `PARTIAL` | [`protocol-v0.md`](protocol-v0.md), [`velum-protocol`](../crates/velum-protocol), and [`velum-session`](../crates/velum-session) provide a bounded codec, authenticated in-memory frame dispatcher, and error registry; carrier I/O integration and review remain |
| S5-02 | Security considerations and explicit threat boundaries | `PARTIAL` | [`protocol-v0.md`](protocol-v0.md) states pre-auth, attachment-secret, replay, and no-cross-process-resume boundaries; threat-model review remains |
| S5-03 | Canonical encoding and state-machine test vectors | `TODO` | Vectors are stable for the preview window |
| S5-04 | Second codec consumer or independent conformance harness | `TODO` | Both consumers agree on all canonical vectors |
| S5-05 | Normative requirement-to-test traceability | `TODO` | Every requirement maps to automation or named human review |
| S5-06 | Parser fuzzing, dependency audits, DoS and resource-limit tests | `TODO` | Required CI and retained campaigns pass |
| S5-07 | Independent security review | `TODO` | No unresolved critical finding exists |
| S5-08 | Version 0 upgrade, downgrade, and removal policy | `TODO` | Compatibility window, owner, tooling, and removal date are published |
| S5-09 | Signed preview artifacts and reproducible-build proof | `TODO` | Independent rebuild matches published artifacts |

### Exit Gate

Two independent codec consumers agree on canonical vectors, normative behavior
is testable, critical security findings are closed, and upgrade/downgrade
behavior is verified.

### Compatibility Window

Version `0` may break only with release notes and migration tooling. Version `1`
is not declared until real deployments exist and the state model has stopped
changing. Removing version `0` requires an explicit release and date.

### Rollback

Withdraw the preview artifacts, retain the failing vectors and findings, and
revise the draft without creating a production compatibility promise.

## Stage 6: Production Candidate

**Outcome:** operate a narrowly scoped, supportable release whose security,
reliability, capacity, and recovery claims are backed by evidence.

### Release and Supply Chain

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S6-01 | Signed, reproducible, versioned release artifacts | `TODO` | Independent rebuild and signature verification pass |
| S6-02 | SBOM, license policy, provenance, and vulnerability scanning | `TODO` | Release gate produces and checks retained artifacts |
| S6-03 | Staged rollout, compatibility window, and rollback procedure | `TODO` | Canary and rollback are exercised |
| S6-04 | Supported platform and upgrade matrix | `TODO` | Clean install and N-1 upgrade tests pass |

### Security and Abuse Resistance

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S6-05 | Threat model, independent review, and finding ownership | `TODO` | Findings are closed or explicitly accepted with expiry/review date |
| S6-06 | Secret and certificate provisioning, rotation, and revocation | `TODO` | Rotation and compromised-secret drills pass |
| S6-07 | Least-privilege runtime and destination controls | `TODO` | Deployment and negative authorization tests pass |
| S6-08 | Abuse quotas, amplification resistance, and DoS containment | `TODO` | Adversarial load tests demonstrate bounded established-session impact |
| S6-09 | Privacy classification, retention, deletion, and telemetry review | `TODO` | Policy and automated redaction checks pass |

### Reliability, Capacity, and Recovery

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S6-10 | User-visible SLOs and error budget | `TODO` | SLOs map to measured service indicators and alert policy |
| S6-11 | Normal, peak, overload, and growth capacity envelope | `TODO` | Load tests record throughput, latency, concurrency, memory, CPU, sockets, and cost |
| S6-12 | Timeouts, retry limits, backpressure, and graceful degradation | `TODO` | Partial-failure and overload tests pass |
| S6-13 | Soak and fault-injection campaigns | `TODO` | Duration, environment, failures, and leak trends retained |
| S6-14 | State-loss and server-restart recovery semantics | `TODO` | RPO/RTO or explicit non-persistence contract is tested |
| S6-15 | Configuration backup, restore, and disaster recovery | `TODO` | Restore and regional/server replacement drill meets stated objectives |

### Operations and Support

| ID | Deliverable | Status | Evidence / completion check |
|---|---|---|---|
| S6-16 | Stable metrics, logs, traces, dashboards, and actionable alerts | `TODO` | Failure drills are detectable and diagnosable without payload access |
| S6-17 | Configuration validation and safe defaults | `TODO` | Invalid and dangerous configurations fail before serving |
| S6-18 | Deployment, upgrade, rollback, incident, and recovery runbooks | `TODO` | An operator unfamiliar with implementation details completes drills |
| S6-19 | Build, release, security, operations, and incident ownership | `TODO` | Every runbook and alert has an accountable owner and review cadence |
| S6-20 | Support policy and known-limit documentation | `TODO` | Supported scope, exclusions, and escalation paths are published |
| S6-21 | Production readiness review | `TODO` | Security, reliability, operability, capacity, and rollback sign-offs retained |

### Exit Gate

- Production readiness review is complete.
- Security findings are closed or explicitly accepted with owners and review
  dates.
- The required CI suite meets its calibrated feedback budget.
- SLOs, limits, alerts, and incident ownership are explicit.
- Upgrade, rollback, secret rotation, overload, and recovery are rehearsed
  against a real deployment.

### Rollback and Containment

Stop rollout, revoke affected credentials or artifacts, and return to the last
verified release using the rehearsed procedure. If compatibility or session
state prevents bounded rollback, the production gate fails until that behavior
has an explicit recovery path.

## Production-Grade Definition

Velum is not production-grade merely because it has binaries or passes
functional tests. The claim requires all of the following:

| Dimension | Required evidence |
|---|---|
| Correctness | Session invariants, compatibility tests, fault tests, and conformance vectors pass |
| Security | Reviewed threat model, no unresolved critical findings, managed secrets, abuse controls |
| Reliability | Stated SLOs, bounded failure behavior, soak evidence, exercised recovery |
| Capacity | Published resource and concurrency limits with overload behavior and cost envelope |
| Operability | Redacted telemetry, actionable alerts, dashboards, runbooks, and named ownership |
| Delivery | Reproducible signed artifacts, staged rollout, tested upgrade and rollback |
| Maintainability | Architecture contract and dependency gates execute in CI; compatibility paths expire |
| Supportability | Supported platforms, known limits, incident process, and release policy are published |

No production claim is allowed while any Stage 6 exit criterion is open.

## Cross-Stage Fitness Functions

These checks start in the earliest applicable stage and remain blocking in all
later stages.

| Fitness function | Starts | Target command or evidence |
|---|---|---|
| Manifest integrity | Foundation | `node validation/validate.mjs` |
| Workload harness correctness | Foundation | `node validation/harness/harness.test.mjs` |
| Module ownership and allowed dependencies | Foundation | `cargo xtask architecture` |
| Formatting, lint, and workspace tests | Foundation / Stage 1 | `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` |
| State-machine invariants | Stage 1 | `cargo xtask model-check` plus seeded and exhaustive tests |
| Parser resilience | Stage 1 | Retained fuzz targets and campaign results |
| Performance and degradation budgets | Stage 0 onward | Versioned matrix runs and retained raw results |
| Wire compatibility and conformance | Stage 5 | Canonical vectors and two independent consumers |
| Supply-chain and release integrity | Stage 5 onward | Reproducible build, SBOM, provenance, signature, and audit gates |
| Production reliability | Stage 6 | SLO evaluation, soak, fault, overload, rollback, and recovery drills |

Numeric budgets must be calibrated from retained baselines. Do not copy generic
thresholds when the reference environment or user impact does not support them.

## Deferred Evolution Triggers

Add these capabilities only when their trigger is observed:

| Capability | Trigger |
|---|---|
| Reliable messages / partial reliability | At least two real workloads cannot be expressed efficiently as stream or datagram |
| Multipath striping | Single-carrier placement leaves measured aggregate capacity unused and transition correctness is stable |
| MASQUE carrier | Existing HTTP proxy infrastructure materially reduces deployment cost |
| TUN adapter | SOCKS/CONNECT cannot support validated target applications |
| Multi-hop privacy | A defined privacy use case, threat model, and operator model justify metadata and latency cost |
| Post-quantum session authentication | Reviewed standards and libraries are deployable with an explicit threat and compatibility plan |
| Distributed control plane | A measured operating or team boundary cannot be handled by the single-deployment model |

## Maintenance Procedure

At every meaningful merge or stage review:

1. Update item status only with a linked artifact or repeatable command.
2. Record new measurements and interviews in the evidence ledger before using
   them to accept a decision.
3. Recalculate the dashboard from item and exit-gate status; never mark a stage
   complete manually because code exists.
4. Add or supersede an ADR for significant scope, ownership, security, wire, or
   compatibility changes. Accepted ADRs remain immutable.
5. Give every compatibility path an owner, expiry condition, and removal test.
6. Move a completed item back to `PARTIAL` when its evidence no longer passes.
