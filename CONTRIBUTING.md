# Contributing to Velum

Velum is a research-stage protocol project. Contributions must preserve the
distinction between verified evidence, hypotheses, proposed decisions, and
stable behavior.

## Before Opening a Change

- Start from an item in [`docs/roadmap.md`](docs/roadmap.md) or explain which
  evidence gate the change advances.
- Use an ADR for changes to responsibility ownership, dependency direction,
  security boundaries, wire behavior, or compatibility policy.
- Do not describe proposed protocol behavior as stable or production-ready.
- Keep runtime modules inside the ownership boundaries in
  [`docs/architecture-contract.yaml`](docs/architecture-contract.yaml).

## Local Validation

Install Node 22.22.2, Rust 1.97.0 with `rustfmt` and `clippy`, and
`cargo-deny` 0.20.2. Then run:

```bash
cargo xtask test
```

Run `cargo xtask architecture` while changing workspace structure or local
dependencies, and `cargo xtask docs` while changing documentation.

## Evidence and Reviews

Benchmark and interview evidence must follow the retention rules under
`validation/`. A pull request may mark a roadmap item `DONE` only when
it links a repeatable command, retained result, or reviewed artifact. Review is
required from the responsibility label named in the architecture contract;
specific maintainers will be assigned before external releases.

By contributing, you agree that your contribution is licensed under the
Apache License 2.0.
