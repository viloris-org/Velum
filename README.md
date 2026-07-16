# Velum

[![Required CI](https://github.com/viloris-org/Velum/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci.yml)
[![CI Health](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](rust-toolchain.toml)
[![Flutter 3.44.0](https://img.shields.io/badge/Flutter-3.44.0-02569B?logo=flutter&logoColor=white)](https://flutter.dev)

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

## Current Validation

The repository pins Node 22.22.2 and Rust 1.97.0. With `cargo-deny` 0.20.2
installed, run every current Foundation gate with:

```bash
cargo xtask test
```

Architecture and documentation checks are also available independently as
`cargo xtask architecture` and `cargo xtask docs`.

## Server Deployment

Install a published release with the checksum-verifying installer. It installs
the `velum` command to `~/.local/bin` and adds that directory to your shell
PATH. Choose the stable channel for a release version, or the beta channel for
a prerelease:

> **Which channel should I use?** `stable` installs the newest stable
> `vX.Y.Z` release and is the preferred choice when one is available. `beta`
> installs the newest prerelease and may include unfinished or changed
> behavior. Both commands use a moving `--latest` reference; use
> `--version vX.Y.Z` or `--version vX.Y.Z-beta` for a reproducible install.

### Stable Channel

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh
sh ./install.sh --channel stable --latest --add-to-path
```

### Beta Channel

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh
sh ./install.sh --channel beta --latest --add-to-path
```

Open a new shell, then deploy the relay on Linux as a systemd user service:

```bash
velum setup --config ~/.config/velum/config.toml
velum config validate --config ~/.config/velum/config.toml
velum deploy --config ~/.config/velum/config.toml
```

`setup` creates the relay configuration and credential, and configures TLS
material. `deploy` validates those files before creating and starting the
systemd user service. Use `velum status`, `velum drain`, and `velum shutdown`
with the same `--config` path to operate the deployed relay. For a source
build, run `cargo build --release -p velum-node --bin velum` and add
`./target/release` to your `PATH` before using the same commands.

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

## Stargazers Over Time

[![Stargazers over time](https://starchart.cc/viloris-org/Velum.svg?variant=adaptive)](https://starchart.cc/viloris-org/Velum)
