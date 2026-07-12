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

For automation, use `velum init`, provision the generated TOML, secret file,
and PEM material through your secret-management system, then run `velum deploy`.
It validates all of that material before creating and starting a per-config
systemd user service. `velum setup` is intentionally interactive. No secret
bytes are stored in the TOML configuration.

## Run

Supply a certificate chain and private key in PEM format. Each destination is
an exact IP address and port; hostnames, CIDR ranges, and implicit allow rules
are deliberately unsupported. Credentials use `principal-id:hex-secret`; all
configured secrets must have the same length.

```bash
target/release/velum serve
```

On a systemd host, deploy a validated configuration as a restart-on-failure
user service with one command:

```bash
target/release/velum deploy --config /srv/velum/config.toml
```

`deploy` writes an owner-only, configuration-scoped unit under
`$XDG_CONFIG_HOME/systemd/user` (or `~/.config/systemd/user`), runs
`systemctl --user daemon-reload`, enables the unit, and starts or restarts it.
Use `--dry-run` to inspect the generated unit first. The command is deliberately
not a secret or certificate provisioner; those materials must exist before it
runs. It requires a working systemd user manager. For a relay that must survive
logout, enable user lingering explicitly with `loginctl enable-linger "$USER"`
according to your host's account policy.

Run `velum` without arguments to open the guided terminal console. Use
`velum help` for available maintenance commands. `SIGINT` and
`SIGTERM` stops accepting connections, closes the endpoint, and drains active
work up to `limits.shutdown_timeout_secs` from the TOML configuration. `velum
drain` stops accepting new connections while existing accepted connections run
to completion; `velum shutdown` closes immediately and applies that bound.

## Local Maintenance

The running service creates the configured Unix-domain admin socket with owner
only permissions. `velum status`, `velum drain`, and `velum shutdown` use that
local socket; they never open a network management port. `drain` and
`shutdown` have distinct behavior: drain stops admitting new connections and
waits for accepted work, while shutdown closes the endpoint and applies the
configured shutdown bound. `reload` is used internally after ACME activation
to replace the server certificate only after the replacement configuration
loads successfully.

`velum status --format json` emits a stable, payload-free record with state,
listener, uptime, admitted connection count, and active-flow count. Admin
requests time out after five seconds. A custom configuration path receives an
adjacent private `.velum-admin/admin.sock` by default, so independent local instances do not share
their management socket.


## Install a Snapshot

Tags named `snapshot-*` produce checksum-verified GitHub prereleases for
Linux x86_64 and macOS x86_64/aarch64. Install only an explicitly selected
snapshot:

Download the installer from the same immutable tag as the requested snapshot,
review it, then run it locally. Do not execute an installer fetched from the
moving `main` branch.

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/snapshot-EXAMPLE/scripts/install.sh
sh ./install.sh --version snapshot-EXAMPLE --add-to-path
```

The script verifies the archive against the release `SHA256SUMS` before
installing `velum` to `~/.local/bin`. This is integrity checking, not a
release signature: snapshots remain unsigned research artifacts. Pass
`--add-to-path` to add the default `~/.local/bin` directory to the current
user's shell startup file, then open a new shell. Select another user-owned
location with `--install-dir` when needed; custom install directories require
external PATH management.

## External ACME

The base binary does not embed an ACME HTTP client. To install the pinned Lego
ACME companion on demand, run `scripts/install-lego.sh`; it downloads Lego
5.2.2 from its official release and verifies the published SHA-256 checksum
before writing only under `${XDG_DATA_HOME:-~/.local/share}/velum/tools`.

For DNS-01 configuration, issuance, renewal, rollback-on-reload-failure
certificate activation, and a systemd user timer, see [ACME operations](acme.md). DNS-provider tokens
remain environment variables consumed by Lego and are never stored in Velum
configuration.
