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

`samples.jsonl` contains one JSON object per trial. Raw timing and counter
samples belong here; summaries must be derived rather than substituted for raw
data. `checksums.sha256` covers every retained file except itself.

Packet captures may be retained under `artifacts/` only after payload and
endpoint privacy review. Never store credentials, private keys, or operator
destination data.

Interrupted, failed, and invalid trials remain present with a machine-readable
status and reason. A benchmark report must state exclusions explicitly.
