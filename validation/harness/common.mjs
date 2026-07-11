import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const harnessRoot = dirname(fileURLToPath(import.meta.url));
const MAX_HEADER_BYTES = 16 * 1024;
const MAX_PAYLOAD_BYTES = 16 * 1024 * 1024;
export const HARNESS_VERSION = "0.1.0";

export function deterministicPayload(sequence, length) {
  const payload = Buffer.allocUnsafe(length);
  for (let index = 0; index < length; index += 1) {
    payload[index] = (sequence * 31 + index * 17) & 0xff;
  }
  return payload;
}

export function encodeFrame(header, payload = Buffer.alloc(0)) {
  const encodedHeader = Buffer.from(JSON.stringify({ ...header, payload_length: payload.length }));
  if (encodedHeader.length > MAX_HEADER_BYTES || payload.length > MAX_PAYLOAD_BYTES) {
    throw new Error("frame exceeds harness limits");
  }
  const prefix = Buffer.allocUnsafe(4);
  prefix.writeUInt32BE(encodedHeader.length);
  return Buffer.concat([prefix, encodedHeader, payload]);
}

export class FrameDecoder {
  #buffer = Buffer.alloc(0);

  push(chunk) {
    this.#buffer = Buffer.concat([this.#buffer, chunk]);
    const frames = [];
    while (this.#buffer.length >= 4) {
      const headerLength = this.#buffer.readUInt32BE(0);
      if (headerLength === 0 || headerLength > MAX_HEADER_BYTES) throw new Error("invalid frame header length");
      if (this.#buffer.length < 4 + headerLength) break;
      const header = JSON.parse(this.#buffer.subarray(4, 4 + headerLength).toString("utf8"));
      const payloadLength = header.payload_length;
      if (!Number.isInteger(payloadLength) || payloadLength < 0 || payloadLength > MAX_PAYLOAD_BYTES) {
        throw new Error("invalid frame payload length");
      }
      const frameLength = 4 + headerLength + payloadLength;
      if (this.#buffer.length < frameLength) break;
      frames.push({ header, payload: this.#buffer.subarray(4 + headerLength, frameLength) });
      this.#buffer = this.#buffer.subarray(frameLength);
    }
    return frames;
  }
}

export function percentile(values, fraction) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
}

export function latencySummary(values) {
  return {
    count: values.length,
    min_ms: values.length ? Math.min(...values) : null,
    median_ms: percentile(values, 0.5),
    p95_ms: percentile(values, 0.95),
    max_ms: values.length ? Math.max(...values) : null,
  };
}

export function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

export async function loadWorkload(id) {
  const path = join(harnessRoot, "..", "manifests", "workloads.json");
  const manifest = JSON.parse(await readFile(path, "utf8"));
  const workload = manifest.workloads.find((candidate) => candidate.id === id);
  if (!workload) throw new Error(`unknown workload: ${id}`);
  return { manifestVersion: manifest.manifest_version, workload };
}

export function parseOptions(argumentsList) {
  const options = {};
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (!argument.startsWith("--")) throw new Error(`unexpected argument: ${argument}`);
    const key = argument.slice(2);
    const value = argumentsList[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`missing value for --${key}`);
    options[key] = value;
    index += 1;
  }
  return options;
}
