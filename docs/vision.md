# Vision and Positioning

## One-Sentence Position

Velum is an adaptive encrypted tunnel for restricted, unstable, and
heterogeneous networks that preserves logical sessions across carriers and
selects delivery behavior per flow.

## Problem

Today's encrypted tunnels tend to make a deployment-time tradeoff:

- QUIC/UDP designs can perform well on lossy, high-bandwidth-delay paths, but
  fail hard where UDP is blocked or aggressively rate-limited.
- TLS/TCP designs are widely deployable, but inherit head-of-line blocking and
  perform poorly when UDP traffic is forced through a reliable byte stream.
- Lightweight proxy protocols often delegate security, camouflage, and
  transport behavior to a large composition of external layers.
- Standard HTTP tunnels optimize interoperability, but do not standardize
  censorship response, proxy discovery, or new congestion-control behavior.

Users currently respond by maintaining multiple protocols and manually
switching configurations. The application connections inside those tunnels
usually do not survive the switch.

## Target Users

The initial target is a technically capable individual or small operator who:

- controls both client and server;
- encounters changing UDP availability, loss, jitter, or middlebox policy;
- needs both reliable streams and low-latency datagrams;
- values a small, observable deployment over broad legacy compatibility.

Enterprise policy networks, anonymous multi-hop systems, and mass-market VPN
products are later markets, not v1 requirements.

## Value Proposition

Velum should earn adoption through three capabilities, in this order:

1. **Continuity:** preserve eligible logical flows when a carrier fails.
2. **Intent-aware delivery:** schedule reliable streams, messages, and
   datagrams according to their semantics instead of forcing all traffic into
   one reliability model.
3. **Forest Native coexistence:** make the endpoint and transport behavior part
   of a real, common application ecosystem rather than a fixed imitation.

Rust is an implementation choice, not a protocol differentiator. It should
produce measurable benefits: memory-safe parsing, predictable scheduling,
fuzzable state machines, low idle cost, and portable single binaries.

## Positioning Boundaries

Velum is not "a faster MASQUE". MASQUE is the standards and interoperability
baseline from which Velum should borrow where appropriate.

Velum is not "another obfuscation protocol". Detectability is a system property
covering endpoint behavior, deployment population, handshake, traffic shape,
and long-lived flow behavior.

Velum is not "a new encryption algorithm". Carrier security must use reviewed
TLS 1.3 or QUIC cryptography. End-to-end protocol security will be specified
separately and reviewed before any production claim.

## Ranked Architecture Drivers

1. **Security and honest claims:** no custom cryptography; explicit threat
   boundaries; no claim of undetectability.
2. **Session continuity:** carrier loss should not automatically become
   application failure.
3. **Deployability:** useful operation on both UDP-friendly and TCP-only paths.
4. **Maintainability:** protocol layers and state ownership must remain
   independently testable.
5. **Performance:** low overhead and good degraded-path behavior, measured
   against relevant baselines rather than asserted.

When drivers conflict, security wins over camouflage, continuity wins over peak
benchmark throughput, and maintainability wins over speculative features.

## Goals

- Multiplex logical flows within a versioned session.
- Support QUIC/UDP and TLS/TCP carriers behind one carrier contract.
- Detect carrier degradation and migrate eligible flows under explicit policy.
- Support reliable byte streams and unreliable datagrams in the first usable
  protocol version.
- Expose enough telemetry to explain every carrier transition.
- Coexist with a real HTTPS/HTTP/3 service on the public endpoint.

## Non-Goals for Version 1

- Perfect traffic-analysis resistance.
- Multipath striping of one flow across carriers.
- Multi-hop anonymity.
- A public relay marketplace or automatic proxy discovery.
- New congestion-control algorithms.
- Layer-2 Ethernet tunneling.
- Compatibility with every proxy client's configuration format.

## Product Hypothesis

Operators will choose Velum over maintaining separate QUIC and TLS tunnels if
Velum can preserve real application sessions during UDP failure without making
normal-path latency or throughput materially worse.

This hypothesis is invalidated if carrier migration cannot preserve common TCP
sessions, takes longer than application reconnect, or requires enough cover
traffic and infrastructure to make deployment impractical.

