# Stage 1 Session Tracer

Status: Experimental. This deterministic model establishes delivery-state
ownership before any network transport, cryptographic attachment, or frame
encoding is introduced.

The [`velum-session`](../crates/velum-session/src/lib.rs) crate is the only
writer of flow sequence allocation, acknowledgement state, epoch validity, and
receive delivery cursors. Carrier implementations will report events; they
must never advance these cursors themselves.

## Reliable Flow Transition Table

| Input | Precondition | State change | Result |
|---|---|---|---|
| `open_reliable_flow` | Flow ID space remains | Allocate flow with send/receive sequence `0` | New `FlowId` |
| `send(bytes)` | Pending segment and byte limits permit it | Append sequence `next_send`; increment `next_send` | Segment bearing its `FlowId` and current `Epoch` |
| `acknowledge(through)` | Acknowledgement epoch is current or retiring; `through < next_send` | Remove pending segments through the cumulative cursor | Memory is released |
| `receive(sequence == next_receive)` | Segment flow exists; epoch is current or retiring | Increment `next_receive` | Deliver bytes exactly once |
| `receive(sequence < next_receive)` | Segment flow exists; epoch is current or retiring | None | Ignore duplicate |
| `receive(sequence > next_receive)` | Segment flow exists; epoch is current or retiring | None | Reject out-of-order input |
| `advance_time(ticks)` | Oldest pending segment has reached its configured age | Clear pending queue; terminate flow | Release bounded memory and report `FlowTimedOut` |

## Epoch Transition Window

Each segment and logical acknowledgement includes a `FlowId` and `Epoch`.
`begin_transition` advances the current epoch and retains exactly the previous
epoch. This bounded two-epoch window admits in-flight data and acknowledgements
from the retiring carrier while the new carrier begins to carry the same
logical flow. `complete_transition` closes that window; old epochs are then
rejected as stale and future epochs are rejected before they can affect flow
state.

The receive cursor is the replay defense within an accepted epoch window: a
segment below the cursor cannot be delivered a second time. The tracer retains
no historical payload data for replay detection, so the replay state stays
constant per flow. Authentication of epoch-bearing inputs remains a Stage 3
carrier-attachment responsibility.

The receiver intentionally does not buffer out-of-order segments in this first
model. A later transition design may add a bounded reorder window only with a
specific memory limit and corresponding tests.

## Pending Lifetime

`FlowLimits` sets independent bounds for pending segment count, byte count, and
age. The tracer's deterministic clock advances only through `advance_time`.
When the oldest unacknowledged segment reaches `max_pending_age`, the tracer
clears that flow's pending queue and permanently terminates the flow with
`FlowTimedOut`. This deliberately fails the reliable flow instead of silently
discarding data and later claiming a successful resume.

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
acknowledgement, invalid acknowledgements, bounded session metadata and pending
memory, flow identity, retiring-epoch acknowledgements, and stale-epoch rejection. It also
exhaustively checks every receive trace up to four events over a three-sequence
input alphabet, plus deterministic loss, duplication, reordering, delay,
black-hole, recovery, and pending-timeout scenarios.

The seeded campaign runs 10,000 transitions over seeds `0..9999`. Every trial
mixes dropped, duplicated, delayed, and black-holed delivery around a
varying-position epoch transition and ordered recovery retransmission. It compares the complete
application byte sequence and contributes its FNV-1a checksum to the retained
aggregate `4550704779471716960`. Required CI stores the model-check log as a
90-day artifact.
