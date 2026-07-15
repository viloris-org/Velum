# Privileged Traffic Host Protocol v1

Velum desktop traffic hosts expose a local, authenticated control channel to
the unprivileged client. The platform transport authenticates the peer and,
where necessary, transfers an operating-system handle. The JSON protocol never
transports credentials, certificates, filesystem paths, shell commands, or
user traffic.

Each frame starts with a four-byte unsigned big-endian JSON length. A zero
length or a body larger than 65,536 bytes closes the peer connection. JSON
objects are strict: an unknown field, command, capability, enum value, or
protocol version rejects the complete request.

Every request contains protocol `version`, numeric `request_id`, numeric
`profile_generation`, negotiated `capabilities`, a `command`, and its typed
`parameters`. Version 1 permits only `hello`, `status`, `start`, `stop`, and
`recover`. `start` carries only MTU, typed IPv4/IPv6 interface networks, and
typed DNS addresses. A response repeats the correlation and generation fields
and contains either a command-specific success category or a stable,
payload-free error category.

The host caches completed request IDs for its authenticated peer so a retry is
idempotent. A lower profile generation cannot replace a newer active
generation. Startup remains in `recovering` until the root-owned transaction
journal has restored any previous route and DNS state; `start` is rejected
during recovery. Logs may contain command, request ID, generation, result code,
and timing, but not serialized frames or platform configuration values.

Platform bindings are deliberately outside the wire format:

- Windows uses a local named pipe with an explicit current-user SID DACL,
  network access denied, and client identity verification.
- Linux uses system D-Bus with polkit authorization and Unix file-descriptor
  passing for the TUN handle.
- macOS uses Network Extension provider messages and the extension lifecycle;
  it does not install a root loopback helper.

The executable contract and malformed-input coverage live in
`crates/velum-helper-protocol`.
