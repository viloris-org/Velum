# Protocol Requirements and Fitness Functions

Status: Proposed. Numeric thresholds are initial experiment gates, not
production SLOs. They must be recalibrated from retained benchmark evidence.

## Delivery Semantics

### Reliable stream

- Ordered byte delivery with no duplication exposed to the application.
- Half-close behavior must be explicit.
- Eligible for carrier migration.
- Flow control is per flow plus per session; one stalled flow must not consume
  unbounded session memory.

### Unreliable datagram

- Message boundaries are preserved.
- Loss, reordering, and duplication may occur.
- Datagrams are not retransmitted during carrier transition by default.
- The maximum payload is discoverable; oversize behavior is explicit.
- A carrier lacking native unreliable delivery may reject the flow or use a
  negotiated degraded mode, but must not silently claim equivalent semantics.

### Deferred semantics

Reliable messages, partial reliability, deadline delivery, and multipath
striping remain experimental until stream migration is correct.

## Session Invariants

1. Session identity is independent of carrier connection identity.
2. A flow ID is unique within a session and is never reused.
3. Every state-changing control frame is scoped to a session epoch.
4. An attached carrier is authenticated and bound to exactly one session.
5. A replayed attach cannot roll back the session epoch or delivery cursor.
6. Reliable bytes are delivered to the application at most once and in order.
7. Flow-control accounting remains bounded during carrier failure.
8. Forest profiles cannot modify authentication or delivery semantics.
9. Unknown mandatory behavior fails closed; optional behavior is negotiated.
10. Protocol errors have stable machine-readable codes without leaking secrets.

## Quality Scenarios

### Q-001 Session continuity

- **Stimulus:** the active QUIC path becomes a complete UDP black hole.
- **Environment:** one long-lived reliable flow, authenticated warm TLS carrier.
- **Response:** attach a new session epoch and resume the flow without an
  application-visible reset or duplicate byte.
- **Initial measure:** P95 transition below 2 seconds; zero duplicate or missing
  bytes across 10,000 deterministic fault trials.
- **Evidence:** integration trace, packet capture, and application checksum.

### Q-002 Cold fallback

- **Stimulus:** UDP is unavailable at initial connection.
- **Environment:** TCP 443 is available; no cached path state.
- **Response:** establish the TLS carrier without waiting for a long QUIC
  timeout.
- **Initial measure:** P95 first useful byte no more than 300 ms plus one TCP/TLS
  handshake over the TLS-only baseline in the same network.
- **Evidence:** controlled network matrix and baseline comparison.

### Q-003 Normal-path overhead

- **Stimulus:** mixed reliable stream and datagram workload.
- **Environment:** stable UDP path, no transition, same QUIC library and host.
- **Response:** remain on the QUIC carrier.
- **Initial measure:** throughput regression below 5%, median latency regression
  below 5%, protocol framing below 3% of payload bytes for 1 KiB and larger
  application writes.
- **Evidence:** reproducible benchmark manifest and raw results.

### Q-004 Bounded overload

- **Stimulus:** a peer opens flows without consuming responses.
- **Environment:** authenticated session at configured flow quota.
- **Response:** reject excess flows and preserve established flow correctness.
- **Initial measure:** memory remains within configured session budget plus 10%;
  established control-flow P99 latency remains below twice baseline.
- **Evidence:** load and fault test.

### Q-005 Pre-auth probe equivalence

- **Stimulus:** active probe matrix against enabled and disabled endpoints.
- **Environment:** identical cover application and infrastructure.
- **Response:** ordinary application behavior before authentication.
- **Initial measure:** zero deterministic differences in response bytes, public
  error codes, or connection-state transitions; timing distributions reviewed
  with retained samples.
- **Evidence:** differential probe report.

### Q-006 Maintainable change locality

- **Stimulus:** add one carrier implementation against the stable carrier API.
- **Environment:** repository architecture checks enabled.
- **Response:** no edits to protocol frame parsing or existing carriers.
- **Initial measure:** change touches the carrier API only if a missing generic
  capability is demonstrated; dependency cycles remain zero.
- **Evidence:** architecture-contract check and merge-base diff.

## Deterministic Gates

| Gate | Planned command | Evidence |
|---|---|---|
| Format and lint | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings` | CI logs |
| Unit and integration tests | `cargo test --workspace --all-targets` | CI logs and test report |
| Dependency boundaries | `cargo xtask architecture` | Contract report against `docs/architecture-contract.yaml` |
| Protocol state model | `cargo xtask model-check` | Retained state-space report |
| Parser fuzz smoke test | `cargo xtask fuzz-smoke` | Corpus and crash artifacts |
| Wire compatibility | `cargo xtask wire-compat` | Versioned vectors and diff report |
| Differential probes | `cargo xtask probe-diff` | Enabled/disabled endpoint comparison |
| Network fault matrix | `cargo xtask netem` | Scenario traces and checksums |

Commands are planned interfaces until the corresponding roadmap stage creates
them. CI must not advertise a gate before the command exists and is blocking.

## Budgets and Trends

- Dependency cycles, forbidden dependencies, unowned modules, and unversioned
  breaking changes: zero.
- Unsafe Rust: denied in protocol, session, policy, forest, and crypto modules;
  isolated and reviewed where an OS adapter demonstrates need.
- Fuzz corpus crashes: zero unresolved.
- Local deterministic test P95: target below 120 seconds after baseline exists.
- Required CI P95: target below 15 minutes after baseline exists.
- Track transition success rate, false transition rate, recovery latency, idle
  bytes, memory per session, battery wakeups, and probe classifier confidence.

## Release Claim Policy

Every published claim names the version, hardware, network conditions,
workload, baseline, sample count, and raw evidence. Terms such as "fast",
"stealth", "undetectable", and "censorship proof" are prohibited without a
precise metric; "undetectable" and "censorship proof" are not acceptable release
claims even with a limited experiment.

