# ADR-0013: Client Runtime And Platform Host Boundary

- **Status:** Proposed
- **Date:** 2026-07-13
- **Owner:** Client maintainers
- **Stakeholders:** Platform, transport, protocol, security, release, and UI maintainers
- **Related:** ADR-0012 and [`client-architecture.md`](../client-architecture.md)

## Context And Evidence

ADR-0012 introduced a reviewed direct Flutter FFI boundary to remove the local
CONNECT data-plane copy. The resulting Flutter application calls only connect
and close, owns no application or system traffic adapter, derives connection
state locally, and reads credential material before invoking Rust. It therefore
demonstrates a QUIC connection but is not a complete cross-platform client.

The repository needs a boundary that can own lifecycle and status consistently
across desktop and mobile hosts without putting OS APIs, secrets, packet I/O, or
transport behavior into the UI.

## Decision Drivers

1. Correct lifecycle and cancellation state has one write authority.
2. Credentials and packet data remain below presentation and diagnostics.
3. Platform-specific service or extension lifecycles do not enter protocol or
   carrier modules.
4. Module dependencies and compatibility paths are machine enforceable.
5. Migration remains reversible while Stage 2 evidence is incomplete.

## Options Considered

| Option | Decision |
|---|---|
| Continue adding state and features to Flutter | Rejected: presentation remains the lifecycle and secret boundary |
| Make synchronous Flutter FFI the permanent control and data boundary | Rejected as the target: it does not fit every background lifecycle and currently blocks native calls |
| Shared Rust runtime behind platform hosts and a control-plane UI | Selected: coherent lifecycle ownership with explicit platform integration |

## Decision

Create `velum-client-runtime` as the single owner of client lifecycle state,
operation generations, active direct-client ownership, and immutable runtime
snapshots. Its initial dependency surface is `velum-client-api` plus telemetry
when runtime events are introduced.

`velum-client-ffi` becomes a compatibility adapter over `client-runtime`; it no
longer owns direct QUIC client lifecycle. ABI v1 remains temporarily so the
first slice is reversible. It is owned by `client-maintainers` and may be
removed only when every shipped consumer uses the asynchronous lifecycle
contract and equivalent live-flow tests pass.

The asynchronous native lifecycle is an additive runtime ABI v1 rather than a
breaking change to synchronous ABI v1. It exposes create, start, latest-value
snapshot, stop, and destroy. Start returns only after the runtime has accepted
the command and published `Connecting`, without waiting for network
establishment. Stop aborts the runtime-owned connection task before ending
`Stopped` and joins it before returning. Snapshot crosses FFI as fixed-width
integer fields, not Rust enum layout.

The target deployment uses a platform host to own OS service/extension
lifecycle, permission integration, secure configuration references, and the
local authenticated control endpoint. Flutter is a control-plane consumer and
must not be the source of runtime connection truth. Platform traffic adapters
connect to a narrow runtime/session API; they do not import carrier internals.

This ADR does not select TUN, SOCKS, or another production traffic adapter.
That choice remains gated by Stage 0 operator and workload evidence.

## Invariants

- At most one active direct client belongs to a runtime instance.
- Every state change increases a monotonic snapshot revision.
- A completion from a superseded lifecycle generation cannot install a client
  or overwrite the current state.
- The runtime owns every connection task and aborts it during stop or drop; no
  native adapter may detach lifecycle work after its handle is destroyed.
- The accepted connection task remains as a generation-guarded closure watcher
  while online; remote QUIC closure retires the client and publishes `Failed`.
- Reliable send and receive halves are independently synchronized, and native
  stream invalidation cancels in-progress I/O.
- `Stopped` retains no active client; compatibility adapters invalidate flow
  handles derived from the previous client generation.
- UI-visible state and failures contain no credential, certificate, payload,
  raw token, or full-destination data.
- The UI never claims system traffic is routed solely because the runtime is
  `Online`.

## Consequences

- Lifecycle policy becomes testable independently of Flutter and OS adapters.
- Platform support requires native host, permission, packaging, and release
  evidence per platform; shared Flutter widgets alone are insufficient.
- The initial runtime wrapper adds one internal call boundary without changing
  the Stage 2 wire path.
- Two client lifecycle APIs coexist temporarily. The compatibility path has the
  owner and removal condition stated above and must not gain new features.
- Automatic reconnect and degraded carrier states remain deferred until live
  health and policy ownership are connected.

## Fitness Functions

1. `cargo xtask architecture` enforces runtime ownership and dependency
   direction, including `client-ffi -> client-runtime -> client-api`.
2. `cargo test -p velum-client-runtime -p velum-client-ffi` covers state
   transitions, stale operation completion, monotonic revisions, and ABI handle
   behavior.
3. `cargo clippy --workspace --all-targets -- -D warnings` and
   `cargo fmt --all --check` remain blocking.
4. `cargo xtask docs` validates the proposal, ADR, and index links.
5. A platform may be called supported only after retained install, live-flow,
   suspend/resume, network-change, failure cleanup, and uninstall evidence.

## Delivery And Rollback

The first tracer slice adds the runtime and routes the existing ABI through it
without changing ABI_VERSION. Rollback restores `client-ffi`'s direct
`client-api` dependency and removes the inactive runtime declaration.

The next slice introduces additive runtime ABI v1 with create, non-blocking
start, snapshot polling, stop, and destroy before any new UI feature relies on
lifecycle state. `velum_client_abi_version() == 1` and its synchronous symbols
remain unchanged during migration. The old synchronous path is deleted only
after its removal condition passes.

## Review And Invalidation Triggers

Review this decision when choosing the first production traffic adapter or
supported platform, when defining the platform control protocol, or if measured
platform limits make the shared Rust runtime infeasible. Supersede rather than
editing this ADR after acceptance.
