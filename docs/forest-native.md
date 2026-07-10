# Forest Native

## Principle

If a leaf must be hidden, it belongs in a forest. Velum interprets this as a
system constraint:

> A public Velum endpoint must be a valid participant in a common application
> ecosystem, not a dedicated tunnel with a painted protocol surface.

Forest Native is not a promise of undetectability. Its goal is to avoid stable,
cheap classification signals and to make unauthenticated behavior useful and
ordinary.

## What It Replaces

Traditional obfuscation often adds random bytes, changes a handshake marker,
or returns a plausible error page. Those techniques can still leave a unique
combination of:

- TLS or QUIC parameters;
- authentication timing and connection-close behavior;
- packet sizes, directions, and burst timing;
- connection lifetime and concurrency;
- DNS, certificate, and hosting characteristics;
- a public endpoint that has no genuine application users.

Forest Native treats the full observable system as the design surface.

## Threat Model

### In Scope

- Passive observers that record packet metadata and connection behavior.
- Active probes that speak TLS, HTTP/2, or HTTP/3 and vary requests.
- Replay of captured pre-authentication traffic.
- Middleboxes that block or rate-limit UDP, QUIC, uncommon ports, or protocol
  fingerprints.
- Statistical classifiers using size, direction, timing, volume, and lifetime.
- Correlation of DNS, certificate transparency, IP reputation, and endpoint
  behavior.

### Out of Scope for Version 1

- A global passive adversary correlating both ends of every connection.
- Endpoint compromise or extraction of client credentials.
- Traffic-analysis immunity without cover traffic.
- Domain-fronting availability.
- Hiding server IP addresses from the access network.
- Defeating every future classifier.

## Three Observable Layers

| Layer | Observer signal | Requirement |
|---|---|---|
| Handshake | TLS/QUIC fingerprint, ALPN, certificates, transport parameters | Use reviewed mainstream stacks and common configurations; no Velum marker before encryption |
| Endpoint behavior | HTTP responses, timing, error paths, connection closure | Serve a real application; authentication failure must not produce a stable protocol oracle |
| Session behavior | Sizes, directions, bursts, concurrency, duration | Profiles model distributions and both directions; fixed padding recipes are insufficient |

Passing one layer does not compensate for failing another.

## Design Rules

### Real service coexistence

- The public endpoint serves real content or reverse-proxies a real application.
- Cover routing is independently testable without Velum credentials.
- Disabling Velum leaves a valid service, not a dead camouflage shell.
- The service uses normal status codes and behavior for its application.

### No pre-authentication marker

- Do not use a unique ALPN, clear-text magic number, path, status code, or error.
- Authentication occurs inside an encrypted, syntactically valid application
  exchange.
- Invalid or absent credentials follow the cover application's behavior.
- Success and failure behavior must be checked for timing and connection-state
  oracles.

### Distributional profiles

- A profile describes bounded distributions for size, direction, burst, delay,
  and connection lifetime where feasible.
- Profiles are versioned data, not hard-coded protocol logic.
- A server can advertise a profile update only after authentication.
- Profile changes cannot alter frame meaning, authentication, replay rules, or
  delivery correctness.
- Every profile declares latency, bandwidth, CPU, and cover-byte budgets.

### Mainstream implementation behavior

- Prefer unmodified mainstream TLS and QUIC libraries.
- Treat custom handshake fingerprints as a measured exception requiring an ADR.
- Keep certificate and HTTP configuration operationally normal.
- Do not claim equivalence to a browser unless measured against that browser's
  implementation and behavior.

## Active-Probe Requirements

The probe suite must compare a Velum-enabled endpoint against the same cover
service with Velum disabled. It should vary:

- valid, malformed, truncated, replayed, and slow HTTP requests;
- TLS versions, ALPN offers, SNI values, and connection reuse;
- request paths, bodies, methods, headers, and stream concurrency;
- timing between handshake and application data;
- repeated requests from new and reused source addresses.

A release fails the gate if a pre-authentication response, close code, header,
body, or timing bucket deterministically distinguishes the enabled endpoint.
Statistical differences are tracked as a protected trend and require explicit
review; a single benchmark cannot prove indistinguishability.

## Operational Requirements

- Cover-service health is monitored independently from tunnel health.
- Operators must be able to rotate cover content and traffic profiles without a
  wire-version change.
- Profile updates are signed or delivered through the authenticated session.
- Cover traffic volume is separately metered so cost is visible.
- Telemetry must not collect the very browsing patterns Forest Native is meant
  to protect.

## Known Limits

A private endpoint with only tunnel traffic has a small anonymity set even when
its packets resemble HTTP/3. Generating synthetic cover traffic may increase
cost without producing a credible user population. Velum therefore documents
deployment quality and classification resistance separately; it does not turn
either into a boolean `obfs: true` claim.

