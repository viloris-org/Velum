# ADR-0021: Native Client Capability And UI Evolution

- **Status:** Proposed
- **Date:** 2026-07-16
- **Owner:** Client maintainers
- **Stakeholders:** Client, security, Android, desktop, and release maintainers
- **Related:** ADR-0013, ADR-0016, ADR-0019, and ADR-0020

## Context And Evidence

Velum has an experimental Flutter controller and Rust implementations of
profile validation, routing, runtime lifecycle, proxy integration, and parts
of TUN handling. The user experience still exposes a connection form rather
than a coherent general-purpose client workflow. A mature proxy client can
inform its information architecture, but importing its code or configuration
surface would introduce an incompatible data plane, protocol semantics, and
license obligations.

ADR-0019 defines the first Velum-native profile, node, and routing model. Its
`velum-client-engine` ownership boundary is the missing implementation layer
between a normalized profile and independently managed node runtimes.

## Decision Drivers

1. Rust remains the sole authority for connection, routing, DNS, and traffic
   lifecycle behavior.
2. A UI setting is actionable only when it maps to a versioned native contract
   and has a defined owner, validation, and rollback behavior.
3. The client must evolve in end-to-end slices without creating a Mihomo or
   Clash configuration compatibility promise.
4. Credentials, packet payloads, and raw destinations remain below Flutter.
5. The UI should be navigable on desktop and mobile without exposing unavailable
   platform features as editable controls.

## Options Considered

| Option | Decision |
|---|---|
| Port a mature proxy client and its core | Rejected: it replaces the Velum data plane and conflicts with Velum protocol and licensing boundaries. |
| Rebuild the Flutter UI before native capability exists | Rejected: creates inert controls, duplicate policy, and misleading support claims. |
| Deliver native capabilities first, then a capability-driven Flutter information architecture | Selected: preserves the Rust security boundary and gives each UI feature an executable contract. |

## Decision

`velum-client-engine` will own normalized profile generations, the bounded node
runtime pool, default-node preconnection, lazy explicit-node connection, and
node-local failure isolation. It consumes an engine input projected by the FFI
or platform host; it must not depend on `velum-client-profile` or parse YAML.

`velum-client-profile` continues to own profile schema, normalization,
import/export, secret references, and node identity. `velum-client-routing`
continues to own deterministic rule evaluation. Platform adapters own only
operating-system traffic capture and restoration. Flutter remains a
control-plane consumer of native snapshots and commands.

The Flutter application will organize supported controls into these pages:

| Page | Native source of truth | Initial scope |
|---|---|---|
| Overview | Runtime and adapter snapshots | lifecycle, selected node, traffic mode, redacted counters |
| Nodes and profiles | Profile plus engine snapshots | import/export, enrollment, node identity, selection, node status |
| Traffic | Platform adapter contracts | desktop proxy, Android VPN/TUN, configured bypass and routing policy |
| Settings | Application and platform host configuration | theme, language, startup behavior, diagnostics, permissions, and about |

Each page shows only supported capabilities for its current platform and native
feature set. DNS behavior, policy groups, remote providers, subscriptions,
geodata, process rules, raw external controllers, and Clash-specific TUN
options remain absent until a Velum-owned contract, security model, and tests
are accepted. FlClash may be used as an interaction and navigation reference;
its source code, assets, wording, and configuration schema are not imported.

## Consequences

The first visible UI changes wait for the engine tracer slice rather than
presenting unavailable settings. This delays superficial parity but prevents
Flutter from becoming a second routing or lifecycle implementation. It also
keeps profile and enrollment compatibility under Velum's versioned contracts.

## Fitness Functions

1. Engine tests prove default preconnection, lazy explicit-node connection,
   node-local failure isolation, and profile-generation invalidation.
2. Profile and routing tests continue to prove normalized node references and
   identical policy meaning independently of UI state.
3. Flutter integration tests assert that each visible traffic or node control
   consumes a native capability snapshot and that unsupported controls are not
   rendered.
4. `cargo xtask architecture`, `cargo xtask client-test`, `cargo fmt --all
   --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and
   `cargo xtask docs` remain blocking.

## Evolution And Rollback

1. Add the engine tracer slice with no UI change: import a profile with two
   nodes, preconnect the default node, and lazily connect an explicit node.
2. Surface read-only profile and node engine snapshots in the Nodes page.
3. Move profile and routing edits into dedicated pages after their native
   commands are versioned and tested.
4. Move adapter-specific settings into the Traffic page only after platform
   restoration and lifecycle evidence exists.

Each slice can roll back by hiding its Flutter entry point and retaining the
previous native profile/runtime contract. A visible control cannot ship before
its native command and test evidence; no long-lived UI-only compatibility layer
is allowed.

## Review Triggers

Revisit this decision when a proposed setting requires Flutter to evaluate
routing, retain credentials, inspect packets, or infer transport health; when
a new supported platform changes the page model; or when adding a configuration
feature outside ADR-0019's versioned schema.
