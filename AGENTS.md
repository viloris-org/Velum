# Repository Guidelines

## Project Structure & Module Organization

Velum is a Rust 2024 workspace for an experimental encrypted-tunneling
protocol. Production code is separated by responsibility: `apps/velum-node/`
contains the node binary, `crates/velum-*` contains protocol, crypto, carrier,
session, policy, telemetry, and server libraries, and `xtask/` owns repository
validation commands. Keep behavior within the ownership boundaries declared in
`docs/architecture-contract.yaml`. Research evidence and repeatable validation
artifacts live under `validation/`; design decisions and the roadmap live in
`docs/`.

## Build, Test, and Development Commands

Use Rust 1.97.0 with `rustfmt` and `clippy`, Node 22.22.2, and `cargo-deny`
0.20.2 (see `rust-toolchain.toml` and `.node-version`).

```bash
cargo xtask test           # Run the required Foundation validation gates
cargo xtask model-check    # Check the deterministic session model
cargo xtask architecture   # Validate workspace ownership/dependency rules
cargo xtask docs           # Validate documentation checks
cargo test -p velum-session # Run one crate's tests while iterating
cargo fmt --all --check    # Check Rust formatting
cargo clippy --workspace --all-targets -- -D warnings # Review lint warnings
```

Run `cargo xtask test` before opening a pull request. Run architecture checks
when changing crate dependencies or workspace structure, and docs checks when
editing documentation.

## Coding Style & Naming Conventions

Follow `rustfmt`; use four-space indentation and idiomatic Rust naming:
`snake_case` for modules, functions, and variables; `PascalCase` for types;
and `SCREAMING_SNAKE_CASE` for constants. Favor small, responsibility-focused
modules over broad shared state. The workspace forbids `unsafe` code and treats
Clippy's `all` lint group as warnings; fix warnings rather than suppressing
them without a documented reason.

## Testing Guidelines

Place unit tests near their implementation using `#[cfg(test)]`, with behavior
focused test names such as `rejects_expired_session_ticket`. Add integration or
model coverage for protocol and state-transition changes. Retain required
benchmark, interview, and validation evidence under `validation/` according to
its local README and retention rules.

## Commit & Pull Request Guidelines

Use concise imperative subjects. Existing history uses both plain forms
(`Add v0 protocol codec`) and Conventional Commit prefixes when helpful
(`feat(session): validate transition state model`, `fix: bound pending QUIC
handshakes`). Keep commits scoped to one concern.

In each PR, explain the problem, tests run, and the roadmap evidence gate it
advances. Link retained results for any item marked `DONE`. Include an ADR for
changes to ownership, dependency direction, security boundaries, wire behavior,
or compatibility policy; never present proposed protocol behavior as stable.
