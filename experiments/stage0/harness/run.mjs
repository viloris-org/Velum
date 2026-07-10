#!/usr/bin/env node
import { HARNESS_VERSION, loadWorkload, parseOptions } from "./common.mjs";
import { runBulk, runIdle, runInteractive } from "./tcp.mjs";
import { runDnsLike, runRealtime } from "./udp.mjs";

const runners = {
  "interactive-tcp": runInteractive,
  "bulk-tcp": runBulk,
  "realtime-udp": runRealtime,
  "dns-like-udp": runDnsLike,
  "idle-mobile": runIdle,
};

try {
  const [workloadId, ...optionArguments] = process.argv.slice(2);
  if (!workloadId) throw new Error("usage: run.mjs <workload-id> --host HOST [--tcp-port PORT] [--udp-port PORT]");
  const options = parseOptions(optionArguments);
  const { manifestVersion, workload } = await loadWorkload(workloadId);
  const durationSeconds = options["duration-seconds"] === undefined
    ? workload.duration_seconds
    : Number(options["duration-seconds"]);
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) throw new Error("duration must be positive");
  const target = {
    host: options.host ?? "127.0.0.1",
    tcpPort: Number(options["tcp-port"] ?? 9000),
    udpPort: Number(options["udp-port"] ?? 9001),
  };
  const startedAt = new Date().toISOString();
  const result = await runners[workloadId](workload, target, durationSeconds);
  for (const sample of result.samples) console.log(JSON.stringify({ type: "sample", workload: workloadId, ...sample }));
  console.log(JSON.stringify({
    type: "summary",
    schema_version: 1,
    harness_version: HARNESS_VERSION,
    workload_manifest_version: manifestVersion,
    workload: workloadId,
    started_at: startedAt,
    finished_at: new Date().toISOString(),
    target,
    duration_seconds: durationSeconds,
    metrics: result.summary,
  }));
} catch (error) {
  console.error(error.stack ?? error.message);
  process.exitCode = 1;
}
