# ACME Operations

Velum uses the externally installed, pinned Lego companion for ACME. The
relay does not contain an ACME client, DNS-provider SDK, or ACME account key.
This keeps DNS credentials and certificate-account state outside the network
process.

## Preconditions

The current QUIC listener cannot answer HTTP-01 or TLS-ALPN-01 challenges:
those challenges require a TCP listener. Use a Lego DNS-01 provider and grant
its token only the narrow DNS permissions required by that provider.

Install the companion, then set the provider-specific environment variables
documented by Lego in a mode-`0600` environment file. Do not put those values
in `config.toml` or command arguments.

```bash
scripts/install-lego.sh
export VELUM_LEGO_BIN="$HOME/.local/share/velum/tools/lego/v5.2.2/lego"
export YOUR_DNS_PROVIDER_TOKEN='...'
```

Configure a staging policy first. `--staging` uses the Let's Encrypt staging
directory, avoiding production rate limits during setup.

```bash
velum acme configure \
  --email ops@example.com \
  --dns your-dns-provider \
  --domain relay.example.com \
  --staging
velum acme obtain
```

After a successful staging test, repeat `acme configure` without `--staging`,
then run `velum acme obtain`. The command invokes Lego with DNS-01, validates
the generated certificate and key, stages both replacements before activation,
retains the previous pair until a confirmed local socket reload, and restores
it if reload fails. A process crash between the two file replacements still
requires operator recovery; this is not a multi-file atomic filesystem update.
The ACME account and
generated files reside in `~/.local/state/velum/acme` (or
`$XDG_STATE_HOME/velum/acme`) with owner-only directory permissions.

`velum cert verify` checks the configured certificate/key pair and reports the
leaf certificate expiry and remaining whole days. It fails for an expired
certificate.

## Renewal

Run the following as the same user that owns Velum's configuration and ACME
state. The DNS-provider credentials must be available to the service.

```ini
# ~/.config/systemd/user/velum-acme-renew.service
[Unit]
Description=Renew and activate Velum ACME certificate

[Service]
Type=oneshot
EnvironmentFile=%h/.config/velum/acme.env
ExecStart=%h/.local/bin/velum acme renew
```

```ini
# ~/.config/systemd/user/velum-acme-renew.timer
[Unit]
Description=Check Velum ACME renewal daily

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

Enable it with `systemctl --user daemon-reload` and
`systemctl --user enable --now velum-acme-renew.timer`. Renewal uses the
configured `renew_before_days` value (30 by default). A failed renewal leaves
the active PEM files untouched. A successful activation requires the running
service's local admin socket; start or restart the relay before running the
first renewal.

Wildcard names are deliberately unsupported by this first wrapper. Use a
concrete DNS name as the primary certificate identity.
