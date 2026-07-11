# Validation Workload Harness

This dependency-free Node.js harness generates the five workload classes from
`../manifests/workloads.json`. It measures application-visible behavior only;
baseline processes, routing, certificates, and `tc/netem` faults remain owned
by the experiment runner.

## Start the Target

Run this on the server side of the baseline tunnel:

```bash
node validation/harness/server.mjs \
  --host 0.0.0.0 --tcp-port 9000 --udp-port 9001
```

The ready event is written to stderr so stdout remains available for external
process supervision.

## Run a Workload

Run this on the client side, pointing at the address exposed through the
baseline:

```bash
node validation/harness/run.mjs interactive-tcp \
  --host 127.0.0.1 --tcp-port 9000 --udp-port 9001
```

Replace `interactive-tcp` with `bulk-tcp`, `realtime-udp`, `dns-like-udp`, or
`idle-mobile`. The duration comes from the manifest. `--duration-seconds` is a
smoke-test override and must be recorded as a manifest deviation in formal
results.

Stdout is JSONL: zero or more `sample` records followed by exactly one
`summary`. Redirect it to the run's `samples.jsonl`; retain stderr separately.

## Limits

- UDP latency is round-trip unless client and server clocks are independently
  synchronized and the runner adds one-way instrumentation.
- `radio_wakeups` is `null`; mobile platform instrumentation must supply it.
- Bulk measurements count application payload, not harness framing bytes.
- This tool does not claim that traffic reached the intended tunnel. The
  experiment runner must retain routing and process evidence.

Run local codec and TCP/UDP integration tests with:

```bash
node validation/harness/harness.test.mjs
```
