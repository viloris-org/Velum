# Velum Operator Console

Flutter client for the Velum research-stage encrypted tunnel. The client turns the existing local operator workflow into a responsive dashboard without changing Velum's security boundary.

## Capabilities

- Service overview for phase, listener, uptime, admitted connections, and active flows.
- Session-continuity visualization across QUIC/UDP and TLS/TCP carriers.
- Version 1 TOML configuration editor and copyable configuration output.
- Local CLI bridge for `status`, `config validate`, `drain`, and `shutdown` on native builds.
- Safe browser demo adapter that never launches local processes.
- Activity timeline with no traffic payloads or secret values.
- Responsive navigation for desktop and narrow screens.

## Run

```powershell
cd apps/velum_client
flutter pub get
flutter run -d edge
```

Use the web server target when a browser should connect manually:

```powershell
flutter run -d web-server --web-hostname 127.0.0.1 --web-port 54021
```

## Verify

```powershell
flutter analyze
flutter test
flutter build web --release
```

## Runtime boundary

The web build always uses the demo adapter because browser applications cannot safely start a local Velum process. Native builds use `dart:io` to invoke the configured Velum binary. The client passes only documented CLI arguments and displays command output; management remains on Velum's local admin socket.

Velum remains experimental, unaudited research software. Do not rely on it for production security, privacy, availability, or circumvention.
