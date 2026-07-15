# Velum Client

`flutter/` is the experimental desktop and Android control application. It loads
the reviewed `velum-client-ffi` native library. Flutter uses runtime ABI v2 to
create a handle, start without waiting for the network result, poll
authoritative lifecycle snapshots, stop, and destroy. Profile ABI v3 validates
and normalizes managed Velum YAML without changing runtime ABI v2. ABI v2 replaces the
internal-test ABI v1; consumers must rebuild against its updated configuration
layout. On Windows, macOS, and supported Linux desktops, the app can install a
restorable dual-loopback system proxy for ordinary HTTP, HTTP CONNECT, and
SOCKS traffic. On Android, it can establish a dual-stack TUN VPN that forwards TCP, UDP, and DNS
through the active runtime.

The settings surface configures the proxy port and bypass list, Android TUN
addresses, prefixes, MTU, DNS servers, and routes. The shared policy supports
ordered `DOMAIN`, `DOMAIN-SUFFIX`, dual-stack `IP-CIDR`, `DST-PORT`, and `MATCH`
rules with `DIRECT`, `PROXY`, `REJECT`, and explicit-node actions. Runtime ABI
v2 proxy mode accepts the first three actions; explicit-node routing requires
the feature-gated ABI v3 node engine. Android TUN rule parity remains gated on
that same engine integration and retained device evidence.

Build the core from the workspace:

```text
cargo build -p velum-client-ffi --release
```

The application loads its packaged native library. Desktop development builds
can set `VELUM_CLIENT_LIBRARY` at build time to use the resulting platform
library, then import a native-validated Velum profile or enter relay details
manually.
Imported profiles contain only `secret://velum/` references; credential and CA
bytes resolve from platform secure storage. Legacy file fields migrate those
bytes into secure storage. Flutter copies connection bytes only for the direct native call and does not
print their contents; the Dart allocation is cleared after the native start
call copies it. Native applications can open client streams or datagrams
through the versioned direct API; the current Flutter UI controls lifecycle and
renders snapshots. Ordinary HTTP, HTTP CONNECT, and SOCKS5 CONNECT are supported.

The configuration screen also accepts a versioned `.velum-enroll` file on
desktop and Android. The Android host exposes QR scanning for the identical
canonical JSON. Native ABI v1 validates the enrollment before Flutter writes
the per-device credential and optional CA into platform secure storage; the
source file is removed when the platform grants delete access.

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
`scripts/build-android-client.sh` for the arm64 development build. Android
release support still requires retained device evidence for permission denial,
TCP, UDP, DNS, relay loss, process death, suspend/resume, and network changes.
## Android release signing

Release APK builds require a dedicated Android upload key. Store its Base64-encoded
contents and credentials in these GitHub Actions secrets:

- `ANDROID_KEYSTORE_BASE64`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

The release workflow decodes the keystore only in the runner's temporary directory.
For local release builds, set the same password and alias variables plus
`ANDROID_KEYSTORE_PATH` to the local keystore path.
