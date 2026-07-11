import { createHash } from "node:crypto";
import { readdir, readFile, stat } from "node:fs/promises";
import { basename, relative, resolve, sep } from "node:path";

const RUN_NAME = /^(\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z)-[a-z0-9-]+$/;
const SHA256_LINE = /^([a-f0-9]{64})  (.+)$/;
const STATUSES = new Set(["completed", "failed", "interrupted", "invalid"]);

function presentString(value) {
  return typeof value === "string" && value.length > 0;
}

function semanticVersion(value) {
  return typeof value === "string" && /^\d+\.\d+\.\d+$/.test(value);
}

function add(errors, condition, location, message) {
  if (!condition) errors.push(`${location}: ${message}`);
}

async function parseJson(path, errors, location) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    errors.push(`${location}: cannot parse JSON: ${error.message}`);
    return null;
  }
}

async function regularFiles(root, directory = root) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await regularFiles(root, path)));
    else if (entry.isFile()) files.push(relative(root, path).split(sep).join("/"));
  }
  return files.sort();
}

function validateMetadata(metadata, directoryName, errors) {
  if (!metadata) return;
  add(errors, metadata.schema_version === 1, "metadata.schema_version", "must be 1");
  for (const name of ["networks", "workloads", "baselines"]) {
    add(errors, semanticVersion(metadata.manifests?.[name]), `metadata.manifests.${name}`, "must be a semantic version");
  }
  for (const name of ["baseline", "network", "workload"]) add(errors, presentString(metadata.selection?.[name]), `metadata.selection.${name}`, "is required");
  if (["baseline", "network", "workload"].every((name) => presentString(metadata.selection?.[name]))) {
    const timestamp = directoryName.slice(0, 20);
    const expected = `${timestamp}-${metadata.selection.baseline}-${metadata.selection.network}-${metadata.selection.workload}`;
    add(errors, directoryName === expected, "directory", `name must match metadata selection (${expected})`);
  }
  add(errors, presentString(metadata.baseline?.version) || presentString(metadata.baseline?.revision), "metadata.baseline", "version or revision is required");
  for (const host of ["client", "server"]) {
    for (const field of ["hardware", "os", "kernel"]) {
      add(errors, presentString(metadata.environment?.[host]?.[field]), `metadata.environment.${host}.${field}`, "is required");
    }
  }
  add(errors, metadata.environment?.tools && typeof metadata.environment.tools === "object" && Object.keys(metadata.environment.tools).length > 0, "metadata.environment.tools", "must contain at least one pinned tool");
  add(errors, Array.isArray(metadata.command?.argv) && metadata.command.argv.length > 0 && metadata.command.argv.every(presentString), "metadata.command.argv", "must be a non-empty string array");
  add(errors, /^[a-f0-9]{64}$/.test(metadata.command?.configuration_sha256 ?? ""), "metadata.command.configuration_sha256", "must be a SHA-256 digest");
  const started = Date.parse(metadata.started_at);
  const ended = Date.parse(metadata.ended_at);
  add(errors, Number.isFinite(started), "metadata.started_at", "must be an ISO-8601 timestamp");
  if (Number.isFinite(started)) {
    const directoryTimestamp = directoryName.slice(0, 20).replace(/T(\d{2})-(\d{2})-(\d{2})Z$/, "T$1:$2:$3Z");
    const directoryStarted = Date.parse(directoryTimestamp);
    add(errors, Number.isFinite(directoryStarted), "directory", "timestamp must be a valid UTC date");
    add(errors, !Number.isFinite(directoryStarted) || new Date(started).toISOString().slice(0, 19) === new Date(directoryStarted).toISOString().slice(0, 19), "metadata.started_at", "must match the directory timestamp to the second");
  }
  add(errors, Number.isFinite(ended), "metadata.ended_at", "must be an ISO-8601 timestamp");
  add(errors, !Number.isFinite(started) || !Number.isFinite(ended) || ended >= started, "metadata.ended_at", "must not precede started_at");
  add(errors, Number.isInteger(metadata.trial_count) && metadata.trial_count > 0, "metadata.trial_count", "must be a positive integer");
  add(errors, Array.isArray(metadata.random_seeds) && metadata.random_seeds.length === metadata.trial_count, "metadata.random_seeds", "length must equal trial_count");
  add(errors, STATUSES.has(metadata.status), "metadata.status", "must be completed, failed, interrupted, or invalid");
  add(errors, Array.isArray(metadata.deviations), "metadata.deviations", "must be an array");
  for (const [index, deviation] of (metadata.deviations ?? []).entries()) {
    add(errors, deviation && typeof deviation === "object" && presentString(deviation.reason), `metadata.deviations[${index}].reason`, "is required");
  }
  if (metadata.status !== "completed") add(errors, presentString(metadata.status_reason), "metadata.status_reason", "is required unless status is completed");
}

async function validateSamples(path, metadata, errors) {
  let lines;
  try {
    lines = (await readFile(path, "utf8")).split(/\r?\n/).filter((line) => line.length > 0);
  } catch (error) {
    errors.push(`samples.jsonl: cannot read: ${error.message}`);
    return;
  }
  const records = [];
  for (const [index, line] of lines.entries()) {
    try {
      records.push(JSON.parse(line));
    } catch (error) {
      errors.push(`samples.jsonl:${index + 1}: invalid JSON: ${error.message}`);
    }
  }
  const summaries = records.filter((record) => record.type === "summary");
  add(errors, records.every((record) => record.type === "sample" || record.type === "summary"), "samples.jsonl", "records must have type sample or summary");
  add(errors, summaries.length > 0, "samples.jsonl", "must contain at least one summary record");
  add(errors, records.length === 0 || records.at(-1)?.type === "summary", "samples.jsonl", "summary must be the final record");
  if (metadata) {
    add(errors, summaries.length === metadata.trial_count, "samples.jsonl", "summary count must equal metadata.trial_count");
    for (const [index, summary] of summaries.entries()) {
      add(errors, summary.workload === metadata.selection?.workload, `samples.jsonl summary[${index}].workload`, "must match metadata selection");
    }
  }
}

async function validateChecksums(directory, errors) {
  let source;
  try {
    source = await readFile(resolve(directory, "checksums.sha256"), "utf8");
  } catch (error) {
    errors.push(`checksums.sha256: cannot read: ${error.message}`);
    return;
  }
  const declared = new Map();
  for (const [index, line] of source.split(/\r?\n/).filter(Boolean).entries()) {
    const match = SHA256_LINE.exec(line);
    if (!match) {
      errors.push(`checksums.sha256:${index + 1}: expected '<sha256>  <relative path>'`);
      continue;
    }
    const [, digest, name] = match;
    const path = resolve(directory, name);
    if (name === "checksums.sha256" || path === directory || !path.startsWith(`${directory}${sep}`)) {
      errors.push(`checksums.sha256:${index + 1}: unsafe or self-referential path ${name}`);
      continue;
    }
    if (declared.has(name)) errors.push(`checksums.sha256:${index + 1}: duplicate path ${name}`);
    declared.set(name, digest);
  }
  const actualFiles = (await regularFiles(directory)).filter((name) => name !== "checksums.sha256");
  for (const name of actualFiles) {
    if (!declared.has(name)) {
      errors.push(`checksums.sha256: missing entry for ${name}`);
      continue;
    }
    const digest = createHash("sha256").update(await readFile(resolve(directory, name))).digest("hex");
    if (declared.get(name) !== digest) errors.push(`checksums.sha256: digest mismatch for ${name}`);
  }
  for (const name of declared.keys()) {
    if (!actualFiles.includes(name)) errors.push(`checksums.sha256: entry refers to missing file ${name}`);
  }
}

export async function validateResultDirectory(input) {
  const directory = resolve(input);
  const errors = [];
  const directoryName = basename(directory);
  if (!RUN_NAME.test(directoryName)) {
    return { directory, errors: ["directory: name must be <UTC timestamp>-<baseline>-<network>-<workload>"] };
  }
  const metadata = await parseJson(resolve(directory, "metadata.json"), errors, "metadata.json");
  validateMetadata(metadata, directoryName, errors);
  await validateSamples(resolve(directory, "samples.jsonl"), metadata, errors);
  for (const required of ["stdout.log", "stderr.log"]) {
    try {
      await readFile(resolve(directory, required));
    } catch (error) {
      errors.push(`${required}: cannot read: ${error.message}`);
    }
  }
  try {
    add(errors, (await stat(resolve(directory, "artifacts"))).isDirectory(), "artifacts", "must be a directory");
  } catch (error) {
    errors.push(`artifacts: cannot inspect: ${error.message}`);
  }
  await validateChecksums(directory, errors);
  return { directory, errors };
}
