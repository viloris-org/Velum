import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const ready = process.argv.includes("--ready");
const errors = [];

async function load(name) {
  const path = join(root, "manifests", name);
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    errors.push(`${name}: cannot parse JSON: ${error.message}`);
    return {};
  }
}

function requireValue(condition, location, message) {
  if (!condition) errors.push(`${location}: ${message}`);
}

function requireVersion(document, name) {
  requireValue(document.schema_version === 1, name, "schema_version must be 1");
  requireValue(
    typeof document.manifest_version === "string" && /^\d+\.\d+\.\d+$/.test(document.manifest_version),
    name,
    "manifest_version must be a semantic version",
  );
}

function uniqueIds(items, location) {
  const ids = new Set();
  for (const [index, item] of items.entries()) {
    const itemLocation = `${location}[${index}]`;
    requireValue(typeof item.id === "string" && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(item.id), itemLocation, "id must be kebab-case");
    requireValue(!ids.has(item.id), itemLocation, `duplicate id ${item.id}`);
    ids.add(item.id);
  }
  return ids;
}

const [networks, workloads, baselines] = await Promise.all([
  load("networks.json"),
  load("workloads.json"),
  load("baselines.json"),
]);
let harnessPackage = {};
try {
  harnessPackage = JSON.parse(await readFile(join(root, "harness", "package.json"), "utf8"));
} catch (error) {
  errors.push(`harness/package.json: cannot parse JSON: ${error.message}`);
}

requireVersion(networks, "networks.json");
requireVersion(workloads, "workloads.json");
requireVersion(baselines, "baselines.json");
requireValue(harnessPackage.name === "velum-validation-harness", "harness/package.json", "unexpected package name");
requireValue(/^\d+\.\d+\.\d+$/.test(harnessPackage.version ?? ""), "harness/package.json", "version must be semantic");
requireValue(baselines.reference_environment?.workload_tool === harnessPackage.name, "baselines.json.reference_environment.workload_tool", "must match the harness package name");
requireValue(baselines.reference_environment?.workload_tool_version === harnessPackage.version, "baselines.json.reference_environment.workload_tool_version", "must match the harness package version");

requireValue(networks.reference_path?.id === "stable-reference", "networks.json.reference_path", "stable-reference path is required");
requireValue(Array.isArray(networks.scenarios) && networks.scenarios.length > 0, "networks.json.scenarios", "must be a non-empty array");
const networkItems = [networks.reference_path, ...(networks.scenarios ?? [])].filter(Boolean);
const networkIds = uniqueIds(networkItems, "networks");
for (const scenario of networks.scenarios ?? []) {
  requireValue(typeof scenario.description === "string" && scenario.description.length > 0, `network ${scenario.id}`, "description is required");
  requireValue(Number.isInteger(scenario.trigger_after_ms) && scenario.trigger_after_ms >= 0, `network ${scenario.id}`, "trigger_after_ms must be a non-negative integer");
  requireValue(Array.isArray(scenario.impairments) && scenario.impairments.length > 0, `network ${scenario.id}`, "at least one impairment is required");
}

requireValue(Array.isArray(workloads.workloads) && workloads.workloads.length > 0, "workloads.json.workloads", "must be a non-empty array");
const workloadIds = uniqueIds(workloads.workloads ?? [], "workloads");
const requiredWorkloads = ["interactive-tcp", "bulk-tcp", "realtime-udp", "dns-like-udp", "idle-mobile"];
for (const id of requiredWorkloads) {
  requireValue(workloadIds.has(id), "workloads.json", `required workload ${id} is missing`);
}
for (const workload of workloads.workloads ?? []) {
  requireValue(Number.isInteger(workload.duration_seconds) && workload.duration_seconds > 0, `workload ${workload.id}`, "duration_seconds must be a positive integer");
  requireValue(Number.isInteger(workload.concurrency) && workload.concurrency > 0, `workload ${workload.id}`, "concurrency must be a positive integer");
  requireValue(typeof workload.generator?.kind === "string", `workload ${workload.id}`, "generator.kind is required");
  requireValue(Array.isArray(workload.measurements) && workload.measurements.length > 0, `workload ${workload.id}`, "measurements must be non-empty");
}
for (const [index, row] of (workloads.matrix ?? []).entries()) {
  requireValue(networkIds.has(row.network), `workloads.json.matrix[${index}]`, `unknown network ${row.network}`);
  requireValue(Array.isArray(row.workloads) && row.workloads.length > 0, `workloads.json.matrix[${index}]`, "workloads must be non-empty");
  for (const id of row.workloads ?? []) {
    requireValue(workloadIds.has(id), `workloads.json.matrix[${index}]`, `unknown workload ${id}`);
  }
}
for (const id of networkIds) {
  requireValue((workloads.matrix ?? []).some((row) => row.network === id), "workloads.json.matrix", `network ${id} has no workload coverage`);
}

requireValue(Array.isArray(baselines.baselines), "baselines.json.baselines", "must be an array");
const baselineIds = uniqueIds(baselines.baselines ?? [], "baselines");
for (const id of ["masque", "anytls", "hysteria2", "conventional"]) {
  requireValue(baselineIds.has(id), "baselines.json", `required baseline ${id} is missing`);
}
if (ready) {
  for (const baseline of baselines.baselines ?? []) {
    requireValue(baseline.status === "pinned", `baseline ${baseline.id}`, "status must be pinned for execution");
    requireValue(typeof baseline.implementation === "string" && baseline.implementation.length > 0, `baseline ${baseline.id}`, "implementation must be selected");
    requireValue(typeof baseline.source_url === "string" && baseline.source_url.startsWith("https://"), `baseline ${baseline.id}`, "source_url must be HTTPS");
    requireValue(Boolean(baseline.version || baseline.revision), `baseline ${baseline.id}`, "version or immutable revision is required");
  }
  for (const [key, value] of Object.entries(baselines.reference_environment ?? {})) {
    requireValue(typeof value === "string" && value.length > 0, `baselines.json.reference_environment.${key}`, "must be pinned for execution");
  }
}

if (errors.length > 0) {
  console.error(`Validation manifest checks failed (${errors.length}):`);
  for (const error of errors) console.error(`- ${error}`);
  process.exitCode = 1;
} else {
  console.log(`Validation manifests are structurally valid${ready ? " and execution-ready" : ""}.`);
}
