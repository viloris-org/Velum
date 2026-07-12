# Velum

[![Required CI](https://github.com/viloris-org/Velum/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci.yml)
[![CI Health](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](rust-toolchain.toml)

[English](README.md) | [Español](README.es.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

Velum is a research-stage encrypted tunneling protocol for restricted,
unstable, and heterogeneous networks.

Its intended differentiator is session continuity across multiple carriers:
the same logical session can adapt between QUIC/UDP and TLS/TCP without making
applications choose a protocol up front. Velum also treats camouflage as
native coexistence with real Internet services, not as a packet-obfuscation
toggle.

> Project status: positioning and architecture discovery. No wire protocol or
> security claim is stable yet.

## Design Direction

- Preserve logical flows while network paths and carriers change.
- Give streams, messages, and datagrams distinct delivery semantics.
- Use standard cryptographic transports; do not invent cryptography.
- Make unauthenticated endpoints behave as real services.
- Measure performance, degradation, and detectability claims.
- Keep the Rust implementation split by responsibility and protocol layer.

Start with the [documentation index](docs/README.md) and the
[implementation status and roadmap](docs/roadmap.md).

## Experimental Operations

The `velum` research CLI can validate a provisioned configuration and deploy it
as a systemd user service with `velum deploy --config PATH`. This is a local
process-lifecycle helper, not a production-ready one-click infrastructure
installer: certificate, secret, DNS, firewall, monitoring, upgrade, and
rollback provisioning remain operator responsibilities. Read the [operator
guide](docs/velum-node.md) before using it.

Install an explicitly selected beta or stable release by downloading its
installer from the same immutable tag and running it locally:

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/v0.0.1-beta/scripts/install.sh
sh ./install.sh --channel beta --version v0.0.1-beta --add-to-path
```

After provisioning the configuration, credential file, and PEM material, deploy
the relay as the current user:

```bash
velum config validate --config /srv/velum/config.toml
velum deploy --config /srv/velum/config.toml
velum status --format json --config /srv/velum/config.toml
```

Open a new shell after installation, or run `export PATH="$HOME/.local/bin:$PATH"`
in the current one. `--add-to-path` changes only the current user's shell
startup file; omit it when PATH is managed externally.

## Current Validation

The repository pins Node 22.22.2 and Rust 1.97.0. With `cargo-deny` 0.20.2
installed, run every current Foundation gate with:

```bash
cargo xtask test
```

Architecture and documentation checks are also available independently as
`cargo xtask architecture` and `cargo xtask docs`.

## Current Non-Goals

- Claiming to be undetectable or unblockable.
- Designing a new cipher suite or TLS replacement.
- Replacing MASQUE, WireGuard, or every application proxy.
- Shipping multi-hop anonymity in the first protocol version.
- Freezing a wire format before the tracer experiments succeed.

Velum is licensed under the [Apache License 2.0](LICENSE). Contribution,
security, support, and release expectations are defined in the corresponding
repository policies.

## Disclaimer

Velum is experimental research software. It has not received a security audit
and must not be relied on for production security, privacy, availability, or
circumvention of network restrictions. Use it only where you are authorized to
do so and accept all associated risks.
