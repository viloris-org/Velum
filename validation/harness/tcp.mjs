import net from "node:net";
import { performance } from "node:perf_hooks";
import { deterministicPayload, encodeFrame, FrameDecoder, latencySummary, sleep } from "./common.mjs";

class TcpPeer {
  #nextRequestId = 1;
  #pending = new Map();
  bytesSent = 0;
  bytesReceived = 0;

  constructor(socket) {
    this.socket = socket;
    const decoder = new FrameDecoder();
    socket.on("data", (chunk) => {
      this.bytesReceived += chunk.length;
      try {
        for (const frame of decoder.push(chunk)) {
          const pending = this.#pending.get(frame.header.request_id);
          if (!pending) continue;
          this.#pending.delete(frame.header.request_id);
          pending.resolve(frame);
        }
      } catch (error) {
        this.#rejectAll(error);
        socket.destroy(error);
      }
    });
    socket.on("error", (error) => this.#rejectAll(error));
    socket.on("close", () => this.#rejectAll(new Error("TCP peer closed")));
  }

  request(header, payload) {
    const requestId = this.#nextRequestId++;
    const frame = encodeFrame({ ...header, request_id: requestId }, payload);
    this.bytesSent += frame.length;
    return new Promise((resolve, reject) => {
      this.#pending.set(requestId, { resolve, reject });
      this.socket.write(frame, (error) => {
        if (!error) return;
        this.#pending.delete(requestId);
        reject(error);
      });
    });
  }

  #rejectAll(error) {
    for (const pending of this.#pending.values()) pending.reject(error);
    this.#pending.clear();
  }

  close() {
    this.socket.destroy();
  }
}

async function connect(host, port) {
  const socket = net.createConnection({ host, port });
  await new Promise((resolve, reject) => {
    socket.once("connect", resolve);
    socket.once("error", reject);
  });
  socket.setNoDelay(true);
  return new TcpPeer(socket);
}

export async function runInteractive(workload, target, durationSeconds) {
  const peer = await connect(target.host, target.tcpPort);
  const samples = [];
  let sequence = 0;
  let missingBytes = 0;
  const deadline = performance.now() + durationSeconds * 1000;
  try {
    while (performance.now() < deadline) {
      const started = performance.now();
      const frame = await peer.request(
        { op: "exchange", sequence, response_bytes: workload.generator.response_bytes },
        deterministicPayload(sequence, workload.generator.request_bytes),
      );
      const expected = deterministicPayload(sequence, workload.generator.response_bytes);
      if (frame.header.sequence !== sequence || !frame.payload.equals(expected)) missingBytes += expected.length;
      samples.push({ sequence, latency_ms: performance.now() - started });
      sequence += 1;
      const wait = workload.generator.interval_ms - (performance.now() - started);
      if (wait > 0) await sleep(wait);
    }
  } finally {
    peer.close();
  }
  return {
    samples,
    summary: {
      request_latency: latencySummary(samples.map((sample) => sample.latency_ms)),
      disconnect_count: 0,
      missing_bytes: missingBytes,
      duplicate_bytes: 0,
      bytes_sent: peer.bytesSent,
      bytes_received: peer.bytesReceived,
    },
  };
}

async function runBulkDirection(peer, direction, chunkBytes, durationMs) {
  const started = performance.now();
  const deadline = started + durationMs;
  let sequence = 0;
  let payloadBytes = 0;
  while (performance.now() < deadline) {
    const requests = [];
    for (let windowIndex = 0; windowIndex < 32 && performance.now() < deadline; windowIndex += 1) {
      const payload = direction === "upload" ? deterministicPayload(sequence, chunkBytes) : Buffer.alloc(0);
      requests.push(peer.request({ op: direction, sequence, response_bytes: chunkBytes }, payload));
      sequence += 1;
    }
    const responses = await Promise.all(requests);
    for (const response of responses) {
      if (!response.header.ok) throw new Error(`${direction} payload verification failed`);
      if (direction === "download") {
        const expected = deterministicPayload(response.header.sequence, chunkBytes);
        if (!response.payload.equals(expected)) throw new Error("download payload verification failed");
      }
      payloadBytes += chunkBytes;
    }
  }
  const elapsedMs = performance.now() - started;
  return { direction, payload_bytes: payloadBytes, elapsed_ms: elapsedMs, throughput_mbit_s: payloadBytes * 8 / elapsedMs / 1000 };
}

export async function runBulk(workload, target, durationSeconds) {
  const peer = await connect(target.host, target.tcpPort);
  try {
    const perDirectionMs = durationSeconds * 500;
    const samples = [];
    for (const direction of workload.generator.directions) {
      samples.push(await runBulkDirection(peer, direction, workload.generator.chunk_bytes, perDirectionMs));
    }
    return {
      samples,
      summary: {
        directions: samples,
        missing_bytes: 0,
        duplicate_bytes: 0,
        bytes_sent: peer.bytesSent,
        bytes_received: peer.bytesReceived,
      },
    };
  } finally {
    peer.close();
  }
}

export async function runIdle(_workload, target, durationSeconds) {
  const memoryBefore = process.memoryUsage().rss;
  const peer = await connect(target.host, target.tcpPort);
  const started = performance.now();
  try {
    await peer.request({ op: "idle" }, Buffer.alloc(0));
    const setupBytesSent = peer.bytesSent;
    const setupBytesReceived = peer.bytesReceived;
    await sleep(durationSeconds * 1000);
    return {
      samples: [],
      summary: {
        elapsed_ms: performance.now() - started,
        idle_bytes_sent: peer.bytesSent - setupBytesSent,
        idle_bytes_received: peer.bytesReceived - setupBytesReceived,
        socket_count: 1,
        process_rss_delta_bytes: process.memoryUsage().rss - memoryBefore,
        radio_wakeups: null,
      },
    };
  } finally {
    peer.close();
  }
}
