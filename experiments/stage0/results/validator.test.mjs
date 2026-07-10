import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { validateResultDirectory } from "./validator.mjs";

async function sha256(path) {
  return createHash("sha256").update(await readFile(path)).digest("hex");
}

async function writeChecksums(directory, names) {
  const checksums = [];
  for (const name of names.sort()) checksums.push(`${await sha256(join(directory, name))}  ${name}`);
  await writeFile(join(directory, "checksums.sha256"), `${checksums.join("\n")}\n`);
}

async function fixture() {
  const parent = await mkdtemp(join(tmpdir(), "velum-results-"));
  const directory = join(parent, "2026-07-11T01-02-03Z-anytls-udp-black-hole-interactive-tcp");
  await mkdir(join(directory, "artifacts"), { recursive: true });
  const metadata = {
    schema_version: 1,
    manifests: { networks: "0.1.0", workloads: "0.1.0", baselines: "0.3.0" },
    selection: { baseline: "anytls", network: "udp-black-hole", workload: "interactive-tcp" },
    baseline: { version: "v0.0.13", revision: "9666872" },
    environment: {
      client: { hardware: "fixture", os: "fixture", kernel: "fixture" },
      server: { hardware: "fixture", os: "fixture", kernel: "fixture" },
      tools: { node: "22.22.2" },
    },
    command: { argv: ["node", "run.mjs", "interactive-tcp"], configuration_sha256: "a".repeat(64) },
    started_at: "2026-07-11T01:02:03Z",
    ended_at: "2026-07-11T01:04:03Z",
    trial_count: 1,
    random_seeds: [42],
    status: "completed",
    deviations: [],
  };
  const files = {
    "metadata.json": `${JSON.stringify(metadata, null, 2)}\n`,
    "samples.jsonl": `${JSON.stringify({ type: "sample", sequence: 0 })}\n${JSON.stringify({ type: "summary", workload: "interactive-tcp" })}\n`,
    "stdout.log": "fixture stdout\n",
    "stderr.log": "fixture stderr\n",
    "artifacts/route.txt": "fixture route\n",
  };
  for (const [name, contents] of Object.entries(files)) await writeFile(join(directory, name), contents);
  await writeChecksums(directory, Object.keys(files));
  return directory;
}

test("accepts a complete, internally consistent result", async () => {
  const result = await validateResultDirectory(await fixture());
  assert.deepEqual(result.errors, []);
});

test("detects modified retained artifacts", async () => {
  const directory = await fixture();
  await writeFile(join(directory, "artifacts/route.txt"), "changed\n");
  const result = await validateResultDirectory(directory);
  assert.ok(result.errors.includes("checksums.sha256: digest mismatch for artifacts/route.txt"));
});

test("accepts summary-only output for workloads without samples", async () => {
  const directory = await fixture();
  await writeFile(join(directory, "samples.jsonl"), `${JSON.stringify({ type: "summary", workload: "interactive-tcp" })}\n`);
  await writeChecksums(directory, ["metadata.json", "samples.jsonl", "stdout.log", "stderr.log", "artifacts/route.txt"]);
  const result = await validateResultDirectory(directory);
  assert.deepEqual(result.errors, []);
});

test("rejects a summary that does not match the selected workload", async () => {
  const directory = await fixture();
  await writeFile(join(directory, "samples.jsonl"), `${JSON.stringify({ type: "sample" })}\n${JSON.stringify({ type: "summary", workload: "bulk-tcp" })}\n`);
  const result = await validateResultDirectory(directory);
  assert.ok(result.errors.some((error) => error.startsWith("samples.jsonl summary[0].workload:")));
});
