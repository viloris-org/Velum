import dgram from "node:dgram";
import { performance } from "node:perf_hooks";
import { deterministicPayload, latencySummary, sleep } from "./common.mjs";

const MAGIC = Buffer.from("VLM0");

export function encodeDatagram(sequence, responseBytes, payload = Buffer.alloc(0)) {
  const header = Buffer.allocUnsafe(12);
  MAGIC.copy(header);
  header.writeUInt32BE(sequence, 4);
  header.writeUInt32BE(responseBytes, 8);
  return Buffer.concat([header, payload]);
}

export function decodeDatagram(message) {
  if (message.length < 12 || !message.subarray(0, 4).equals(MAGIC)) throw new Error("invalid harness datagram");
  return { sequence: message.readUInt32BE(4), responseBytes: message.readUInt32BE(8), payload: message.subarray(12) };
}

function bindClient() {
  const socket = dgram.createSocket("udp4");
  return new Promise((resolve, reject) => {
    socket.once("error", reject);
    socket.bind(0, "0.0.0.0", () => {
      socket.off("error", reject);
      resolve(socket);
    });
  });
}

function send(socket, message, target) {
  return new Promise((resolve, reject) => {
    socket.send(message, target.udpPort, target.host, (error) => error ? reject(error) : resolve());
  });
}

export async function runRealtime(workload, target, durationSeconds) {
  const socket = await bindClient();
  const sentAt = new Map();
  const samples = [];
  const seen = new Set();
  let duplicateCount = 0;
  let reorderedCount = 0;
  let highestSequence = -1;
  socket.on("message", (message) => {
    try {
      const response = decodeDatagram(message);
      if (seen.has(response.sequence)) duplicateCount += 1;
      if (response.sequence < highestSequence) reorderedCount += 1;
      highestSequence = Math.max(highestSequence, response.sequence);
      seen.add(response.sequence);
      const started = sentAt.get(response.sequence);
      if (started !== undefined) samples.push({ sequence: response.sequence, latency_ms: performance.now() - started });
    } catch {
      // Non-harness datagrams are ignored and remain visible as loss.
    }
  });
  const intervalMs = 1000 / workload.generator.packets_per_second;
  const deadline = performance.now() + durationSeconds * 1000;
  let sequence = 0;
  try {
    while (performance.now() < deadline) {
      const started = performance.now();
      sentAt.set(sequence, started);
      await send(socket, encodeDatagram(sequence, workload.generator.payload_bytes), target);
      sequence += 1;
      const wait = intervalMs - (performance.now() - started);
      if (wait > 0) await sleep(wait);
    }
    await sleep(Math.min(1000, durationSeconds * 1000));
  } finally {
    socket.close();
  }
  return {
    samples,
    summary: {
      one_way_latency_note: "Local harness records round-trip latency; synchronized hosts are required for one-way latency.",
      round_trip_latency: latencySummary(samples.map((sample) => sample.latency_ms)),
      sent_datagrams: sequence,
      received_datagrams: samples.length,
      loss_ratio: sequence === 0 ? 0 : (sequence - seen.size) / sequence,
      duplicate_ratio: sequence === 0 ? 0 : duplicateCount / sequence,
      reorder_ratio: sequence === 0 ? 0 : reorderedCount / sequence,
    },
  };
}

export async function runDnsLike(workload, target, durationSeconds) {
  const socket = await bindClient();
  const pending = new Map();
  socket.on("message", (message) => {
    try {
      const response = decodeDatagram(message);
      pending.get(response.sequence)?.(response);
    } catch {
      // Ignore traffic that is not part of this harness.
    }
  });
  let nextSequence = 0;
  async function query() {
    const sequence = nextSequence++;
    const started = performance.now();
    for (let attempt = 1; attempt <= workload.generator.max_attempts; attempt += 1) {
      const response = new Promise((resolve) => pending.set(sequence, resolve));
      await send(socket, encodeDatagram(sequence, workload.generator.response_bytes, deterministicPayload(sequence, workload.generator.request_bytes)), target);
      const result = await Promise.race([response, sleep(workload.generator.timeout_ms).then(() => null)]);
      if (result) {
        pending.delete(sequence);
        return { sequence, latency_ms: performance.now() - started, attempt_count: attempt, timed_out: false };
      }
    }
    pending.delete(sequence);
    return { sequence, latency_ms: null, attempt_count: workload.generator.max_attempts, timed_out: true };
  }
  const samples = [];
  const deadline = performance.now() + durationSeconds * 1000;
  try {
    while (performance.now() < deadline) {
      const batch = Array.from({ length: workload.concurrency }, () => query());
      samples.push(...await Promise.all(batch));
    }
  } finally {
    socket.close();
  }
  const successfulLatencies = samples.filter((sample) => !sample.timed_out).map((sample) => sample.latency_ms);
  return {
    samples,
    summary: {
      response_latency: latencySummary(successfulLatencies),
      query_count: samples.length,
      timeout_ratio: samples.length === 0 ? 0 : samples.filter((sample) => sample.timed_out).length / samples.length,
      total_attempts: samples.reduce((total, sample) => total + sample.attempt_count, 0),
    },
  };
}
