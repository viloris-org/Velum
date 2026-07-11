# `velum` Research CLI

`velum` is the Stage 2 QUIC relay composition root. It is experimental:
its command-line configuration and application control record are not a stable
Velum protocol and should not be deployed for production traffic.

## Build

```bash
cargo build --release -p velum-node --bin velum
```

## First-Time Setup

Run `velum` in a terminal and select **Guided first-time setup**. The wizard
collects the listener, PEM certificate and key paths, and one exact TCP target;
it generates a 32-byte credential into a separate owner-only file. It then
writes `~/.config/velum/config.toml` by default.

The same actions are available to automation through `velum setup`, `velum
config validate`, and `velum serve`. No secret bytes are stored in the TOML
configuration.

## Run

Supply a certificate chain and private key in PEM format. Each destination is
an exact IP address and port; hostnames, CIDR ranges, and implicit allow rules
are deliberately unsupported. Credentials use `principal-id:hex-secret`; all
configured secrets must have the same length.

```bash
target/release/velum serve
```

Run `velum` without arguments to open the guided terminal console. Use
`velum help` for available maintenance commands. `SIGINT` and
`SIGTERM` stop accepting connections, close the endpoint, and drain active
work up to `--shutdown-timeout-secs`.

## Local Maintenance

The running service creates the configured Unix-domain admin socket with owner
only permissions. `velum status`, `velum drain`, and `velum shutdown` use that
local socket; they never open a network management port. `drain` and
`shutdown` currently use the same bounded listener shutdown path. A future
reload command will require transactional live configuration semantics and is
not exposed yet.


## Install a Snapshot

Tags named `snapshot-*` produce checksum-verified GitHub prereleases for
Linux x86_64 and macOS x86_64/aarch64. Install only an explicitly selected
snapshot:

```bash
curl --fail --location --silent --show-error \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh \
  | sh -s -- --version snapshot-EXAMPLE
```

For a reviewable install, download `scripts/install.sh` at the same source
revision and run it locally. The script verifies the archive against the
release `SHA256SUMS` before installing `velum` to `~/.local/bin`. Select
another user-owned location with `--install-dir` when needed.

## External ACME

The base binary does not embed an ACME HTTP client. To install the pinned Lego
ACME companion on demand, run `scripts/install-lego.sh`; it downloads Lego
5.2.2 from its official release and verifies the published SHA-256 checksum
before writing only under `${XDG_DATA_HOME:-~/.local/share}/velum/tools`.
