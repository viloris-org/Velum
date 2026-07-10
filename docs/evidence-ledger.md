# Evidence Ledger

This ledger prevents hypotheses from becoming accidental protocol promises.

## Facts

| ID | Fact | Evidence | Consequence |
|---|---|---|---|
| F-001 | CONNECT-UDP can run over HTTP/1.1, HTTP/2, and HTTP/3; HTTP/3 avoids TCP's nested loss recovery | [RFC 9298, sections 1 and 6](https://www.rfc-editor.org/rfc/rfc9298.html) | A TCP carrier is a deployability fallback, not the preferred datagram path |
| F-002 | QUIC DATAGRAM frames are unreliable and cannot carry arbitrarily large payloads | [RFC 9221](https://www.rfc-editor.org/rfc/rfc9221.html) | Datagram MTU discovery and explicit oversize behavior are mandatory |
| F-003 | MASQUE excludes proxy discovery and new congestion algorithms from its current charter | [MASQUE charter](https://datatracker.ietf.org/wg/masque/about/) | Velum may own endpoint selection and policy without competing with the base standard |
| F-004 | AnyTLS provides updateable padding schemes and multiplexed streams over TLS | [AnyTLS protocol](https://github.com/anytls/anytls-go/blob/main/docs/protocol.md) | Profile agility is feasible, but must be separated from session correctness |
| F-005 | AnyTLS documents residual timing, downstream, MTU, and probing weaknesses | [AnyTLS FAQ](https://github.com/anytls/anytls-go/blob/main/docs/faq.md) | Forest Native must model more than early uplink packet lengths |
| F-006 | Hysteria 2 maps TCP to QUIC streams and UDP to QUIC datagrams | [Hysteria 2 protocol](https://github.com/apernet/hysteria/blob/master/PROTOCOL.md) | This is the preferred-carrier performance baseline |

## Assumptions to Validate

| ID | Assumption | Test | Invalidation signal |
|---|---|---|---|
| A-001 | Common application TCP sessions can survive a carrier transition | Netem-based tracer with long-lived SSH, HTTP/2, and WebSocket flows | Flow resets or duplicate delivery after transition |
| A-002 | UDP failure can be distinguished from transient loss quickly enough to help users | Replay loss, black-hole, rate-limit, and recovery scenarios | False transitions harm P95 latency more than reconnecting |
| A-003 | Maintaining a warm TCP fallback has acceptable idle cost | Measure bytes, sockets, memory, and battery wakeups | Cost is unacceptable on mobile or metered networks |
| A-004 | Real service coexistence reduces simple active-probe signals | Differential probe suite against Velum and the cover service | Probe can classify the endpoint using stable pre-auth behavior |
| A-005 | A small operator accepts certificate, cover-service, and telemetry setup | Five deployment trials without developer assistance | Setup requires protocol expertise or fragile manual composition |

## Unknowns

- Which carrier should bootstrap session identity when both UDP and TCP work?
- Can migration remain replay-safe without a stable end-to-end session key?
- Should reliable-message semantics enter v1 or wait until stream migration is
  proven?
- Which real application profiles are ethical, deployable, and sufficiently
  common to support Forest Native behavior?
- How should mobile radio and battery cost affect path probing?
- Does an HTTP-native carrier provide enough benefit to justify its dependency
  and wire complexity, or should v1 use TLS and QUIC directly?
- Which client integration should be first: SOCKS, HTTP CONNECT, or TUN?

## Decisions

Accepted decisions are recorded only in accepted ADRs. The three current ADRs
are **Proposed** and remain reversible.

## Review Cadence

Update this ledger at every roadmap gate. A benchmark result becomes a fact
only when its environment, workload, baseline, and raw evidence are retained.

