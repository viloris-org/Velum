# Protocol Landscape

This comparison identifies design space; it is not a security ranking. Protocol
behavior changes over time, so conclusions must be rechecked before a wire
protocol is frozen.

## Comparison

| Protocol | Primary strength | Transport model | Camouflage model | Gap relevant to Velum |
|---|---|---|---|---|
| MASQUE | Standards-based HTTP tunneling | CONNECT-UDP and CONNECT-IP over HTTP, preferably HTTP/3 | Inherits HTTP and TLS ecosystem behavior | Discovery, new congestion control, and censorship response are outside the working-group scope; cross-carrier continuity is not its product goal |
| AnyTLS | Mitigating nested TLS traffic features | Multiplexed proxy streams over TLS/TCP; UDP over a TCP-based subprotocol | Configurable early-packet splitting and padding, with server-updatable schemes | TCP head-of-line behavior; documented timing, downstream-shape, MTU, and active-probing limitations |
| VLESS/Xray | Small proxy request framing and transport composition | TCP, UDP, mux, and external transports depending on configuration | Supplied by the selected TLS/REALITY/XTLS/transport composition | Security and behavior are properties of the composition, increasing configuration and analysis surface |
| Hysteria 2 | Throughput on lossy or congested paths | QUIC streams plus QUIC datagrams | HTTP/3 server behavior plus optional Salamander/Gecko obfuscation | UDP/QUIC dependency; fixed-rate Brutal mode requires a credible bandwidth estimate and accepts fairness costs |
| WireGuard | Minimal, reviewed layer-3 VPN | Encrypted UDP tunnel | No application-protocol camouflage | Excellent VPN baseline, but no TCP carrier fallback or application-aware delivery policy |

## MASQUE Baseline

[RFC 9298](https://www.rfc-editor.org/rfc/rfc9298.html) defines UDP proxying
over HTTP. It explicitly discusses nested congestion controllers, double loss
recovery when HTTP runs over TCP, and datagram MTU constraints.

[RFC 9484](https://www.rfc-editor.org/rfc/rfc9484.html) extends the model to IP
proxying. It carries IP packets in HTTP Datagrams and requires careful handling
of IPv6 minimum MTU and ICMP errors.

The current [MASQUE working-group charter](https://datatracker.ietf.org/wg/masque/about/)
states that proxy discovery and new congestion-control or loss-recovery
algorithms are out of scope. It focuses on extensions to CONNECT-UDP and
CONNECT-IP, including QUIC-aware proxying and bound UDP.

**Implication:** Velum should reuse standard mechanisms where they satisfy a
requirement, but its differentiated work is policy, continuity, deployment,
and failure behavior rather than inventing another HTTP tunnel vocabulary.

## AnyTLS Baseline

The [AnyTLS protocol](https://github.com/anytls/anytls-go/blob/main/docs/protocol.md)
uses TLS/TCP, a multiplexed session layer, and a server-updatable padding scheme
for early writes. Its own [FAQ](https://github.com/anytls/anytls-go/blob/main/docs/faq.md)
documents unresolved downstream, timing, TLS-in-TLS, MTU, and active-probing
characteristics.

**Implication:** updateable profiles and cheap feature rotation are useful, but
packet-length padding alone is not a sufficient camouflage model.

## VLESS Baseline

The [Xray-core VLESS implementation](https://github.com/XTLS/Xray-core/tree/main/proxy/vless)
encodes a small versioned request with user identity, command, and destination,
while encryption and transport choices are configured around it.

**Implication:** a small inner protocol is attractive, but Velum should expose
one coherent security and failure model instead of requiring users to reason
about arbitrary layer combinations.

## Hysteria 2 Baseline

The [Hysteria 2 protocol specification](https://github.com/apernet/hysteria/blob/master/PROTOCOL.md)
uses QUIC streams for TCP, QUIC datagrams for UDP, HTTP/3 behavior for
unauthenticated access, and optional packet obfuscation. Its congestion model
can use conventional controllers or a configured fixed-rate mode.

**Implication:** QUIC is the strong preferred carrier, but it cannot be the only
carrier if continuity under UDP restriction is the primary product claim.

## Defensible Differentiation

Features already available elsewhere are not a moat: encryption, mux, QUIC,
padding, port hopping, and single-binary deployment are table stakes.

The candidate differentiation requiring experimental proof is:

- one logical session spanning unlike transport carriers;
- flow migration driven by observable policy rather than manual configuration;
- distinct delivery semantics sharing one session and identity;
- real service coexistence as a protocol and deployment invariant;
- explainable degradation, with deterministic fallback rather than silent
  stalls.

