# Validation Evidence

This directory provides a repeatable evidence collection process for roadmap
validation stages. It does not define the Velum wire protocol or count as
evidence by itself.

## Contents

- `manifests/networks.json` defines the reference path and failure matrix.
- `manifests/workloads.json` defines the five required workload classes.
- `manifests/baselines.json` records competitor selection and version pins.
- `interviews/README.md` defines the operator interview record.
- `results/README.md` defines the retained raw-result layout and its executable
  integrity checks.
- `harness/README.md` documents the dependency-free workload generator.
- `validate.mjs` checks manifest structure, identifiers, and cross-references.

## Validation

Run the structural checks while designing experiments:

```bash
node validation/validate.mjs
```

Before collecting publishable results, require every baseline and toolchain to
have an immutable version or revision:

```bash
node validation/validate.mjs --ready
```

Validate retained result structure, trial summaries, and checksums with:

```bash
node validation/results/validate.mjs
```

The initial manifests deliberately fail `--ready`. Baseline entries remain
`candidate_pinned` until their builds and workload coverage are verified, and
server/toolchain versions must come from the actual reference hosts.

## Evidence Workflow

1. Copy no manifest into an ad hoc script. Runners consume these files or
   record an explicit transformed copy with a new `manifest_version`.
2. Pin host OS, kernel, impairment tooling, workload tooling, and every
   baseline before a benchmark run.
3. Allocate one immutable result directory per run as described in
   `results/README.md`.
4. Record failed and interrupted trials. Do not retain only favorable samples.
5. Promote a result to `docs/evidence-ledger.md` only after its environment,
   workload, baseline, sample count, and raw artifacts are reviewable.

Each roadmap stage exits only when its operator-validation and reproducibility
gates are met. Passing this validator is necessary setup, not an exit signal.
