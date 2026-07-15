# ADR-0019: Profile And Routing Evolution

- **Status:** Accepted
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Stakeholders:** Protocol, security, client, Android, desktop, and release maintainers
- **Supersedes:** ADR-0017

## Context

ADR-0017 introduced local proxy rules and editable adapter values, but Flutter
owned their text format, a runtime owned only one relay node, and Android TUN
could not apply domain-aware or direct rules. Velum needs one portable profile
and one routing meaning across proxy and TUN adapters without claiming support
for the much broader Mihomo configuration surface.

## Decision

Velum owns a versioned YAML profile parsed, validated, normalized, and persisted
by a native profile module. Version 1 contains profile identity, a default node,
multiple node descriptors, traffic adapter settings, and ordered routing rules.
Unknown fields or versions fail closed. Imports are previewed and completely
validated before an atomic commit; the application then owns the imported copy
and does not watch the source file.

Nodes have stable unique IDs and unique aliases. A rule may name either during
import, but persisted and exported profiles normalize references to stable IDs.
Deleting or renaming a referenced node is rejected until its rules are updated.
Profiles store only opaque `secret://` references; inline credentials and CA
material are invalid, and exports remain secret-free.

Version 1 matchers are exact domain, domain suffix, IPv4/IPv6 CIDR,
destination port or inclusive port range, and final `MATCH`. First match wins
and `MATCH`, when present, must be last. Actions are `DIRECT`, `REJECT`,
`PROXY` through the default node, and `NODE` through an explicit node ID.
Unsupported geodata, process, provider, subscription, and policy-group fields
are rejected rather than approximated.

A node pool composes existing single-connection runtimes. It eagerly connects
the default node and lazily connects explicitly selected nodes. Failure is
isolated to flows selecting that node. Changing profile generation invalidates
old flows and fake-IP DNS mappings.

The shared routing policy is evaluated for desktop proxy and all TUN hosts.
TUN DNS keeps bounded, TTL-governed fake-IP mappings so domain rules survive
packet capture; literal IP and encrypted DNS traffic without an observed name
use only CIDR, port, and `MATCH`. Direct sockets must use platform protection to
avoid TUN recursion. Rejection produces protocol-appropriate TCP reset or UDP
unreachable behavior.

Native ABI v3 exposes profile preview, atomic activation, export, node CRUD,
and engine lifecycle using append-only status and error categories. ABI v2
stays available until the Flutter migration and dynamic-library compatibility
tests are complete.

## Consequences

Routing meaning and node references become native, deterministic, and shared
across platforms. Fake-IP DNS and multiple runtime connections add bounded
state that must be cleared on generation changes. This profile is intentionally
Velum-specific and is not a Clash or Mihomo compatibility promise.

## Fitness Functions

- Golden profile tests cover normalization, round trips, duplicate identities,
  dangling node references, secret references, unknown fields, and atomic
  import failure.
- Policy tests cover every matcher and action, dual-stack CIDR, port boundaries,
  first match, final `MATCH`, node failure isolation, and generation invalidation.
- TUN tests cover fake-IP TTL, direct-loop prevention, dual-stack TCP/UDP/DNS,
  and protocol-specific rejection.
- ABI tests load v2 and v3 from the produced dynamic library during migration.

## Review Triggers

Introduce a new profile version before adding policy groups, geodata, process
rules, remote providers, subscriptions, or wire-incompatible matcher semantics.
Review the decision when ABI v2 removal is proposed or fake-IP DNS constraints
change.
