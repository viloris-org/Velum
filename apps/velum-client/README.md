# Velum Client

`flutter/` is the cross-platform desktop client. It loads the reviewed
`velum-client-ffi` native library and calls the direct client API; it does not
start a local proxy process.

Build the core from the workspace:

```text
cargo build -p velum-client-ffi --release
```

In Flutter, set **Native client library** to the resulting platform library,
then enter the relay address, TLS server name, CA PEM path, and credential-file
path. Flutter copies those bytes only for the direct native call and does not
print their contents. Applications open client streams or datagrams through the
versioned direct API; HTTP CONNECT is unsupported.

This is not a production client or a stable protocol implementation. It uses
the application-owned Stage 2 control record described in ADR-0012.
