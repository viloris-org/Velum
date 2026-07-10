# Stage 1 Session Tracer

Status: Experimental. This deterministic model establishes delivery-state
ownership before any network transport, cryptographic attachment, or frame
encoding is introduced.

The [`velum-session`](../crates/velum-session/src/lib.rs) crate is the only
writer of flow sequence allocation, acknowledgement state, and receive
delivery cursors. Carrier implementations will report events; they must never
advance these cursors themselves.

## Reliable Flow Transition Table

| Input | Precondition | State change | Result |
|---|---|---|---|
| `open_reliable_flow` | Flow ID space remains | Allocate flow with send/receive sequence `0` | New `FlowId` |
| `send(bytes)` | Pending segment and byte limits permit it | Append sequence `next_send`; increment `next_send` | Segment for carrier delivery |
| `acknowledge(through)` | `through < next_send` | Remove pending segments through the cumulative cursor | Memory is released |
| `receive(sequence == next_receive)` | Flow exists | Increment `next_receive` | Deliver bytes exactly once |
| `receive(sequence < next_receive)` | Flow exists | None | Ignore duplicate |
| `receive(sequence > next_receive)` | Flow exists | None | Reject out-of-order input |

The receiver intentionally does not buffer out-of-order segments in this first
model. A later transition design may add a bounded reorder window only with a
specific memory limit and corresponding tests.

## In-Memory Carrier Simulator

`InMemoryCarrier` is a deterministic test-only carrier model. It can drop,
duplicate, delay, and reorder segments; toggling availability models a black
hole and subsequent recovery. It owns only scheduling: the session tracer
remains the sole owner of acknowledgement and delivery cursors.

The recovery scenario deliberately resends `SessionTracer::pending` segments
after a black hole or loss. This demonstrates that a carrier transition cannot
invent acknowledgement state or bypass exact-once receiver delivery.

## Verification

Run the focused deterministic state checks:

```bash
cargo xtask model-check
```

The suite covers duplicate-free in-order delivery, gap rejection, cumulative
acknowledgement, invalid acknowledgements, and bounded pending memory. It also
exhaustively checks every receive trace up to four events over a three-sequence
input alphabet, plus deterministic loss, duplication, reordering, delay,
black-hole, and recovery scenarios.
