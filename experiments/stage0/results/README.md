# Raw Result Retention

Create one immutable directory per run:

```text
results/<UTC timestamp>-<baseline>-<network>-<workload>/
  metadata.json
  samples.jsonl
  stdout.log
  stderr.log
  checksums.sha256
  artifacts/
```

`metadata.json` must record:

- the three manifest versions and selected IDs;
- exact baseline version or source revision;
- client and server hardware, OS, kernel, and tool versions;
- command arguments and redacted configuration hashes;
- UTC start/end times, trial count, random seeds, and run status;
- deviations from the manifest and the reason for each deviation.

The version 1 shape is:

```json
{
  "schema_version": 1,
  "manifests": {
    "networks": "0.1.0",
    "workloads": "0.1.0",
    "baselines": "0.3.0"
  },
  "selection": {
    "baseline": "anytls",
    "network": "udp-black-hole",
    "workload": "interactive-tcp"
  },
  "baseline": { "version": "v0.0.13", "revision": "9666872" },
  "environment": {
    "client": { "hardware": "...", "os": "...", "kernel": "..." },
    "server": { "hardware": "...", "os": "...", "kernel": "..." },
    "tools": { "node": "22.22.2", "tc": "iproute2-6.17.0" }
  },
  "command": {
    "argv": ["node", "experiments/stage0/harness/run.mjs", "interactive-tcp"],
    "configuration_sha256": "<64 lowercase hexadecimal characters>"
  },
  "started_at": "2026-07-11T01:02:03Z",
  "ended_at": "2026-07-11T01:04:03Z",
  "trial_count": 1,
  "random_seeds": [42],
  "status": "completed",
  "deviations": []
}
```

The directory timestamp is the UTC `started_at` value to the second, with
colons replaced by hyphens. Non-completed runs use `failed`, `interrupted`, or
`invalid` and add a non-empty `status_reason`.

`samples.jsonl` retains the harness JSONL without transformation: zero or more
`sample` records followed by one `summary` per trial. The number of summaries
must equal `trial_count`, and the last record is a summary. Idle workloads may
legitimately produce no sample records. Raw timing and counter samples must not
be replaced by a report-level aggregate. `checksums.sha256` uses the format
produced by `sha256sum` and covers every retained file except itself.

Packet captures may be retained under `artifacts/` only after payload and
endpoint privacy review. Never store credentials, private keys, or operator
destination data.

Interrupted, failed, and invalid trials remain present with a machine-readable
status and reason. A benchmark report must state exclusions explicitly.

Validate every retained directory with:

```bash
node experiments/stage0/results/validate.mjs
```

Pass one or more directory paths to validate results outside this repository.
An empty repository result directory is valid setup, but does not satisfy a
Stage 0 evidence gate.
