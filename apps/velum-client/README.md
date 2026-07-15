# Velum Client

`flutter/` is the experimental desktop and Android control application. It loads
the reviewed `velum-client-ffi` native library. Flutter uses runtime ABI v2 to
create a handle, start without waiting for the network result, poll
authoritative lifecycle snapshots, stop, and destroy. ABI v2 replaces the
internal-test ABI v1; consumers must rebuild against its updated configuration
layout. The desktop
app can install a loopback system proxy for IP-literal HTTPS CONNECT and SOCKS
traffic after the runtime is online. It does not install a TUN device.

Build the core from the workspace:

```text
cargo build -p velum-client-ffi --release
```

In Flutter, set **Native client library** to the resulting platform library,
then enter the relay address, TLS server name, CA PEM path, and credential-file
path. Flutter copies those bytes only for the direct native call and does not
print their contents; the Dart allocation is cleared after the native start
call copies it. Native applications can open client streams or datagrams
through the versioned direct API; the current Flutter UI controls lifecycle and
renders snapshots. HTTP CONNECT is unsupported.

This is not a production VPN or a stable protocol implementation, and an
`Online` runtime does not imply that system traffic is routed. The runtime does
observe a later QUIC close and moves that generation to `Failed`, but it does
not yet implement reconnect policy or route health. It uses the
application-owned Stage 2 control record described in
[ADR-0012](../../docs/adr/0012-flutter-direct-client-api.md). The target client
boundary and migration are documented in
[ADR-0013](../../docs/adr/0013-client-runtime-boundary.md) and the
[client architecture proposal](../../docs/client-architecture.md).

Android builds require `libvelum_client_ffi.so` to be packaged for each shipped
ABI under `flutter/android/app/src/main/jniLibs/`; use
`scripts/build-android-client.sh` for the arm64 development build. The Android
runner declares a VPN service but does not establish a TUN device until its
TCP/UDP/DNS packet engine is complete and device-tested.
