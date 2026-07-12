# TLS Fallback Evolution

Status: exploratory and deferred. This document records a direction for a
future TLS fallback implementation. It does not define Velum wire behavior,
enable a protocol feature, or create interoperability commitments.

## Current Boundary

The current TLS carrier maps one TLS 1.3/TCP connection to one reliable byte
stream. It rejects datagrams and does not multiplex logical flows. QUIC is the
primary path and uses its native independent bidirectional streams. Session
state already gives reliable segments a carrier-independent `FlowId` and
`Epoch`, but that model is not yet connected to a TLS inner multiplexer.

This boundary is intentional. A TLS byte stream cannot provide QUIC-like
independent loss recovery: one lost TCP segment delays every multiplexed flow.
TLS fallback must remain honestly stream-only.

## Tunnel-Behavior Boundary

Avoiding a recognisable tunnel is a broader deployment and observable-behavior
problem than hiding application-frame lengths. A TLS fallback can still be
distinguishable through its handshake and certificate configuration, endpoint
reputation, connection lifetime, idle behavior, reconnect rate, direction and
volume ratio, burst timing, response semantics, and behavior under active
probes. Application-level padding does not correct those signals.

Any future fallback deployment must therefore start with a separately operated
real cover service. Traffic that does not present a cryptographically verified,
replay-bounded Velum attachment preface must be handed to that service with
ordinary HTTP/2 or reverse-proxy behavior. It must not receive a Velum-specific
error, close pattern, timing distinction, synthetic response, or a
configuration-only imitation of a website. The cover service owns its content,
TLS policy, request handling, error behavior, timeouts, rate limits, and
operational telemetry; the TLS carrier does not emulate those concerns.

This is an operational direction, not a carrier-protocol requirement. It does
not promise indistinguishability, permit certificate-verification bypass, or
allow an endpoint to impersonate an unrelated service. The reference deployment
must use standard TLS 1.3, normal certificate validation, and an endpoint that
the operator is authorized to serve.

The attachment gate must be narrowly scoped: it decides whether a connection is
accepted by the Velum session layer or passed through to the cover service. It
must not introduce static-password first-write authentication, own reliable
delivery state, or turn the TLS carrier into a session multiplexer. Failed,
expired, malformed, or replayed attachment attempts follow the same cover
service path unless an explicit resource-protection policy rejects them before
the TLS connection is established.

## Application-Layer Padding Assessment

An inner protocol can place length-delimited waste frames around real frames
inside TLS. A per-session plan for early writes may use rough `web`, `api`, or
random size distributions, then add intermittent later waste. Its purpose is
to reduce a stable application-frame length signature visible through encrypted
TLS records.

This is not a TLS security improvement or a tunnel-avoidance mechanism. It does
not hide the TLS ClientHello, SNI, certificate, endpoint reputation, long-lived
timing behavior, connection lifecycle, downstream shape, active probes, or TCP
head-of-line blocking. Fixed early-write recipes can also become a stable
protocol fingerprint, while waste raises bandwidth, latency, and queueing
costs.

Velum must not adopt an unmeasured frame format, static-password
authentication, or `web`/`api` recipe. Such padding is coupled to an inner
multiplexer, whereas Velum keeps carrier transport separate from session
correctness.

## Candidate Shaping Direction

Any future observable-write profile is owned by `velum-forest`; the TLS
carrier may execute a verified bounded profile but must not own flow identity,
authentication, acknowledgement, or delivery order.

A candidate profile may specify:

- a version, expiry, authenticated digest, and explicit activation point;
- bounded size buckets for initial and later writes, rather than fixed packet
  recipes;
- a per-attachment cover-byte budget and bounded optional delay budget;
- stop conditions for congestion, low-latency traffic, excessive queueing, or
  exhaustion of the profile budget; and
- failure behavior that disables shaping or rejects the attachment without
  changing reliable-stream semantics.

The default remains no application-level padding until retained measurements
justify a profile. A credible real cover service and normal endpoint behavior
are prerequisites, not optional enhancements. Velum must not introduce TLS 1.2
fallback, certificate-verification bypass, synthetic ClientHello behavior, or
arbitrary ALPN offers for this feature.

Profiles must be derived from retained measurements of the authorized cover
service and its intended workload, not from generic `web` or `api` labels.
They must not force a connection lifetime, request cadence, response shape, or
reconnect pattern that conflicts with the cover service merely to conceal an
inner frame length.

## TLS Multiplexing Decision

TLS inner multiplexing is deferred. It is useful only when TLS fallback
measurements show that sharing a warm TLS connection improves a real workload:
many concurrent short reliable flows, expensive repeated handshakes, or an
unacceptable socket and CPU budget. It is not required for the QUIC primary
path, which already multiplexes natively.

Before implementing it, retain evidence that at least one of these conditions
holds in the reference workload:

- TLS fallback concurrent-flow P95 is greater than one; or
- repeated TLS connections breach an approved P95 first-useful-byte, CPU, or
  socket budget.

If the gate is met, the multiplexer must be session-owned. It needs authenticated
open/data/close controls keyed by `FlowId` and `Epoch`, bounded per-flow and
connection queues, fair scheduling, slow-consumer handling, explicit global
backpressure, quotas, and bounded recovery. The TLS carrier remains a byte
transport and must not absorb that state. A disconnect affects all inner flows;
the session's acknowledgement cursor remains the only authority for reliable
recovery.

## Evidence Gate

No shaping or TLS inner multiplexing becomes enabled by default without a
retained comparison against unshaped TLS fallback, the real cover service, and
the QUIC primary path. The evaluation must include both traffic directions,
first-useful-byte latency, tail latency, cover-byte overhead, CPU, memory,
socket count, queue depth, disconnect rate, packet-size and timing samples,
connection lifetime and reconnect samples, cover-service response semantics,
and differential active-probe results. A candidate that improves an early
packet distribution but regresses cover-service behavior, reliability, or the
approved budgets is rejected.
