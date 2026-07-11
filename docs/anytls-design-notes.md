# AnyTLS Design Notes

Status: exploratory. This document records reusable design ideas from the
[AnyTLS protocol](https://github.com/anytls/anytls-go/blob/main/docs/protocol.md).
It does not define Velum wire behavior or create interoperability commitments.

## Worth Testing

### Authenticated profile rotation

AnyTLS lets a server replace the client's early-write padding scheme after
authentication. The useful idea is not the particular packet-length recipe;
it is operational agility. A detected or poorly fitting profile can be rotated
without a client release, while a new client has only a small bootstrap
exposure.

For Velum, a Forest profile update should be a versioned, authenticated data
object with a stable identifier and digest. The client acknowledges an accepted
update and applies it only to later carrier writes or later carrier attachments.
An update must be bounded by an expiry and must be safe to reject. It must not
change frame encoding, session authentication, flow identifiers, epochs,
acknowledgements, retransmission, or delivery order.

This preserves the existing ownership rule: Forest selects observable write
behavior; `velum-session` owns delivery correctness.

### Explicit readiness and liveness signals

AnyTLS v2 added a stream-open acknowledgement and a request/response heartbeat
to turn silent TCP failures into bounded, observable states. Velum has a
stronger need for this at the carrier-attachment boundary: a transition should
not wait for operating-system TCP timeouts to discover that an attachment is
unusable.

Candidate control semantics to evaluate after the wire format exists:

- an authenticated attachment-ready acknowledgement, emitted only after the
  peer has accepted the carrier, session, epoch, and negotiated version;
- bounded liveness probes with a correlation value and explicit timeout policy;
- no implicit retransmission of reliable bytes until session-owned
  acknowledgement state identifies the pending sequence range.

These controls require rate limits and a per-session budget. They are evidence
for placement and transition policy, never delivery truth by themselves.

### Conservative version activation

AnyTLS v2 remains compatible with a v1 peer by requiring confirmation before
using v2-only controls. Velum's existing `VersionRange` supplies the foundation.
Future optional features should likewise remain inactive until both peers have
explicitly selected a shared version and feature set. Absence, malformation, or
timeout of an optional feature must degrade only that feature or reject the
attachment according to a versioned policy; it must not silently alter stream
semantics.

## Not Reusable As-Is

- **TLS-layer multiplexing:** AnyTLS multiplexes proxy streams inside one
  TLS/TCP connection. Velum needs carrier-independent flow identity and
  migration, so session flow state cannot be tied to one carrier's byte stream.
- **Fixed early-packet recipes:** splitting and padding selected early writes
  is a useful experiment input, but is insufficient for Forest Native. It omits
  downstream behavior, timing, endpoint behavior, and deployment population.
- **Password-hash authentication:** Velum already separates authenticated
  carrier attachment from TLS and binds it to session and epoch. Replacing that
  with a static password hash would weaken the transition model.
- **UDP over TCP as equivalent UDP:** TLS fallback remains stream-only unless a
  separately negotiated mode states its loss, ordering, head-of-line, and MTU
  semantics honestly.

## Experiment Gate

Before protocol work begins, retain a comparison between a fixed bootstrap
profile and a rotated profile under the same cover service. The report must
include both directions' packet-size and timing samples, profile-update success
and rejection rates, update latency, cover bytes, CPU, and differential active
probe results. A profile rotation that improves one early uplink distribution
but makes endpoint behavior distinguishable is a failed result.
