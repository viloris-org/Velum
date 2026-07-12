# Velum Protocol Version 0 Draft

Status: Draft. This document defines the v0 frame grammar and state rules that
an implementation MUST follow before claiming wire compatibility. It is not a
production security claim, does not define initial credentials, and does not
provide cross-process session recovery.

The draft serializes the ownership model in [the architecture](architecture.md)
and the reliable-delivery behavior in [the session tracer](session-tracer.md).
It is deliberately carrier-independent: QUIC and TLS carry the same logical
frames, while their transport capabilities remain explicit.

## Conventions And Scope

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, **SHOULD NOT**,
and **MAY** are to be interpreted as described by BCP 14 when, and only when,
they appear in all capitals.

An *initiator* proposes a carrier attachment. An *acceptor* owns the target
logical session and accepts or rejects that attachment. A *carrier* is a
transport instance, not a session. All v0 frames MUST run inside an encrypted,
ordered, reliable carrier byte stream. Carrier encryption does not replace
logical-session authentication.

This document starts after an external authentication mechanism has supplied a
session-specific attachment secret to both peers. It does not define session
creation, credential provisioning, key rotation, cover-service routing, or
datagram application payloads.

## Canonical Encoding

An octet is an 8-bit byte. All integer values are unsigned and fixed-width in
big-endian order. `bytes[N]` is exactly `N` octets and has no prefix. A sender
MUST emit the sole representation defined here; a receiver MUST reject a
truncated field, nonzero reserved value, invalid fixed length, or trailing
payload bytes.

| Name | Encoding |
|---|---|
| `u8` | One octet |
| `u16` | Two octets, big-endian |
| `u64` | Eight octets, big-endian |
| `session_id` | `bytes[16]` |
| `attachment_id` | Nonzero `bytes[16]` |
| `nonce` | Nonzero `bytes[16]` |
| `flow_id`, `epoch`, `segment_sequence` | `u64` |
| `capabilities` | `u64` bitset |
| `attachment_proof` | `bytes[32]` |
| `error_code` | `u16` |

### Frame Envelope

Every frame is a four-octet header followed by its declared payload:

```text
+----------------+----------------------+------------------------+
| frame_type: u8 | reserved: u8         | payload_length: u16    |
+----------------+----------------------+------------------------+
| payload: bytes[payload_length]                                  |
+--------------------------------------------------------------+
```

`reserved` MUST be zero. `payload_length` MUST be at most `65535`, the largest
value representable by `u16`. A receiver MAY configure a smaller maximum, but
it MUST reject a larger declared payload before allocating or buffering it. An
incremental decoder MUST retain no more than one declared frame plus its
four-octet header.

Frame types `0x00` through `0x7f` are mandatory. An unknown mandatory frame
MUST close the affected attachment with `PROTOCOL_VIOLATION` once it is
authenticated. Frame types `0x80` through `0xff` are optional extensions. A
receiver MAY skip an unknown optional frame only after validating its bounded
envelope; it MUST NOT reinterpret it as a known frame.

The assigned v0 frame types are:

| Type | Name | Payload length |
|---|---|---|
| `0x01` | `NEGOTIATION_OFFER` | 36 |
| `0x02` | `NEGOTIATION_ACCEPT` | 42 |
| `0x03` | `ATTACH` | 114 |
| `0x04` | `ATTACH_ACCEPTED` | 40 |
| `0x10` | `STREAM_OPEN` | 8 |
| `0x11` | `STREAM_DATA` | 25 through 65535 |
| `0x12` | `STREAM_ACK` | 24 |
| `0x13` | `STREAM_FINISH` | 24 |
| `0x14` | `STREAM_RESET` | 10 |
| `0x20` | `CONNECTION_CLOSE` | 2 |

All unassigned mandatory types are reserved. A known frame with a payload
length other than the one defined here is malformed. `STREAM_DATA` is variable
only because its final data field consumes the remaining nonempty payload.

## Version And Capability Negotiation

Before an attachment, the initiator sends exactly one `NEGOTIATION_OFFER`. The
acceptor either closes or sends exactly one `NEGOTIATION_ACCEPT`. An acceptor
MUST NOT send a protocol-specific reject before logical authentication
succeeds; pre-auth failures follow the cover-service behavior of the
deployment.

### Capabilities

`capabilities` is a big-endian `u64` bitset. Bit numbering starts at the least
significant bit.

| Bit | Capability | Meaning |
|---|---|---|
| 0 | `RELIABLE_STREAM` | Ordered, duplicate-free logical stream segments. |
| 1 | `UNRELIABLE_DATAGRAM` | Native unreliable datagrams are available on this carrier. |

Unknown required bits fail negotiation. An implementation MAY advertise an
unknown optional bit, but a peer that does not understand it MUST leave it
disabled. A TLS carrier MUST NOT enable `UNRELIABLE_DATAGRAM` merely because it
can carry reliable bytes.

### `NEGOTIATION_OFFER`

The body is exactly:

```text
+----------------+----------------+---------------------+---------------------+-------------------+
| minimum: u16   | maximum: u16   | required: u64       | optional: u64       | client_nonce[16]  |
+----------------+----------------+---------------------+---------------------+-------------------+
```

`minimum` MUST be no greater than `maximum`. `required` and `optional` MUST
not share a set bit. A valid v0 offer MUST require `RELIABLE_STREAM`.
`client_nonce` MUST be nonzero and freshly generated for the offer. The
initiator MUST retain the offer until the attachment completes or fails.

### `NEGOTIATION_ACCEPT`

The body is exactly the canonical negotiated-parameters value:

```text
+----------------------+---------------------+-------------------+-------------------+
| selected_version: u16 | enabled: u64        | client_nonce[16]  | server_nonce[16]  |
+----------------------+---------------------+-------------------+-------------------+
```

The acceptor MUST select the newest common version: it chooses
`min(local.maximum, peer.maximum)` only when that value is at least
`max(local.minimum, peer.minimum)`. `enabled` MUST be a subset of the offered
bits, MUST include every required bit, and MUST include `RELIABLE_STREAM`.
`client_nonce` MUST exactly echo the offer. `server_nonce` MUST be nonzero and
freshly generated for this acceptance.

The initiator MUST reject an acceptance whose version is outside its offered
range, whose enabled bits are not an allowed selection, whose client nonce does
not match, or whose server nonce is zero. The exact 42-octet representation
above is the canonical negotiated-parameters value used by attachment proofs.

## Carrier Attachment

An `attachment_id` is a protocol identifier, not a locally assigned carrier
ID. The initiator MUST generate a fresh, nonzero, opaque 16-octet
`attachment_id` for each new attachment attempt. It MAY retransmit the exact
same pending `ATTACH` while awaiting a response, but it MUST use a new value
for a new attempt. An acceptor MUST NOT use a transport-local connection ID as
an `attachment_id`.

The session attachment secret is external to this draft. Given that secret,
the attachment proof uses HMAC-SHA-256 with these ASCII labels, without a
trailing NUL:

```text
attachment_key = HMAC-SHA-256(
    session_attachment_secret,
    "velum v0 attachment key"
)

attachment_proof = HMAC-SHA-256(
    attachment_key,
    "velum v0 carrier attachment" || session_id || epoch || attachment_id || negotiated_parameters
)
```

`negotiated_parameters` is the exact 42-octet canonical value from
`NEGOTIATION_ACCEPT`, not a re-encoded semantic equivalent. The proof binds
the logical session, current epoch, fresh attachment identifier, selected
version, selected capabilities, and both negotiation nonces. Implementations
MUST use a constant-time HMAC verification supplied by their cryptographic
library.

### `ATTACH`

The body is exactly:

```text
+----------------+-------------+-------------------+--------------------------+----------------------+
| session_id[16] | epoch: u64  | attachment_id[16] | negotiated_parameters[42] | attachment_proof[32] |
+----------------+-------------+-------------------+--------------------------+----------------------+
```

The acceptor MUST verify all of the following before binding a carrier:

1. The session exists in its current in-memory state.
2. The negotiated parameters validate against the outstanding offer and local
   capability set.
3. The stated `epoch` is the session's current epoch, not a retiring, stale, or
   future epoch.
4. The `attachment_id` is nonzero and fresh within this logical session's
   attachment attempts.
5. The proof validates for the exact session, attachment ID, epoch, and
   canonical negotiated parameters.
6. Either the session replay window has not accepted an attachment for this
   epoch, or this is an exact retransmission of the already accepted
   `(epoch, attachment_id)` pair.

The acceptor MUST verify the proof before consuming the epoch in its replay
window. A failed proof, stale/future epoch, or replay MUST leave delivery
cursors and replay state unchanged. It MUST respond to an exact retransmission
on the same locally tracked carrier with the same `ATTACH_ACCEPTED`; a different
attachment or a different local carrier for that epoch is a replay. One current
epoch can accept at most one attachment identity. A new carrier attachment is
never authenticated by a transport carrier ID.

### `ATTACH_ACCEPTED`

On success, the acceptor sends this exact body:

```text
+----------------+-------------+-------------------+
| session_id[16] | epoch: u64  | attachment_id[16] |
+----------------+-------------+-------------------+
```

The initiator MUST match every field against its pending `ATTACH` before
marking the carrier attached. The acknowledgement carries no flow cursor,
resume token, server proof, or transport-local carrier identifier.

After `ATTACH_ACCEPTED`, the same carrier MUST NOT receive another negotiation
or a different attachment exchange. The acceptor MAY receive and acknowledge
an exact `ATTACH` retransmission until the initiator receives its
`ATTACH_ACCEPTED`. Any other unexpected known frame for the current state is a
`PROTOCOL_VIOLATION`.

## Reliable Stream Frames

Flow IDs are unique within a logical session and MUST NOT be reused. Each
`STREAM_OPEN` ID MUST be greater than every previously opened ID in that
in-memory session; the all-ones `u64` value is unavailable so the next ID
remains representable. Segment sequence values are logical per-flow segment
numbers, not byte offsets. The first sent segment uses sequence `0`; each
`STREAM_DATA` increments the sender's next sequence by one, regardless of its
byte length.

### `STREAM_OPEN`

```text
+----------------+
| flow_id: u64   |
+----------------+
```

`STREAM_OPEN` creates a reliable flow only on an attached carrier and only if
the configured flow budget permits it. Reusing an existing or closed flow ID
is `FLOW_STATE`.

The sender MUST enforce pending-segment and pending-byte budgets both per flow
and per session before retaining a segment for retransmission. This prevents a
peer from multiplying memory use by opening many individually valid flows.

### `STREAM_DATA`

```text
+----------------+-------------+-----------------------+-------------------+
| flow_id: u64   | epoch: u64  | segment_sequence: u64 | bytes[remaining] |
+----------------+-------------+-----------------------+-------------------+
```

`bytes` MUST be nonempty. A zero-length `STREAM_DATA` is malformed and MUST
not consume a segment sequence. The receiver delivers bytes only when
`segment_sequence` equals its next expected sequence. A lower sequence is a
duplicate and MUST NOT be delivered again; a higher sequence is out of order
and MUST NOT advance the receive cursor. A v0 receiver does not buffer an
out-of-order segment.

### `STREAM_ACK`

```text
+----------------+-------------+----------------+
| flow_id: u64   | epoch: u64  | through: u64   |
+----------------+-------------+----------------+
```

`through` is an inclusive cumulative acknowledgement: it confirms every
segment sequence less than or equal to `through`. A sender releases only those
pending segments. An acknowledgement at or beyond the next unsent sequence,
or from a retiring epoch that would acknowledge a current-epoch segment, is
`FLOW_STATE`.

### `STREAM_FINISH`

```text
+----------------+-------------+--------------------------+
| flow_id: u64   | epoch: u64  | final_next_sequence: u64 |
+----------------+-------------+--------------------------+
```

`final_next_sequence` is exclusive. It MUST equal the sender's next segment
sequence at the time of finish. Thus an empty flow finishes with `0`; when the
last data segment has sequence `n`, the finish value is `n + 1`. A receiver
MUST not accept or deliver a segment with a sequence at or above the final
exclusive value. It considers the receiving direction finished only after all
sequences in `[0, final_next_sequence)` have been delivered exactly once.

### `STREAM_RESET`

```text
+----------------+------------------+
| flow_id: u64   | error_code: u16   |
+----------------+------------------+
```

`STREAM_RESET` terminates the affected flow. It MUST NOT imply that any
unacknowledged reliable data was delivered. Its error code is drawn from the
registry below.

### Epoch Window And Migration

`STREAM_DATA`, `STREAM_ACK`, and `STREAM_FINISH` MUST carry either the current
epoch or the single retiring epoch. A session advances to a new current epoch
before attaching a replacement carrier and retains exactly one previous epoch
for in-flight data and acknowledgements. It MUST reject a second concurrent
transition. A retiring epoch may carry in-flight frames but MUST NOT accept a
new attachment.

After a successful new attachment, a sender MAY reissue retained,
unacknowledged data under the current epoch with the same flow ID and segment
sequence. The receiver's cursor suppresses a duplicate. This rule does not
apply to datagrams and does not permit delivery cursor reconstruction after
state loss.

## Connection Close And Errors

`CONNECTION_CLOSE` is exactly:

```text
+------------------+
| error_code: u16   |
+------------------+
```

After logical attachment, a peer MAY send one `CONNECTION_CLOSE` before
closing the carrier. It MUST NOT respond to a close with another close. Errors
MUST contain no free-form text, credential, session secret, destination,
address, or payload bytes.

| Code | Name | Meaning |
|---|---|---|
| `0x0001` | `NO_SHARED_VERSION` | Negotiation has no shared version. |
| `0x0002` | `INCOMPATIBLE_CAPABILITIES` | A required capability is unavailable or selection is invalid. |
| `0x0003` | `AUTHENTICATION_FAILED` | Attachment proof or negotiated binding did not verify. |
| `0x0004` | `REPLAY_DETECTED` | An attachment or protected operation was replayed. |
| `0x0005` | `PROTOCOL_VIOLATION` | Frame grammar, ordering, or mandatory-extension rule failed. |
| `0x0006` | `RESOURCE_LIMIT` | A configured bounded resource limit prevented progress. |
| `0x0007` | `FLOW_NOT_FOUND` | The referenced flow does not exist. |
| `0x0008` | `FLOW_STATE` | The operation conflicts with flow lifecycle or cursor state. |
| `0x0009` | `RESUME_UNSUPPORTED` | Required in-memory state is unavailable. |
| `0x00ff` | `INTERNAL` | A local non-secret failure prevented progress. |

All other codes are reserved. An implementation that receives an unknown code
MUST treat it as `PROTOCOL_VIOLATION` for connection handling and MUST NOT
surface it as a user-visible diagnostic by default.

Before logical attachment, these codes are a local or redacted telemetry
vocabulary only. The acceptor MUST NOT send `CONNECTION_CLOSE` or another
distinguishing Velum response for malformed negotiation, unknown sessions,
invalid proofs, or an unsupported version. It MUST instead close or continue
the ordinary cover-service behavior configured for that endpoint.

## State Loss And Recovery Boundary

Version 0 supports carrier transition only while the relevant session state,
flow cursors, pending reliable segments, and replay window remain in memory.
It defines no durable resume token, replicated state, persistent cursor,
server-restart handoff, or cross-process recovery mechanism.

After restart, eviction, or unrecoverable process failure, a peer MUST NOT
accept an old `ATTACH` and reconstruct the old session from its proof. It MUST
NOT reset an epoch, flow ID allocation, segment sequence, acknowledgement
cursor, final cursor, or replay window under the old `session_id` and call that
recovery. A missing session state produces the stable local condition
`RESUME_UNSUPPORTED`; before attachment it remains indistinguishable from any
other pre-auth failure.

The peer MUST terminate affected local flows rather than silently claim
continuity. It MAY create a newly authenticated session through the external
session-creation protocol, but that session MUST use a new `session_id` and
contains no recovered flows. An in-process carrier migration may resume only
the unacknowledged segments still retained by the existing session state.

## Implementation And Conformance Boundary

The draft deliberately maps to the v0 protocol types and codec: fixed-width
big-endian fields, newest-common-version selection, required-versus-optional
capability bitsets, nonce-bound negotiation parameters, bounded frame parsing,
and HMAC-SHA-256 attachment proofs. `AttachmentId` is a wire-level protocol
identity and MUST replace any attempt to serialize a local `CarrierId` in an
attachment. The tracer also enforces immutable negotiated parameters,
idempotent attachment acceptance on its tracked local carrier, and per-session
pending-data limits.

The in-memory session dispatcher maps `ATTACH`, `STREAM_*`, optional, and
close frames to the tracer. It gates every frame on the local carrier's
authenticated attachment, permits only in-flight frames on a retiring carrier,
and implements `OPEN`, `FINISH`, `RESET`, and `CONNECTION_CLOSE` state rules.
It remains deliberately separate from the existing QUIC listener, which is an
experimental control-record path and MUST NOT claim v0 wire compatibility.

Before advertising interoperability, the repository still needs canonical test
vectors, parser fuzzing, a second codec consumer or independent conformance
harness, requirement-to-test traceability, and independent security review.
Until those gates close, this document is a preview draft and the existing
in-memory tracer remains the verified source for delivery behavior.
