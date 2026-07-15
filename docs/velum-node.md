# `velum` Research CLI

`velum` is the Stage 2 QUIC relay composition root. It is experimental:
its command-line configuration and application control record are not a stable
Velum protocol and should not be deployed for production traffic.

## Build

```bash
cargo build --release -p velum-node --bin velum
```

## First-Time Setup

Run `velum` in a terminal and select **Guided setup or reconfigure**. For a new
configuration, the wizard generates a random port in the `49152-65535` dynamic
range after checking that both TCP and UDP can bind it, then lets the operator
confirm it and one exact TCP target. It generates
a 32-byte credential into a separate owner-only file and offers three
certificate sources:

1. request a CA certificate through ACME DNS-01;
2. select an existing PEM certificate and private key; or
3. generate an owner-only self-signed certificate for explicit client trust.

The wizard writes `~/.config/velum/config.toml` by default. It is resumable and
can reconfigure an existing file without replacing an existing credential.

For automation, use `velum init`, provision the generated TOML, secret file,
and PEM material through your secret-management system, then run `velum deploy`.
It validates all of that material before creating and starting a per-config
systemd user service. `velum setup` is intentionally interactive. No secret
bytes are stored in the TOML configuration.

## Client Enrollment

Issue a separate 32-byte credential for every client device. Mobile clients can
scan the terminal QR code; desktop and mobile clients can import the same
canonical enrollment from an owner-only file:

```bash
velum client issue --name phone \
  --relay 203.0.113.10:4433 \
  --server-name relay.example \
  --qr

velum client issue --name laptop \
  --relay 203.0.113.10:4433 \
  --server-name relay.example \
  --output laptop.velum-enroll
```

Use `--trust custom-ca` to include the configured public certificate for a
self-signed relay; system trust is the default. The CLI never accepts a
credential value. It generates the secret, adds a named principal to the node
configuration, and creates the enrollment only after local validation.

Revoke one device without rotating every client:

```bash
velum client revoke --name laptop
```

Issuance and revocation update persistent configuration. Restart or redeploy a
running relay before the change becomes active in its in-memory authenticator.

The guided setup offers to create the first enrollment, and the operator
console exposes issuance and revocation as separate actions. Enrollment files
contain a long-lived secret. Transfer each file over an authorized channel,
import it once, and remove every remaining copy.

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

Remove a deployed relay with an explicit confirmation:

```bash
velum uninstall --config /srv/velum/config.toml
```

The command stops and removes the configuration-scoped systemd user service
and its local admin socket. Add `--purge` to remove the configuration file as
well; outside an interactive terminal, use `--yes` to confirm. Credentials,
certificates, the Velum binary, and the Lego tool are retained because they may
be owned by the operator, a package manager, or another provisioning system.
The guided operator console includes the same uninstall flow.


## Install a Release

Tags named `v*` produce checksum-verified GitHub Releases for Linux
x86_64/aarch64 and macOS x86_64/aarch64. Tags with a prerelease suffix, such as
`v0.0.1-beta`, produce prereleases; `vX.Y.Z` tags produce stable releases.
Install only an explicitly selected release:

Choose a channel and paste its command. The installer resolves the latest
published matching release itself:

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh && \
sh ./install.sh --channel stable --latest --add-to-path
```

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh && \
sh ./install.sh --channel beta --latest --add-to-path
```

For a reproducible installation, download a reviewed installer from a pinned
tag and select the exact version:

```bash
INSTALLER_TAG='vX.Y.Z'
curl --fail --location --remote-name \
  "https://raw.githubusercontent.com/viloris-org/Velum/${INSTALLER_TAG}/scripts/install.sh"

sh ./install.sh --channel beta --version vX.Y.Z-beta --add-to-path
```

The convenience commands fetch the current installer from `main`, and
`--latest` resolves a moving release reference. The installer prints the
resolved tag before downloading it; use the pinned form for a recorded or
reproducible installation. When run from an interactive terminal, the
installer explains the setup stages and certificate choices, then runs the
installed `velum setup` command. It also installs the pinned Lego companion
used by the ACME option.

The script verifies the archive against the release `SHA256SUMS` before
installing `velum` to `~/.local/bin`. This is integrity checking, not a
release signature: beta releases remain unsigned research artifacts. Pass
`--add-to-path` to add the default `~/.local/bin` directory to the current
user's shell startup file, then open a new shell. Select another user-owned
location with `--install-dir` when needed; custom install directories require
external PATH management.

## External ACME

The base binary does not embed an ACME HTTP client. The release installer
installs the pinned Lego companion automatically. Source builds can install it
with `scripts/install-lego.sh`; both paths download Lego 5.2.2 from its official
release, verify the published SHA-256 checksum, and write it only under
`${XDG_DATA_HOME:-~/.local/share}/velum/tools`.

For DNS-01 configuration, issuance, renewal, rollback-on-reload-failure
certificate activation, and a systemd user timer, see [ACME operations](acme.md). DNS-provider tokens
remain environment variables consumed by Lego and are never stored in Velum
configuration.

## Optional Cover Service

`velum init` includes the following commented example. Removing the comment
markers enables a bounded plaintext HTTP/1.1 reverse-proxy listener; it is
intended to run behind an operator-owned standard TLS terminator. The TCP cover
listener may use the same IP and port as the QUIC UDP listener.

```toml
[cover_service]
bind = "0.0.0.0:4433"
upstream = "cover.example.com:8080"
request_head_timeout_secs = 5
upstream_timeout_secs = 5
max_connections = 256
```

`bind` must be a literal `IP:PORT`; `upstream` may be a literal address or a
`hostname:PORT`. Hostnames are resolved when the configuration is loaded and
their selected address remains fixed until the next reload or restart. The
service is disabled when the section is absent. It does not implement TLS
fallback attachment routing; the upstream must be a real application the
operator is authorized to serve.
