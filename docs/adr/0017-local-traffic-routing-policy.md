# ADR-0017: Local Traffic Routing Policy

- **Status:** Accepted
- **Date:** 2026-07-15
- **Owner:** Client maintainers
- **Stakeholders:** Security, transport, Android, and release maintainers

## Context

The first platform adapters routed every accepted flow through Velum. They did
not provide a deterministic way to bypass private destinations or reject local
proxy requests. TUN address, DNS, route, MTU, proxy port, and proxy bypass
values were fixed in platform code, which prevented repeatable deployment
configuration.

FlClash demonstrates a broad Mihomo-compatible rule surface. Velum does not yet
own the geodata, process attribution, DNS interception, or rule-provider
lifecycle required to implement that entire surface safely.

## Decision

Desktop proxy routing has three modes: `rule`, `global`, and `direct`. Rule mode uses
an ordered list with strict `DOMAIN`, `DOMAIN-SUFFIX`, IPv4 `IP-CIDR`, and
`MATCH` matchers and `DIRECT`, `PROXY`, and `REJECT` actions. The first matching
rule wins. `MATCH`, when present, must be last. Unknown or malformed rules fail
configuration before an adapter is installed.

Flutter owns editable configuration and canonical text serialization. The
native ABI adds `velum_client_runtime_proxy_start_v2`, which copies and parses
the complete UTF-8 policy before binding a listener. The Rust proxy retains the
original domain through host DNS resolution, evaluates the domain and resolved
address together, and then either opens a local TCP connection, opens a
generation-bound Velum runtime stream, or returns an HTTP/SOCKS rejection.
The original proxy start entry remains an all-`PROXY` compatibility path.

System proxy options select the local listener port and platform bypass list.
Android TUN options select its IPv4 address and prefix, MTU, DNS servers, and
captured routes. Options are read at activation time and validated before the
platform mutation begins.

Routing rules currently execute only in the desktop HTTP CONNECT/SOCKS adapter.
Android TUN continues to proxy every packet captured by its configured routes.
Domain-aware TUN rules require an owned DNS strategy, and `DIRECT` TUN rules
require route exclusion or a native direct-flow path; neither is implied by
this decision.

## Consequences

- Desktop application traffic gets deterministic direct, proxied, and rejected
  behavior without moving destination data into Flutter.
- Host DNS resolution remains observable for domain proxy rules.
- Configuration is intentionally smaller than Mihomo and rejects unsupported
  rule kinds instead of silently changing their meaning.
- Android route selection can express split capture, but it is not presented as
  full rule-mode parity.

## Fitness Functions

- Dart rule parser and first-match tests cover canonicalization and invalid
  configuration.
- Rust proxy tests cover domain, suffix, CIDR, fallback ordering, direct relay,
  and protocol rejection.
- `cargo test -p velum-adapter-proxy -p velum-client-ffi`
- `flutter analyze` and `flutter test`
- `cargo xtask architecture` and `cargo xtask docs`

## Review Triggers

Revisit this decision before adding TUN rule parity, encrypted remote-name
resolution, IPv6 TUN routing, geodata rules, process rules, or rule providers.
