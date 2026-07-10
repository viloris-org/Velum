import assert from "node:assert/strict";
import test from "node:test";
import { deterministicPayload, encodeFrame, FrameDecoder, loadWorkload, percentile } from "./common.mjs";
import { startHarnessServer } from "./server-lib.mjs";
import { runBulk, runIdle, runInteractive } from "./tcp.mjs";
import { decodeDatagram, encodeDatagram, runDnsLike, runRealtime } from "./udp.mjs";

test("frame decoder handles fragmented input", () => {
  const frame = encodeFrame({ op: "test" }, Buffer.from("payload"));
  const decoder = new FrameDecoder();
  assert.deepEqual(decoder.push(frame.subarray(0, 3)), []);
  const decoded = decoder.push(frame.subarray(3));
  assert.equal(decoded[0].header.op, "test");
  assert.equal(decoded[0].payload.toString(), "payload");
});

test("datagram codec preserves sequence and payload", () => {
  const encoded = encodeDatagram(7, 32, deterministicPayload(7, 8));
  const decoded = decodeDatagram(encoded);
  assert.equal(decoded.sequence, 7);
  assert.equal(decoded.responseBytes, 32);
  assert.equal(decoded.payload.length, 8);
});

test("percentile uses nearest-rank semantics", () => {
  assert.equal(percentile([4, 1, 3, 2], 0.5), 2);
  assert.equal(percentile([4, 1, 3, 2], 0.95), 4);
  assert.equal(percentile([], 0.95), null);
});

test("all workload classes complete against local server", async () => {
  const server = await startHarnessServer({ host: "127.0.0.1", tcpPort: 0, udpPort: 0 });
  const target = { host: "127.0.0.1", tcpPort: server.tcpPort, udpPort: server.udpPort };
  try {
    const interactive = await loadWorkload("interactive-tcp");
    const tcpResult = await runInteractive(interactive.workload, target, 0.3);
    assert.equal(tcpResult.summary.missing_bytes, 0);
    assert.ok(tcpResult.samples.length > 0);

    const bulk = await loadWorkload("bulk-tcp");
    const bulkResult = await runBulk(bulk.workload, target, 0.2);
    assert.equal(bulkResult.summary.directions.length, 2);
    assert.ok(bulkResult.summary.directions.every((direction) => direction.payload_bytes > 0));

    const realtime = await loadWorkload("realtime-udp");
    const udpResult = await runRealtime(realtime.workload, target, 0.3);
    assert.equal(udpResult.summary.loss_ratio, 0);
    assert.ok(udpResult.samples.length > 0);

    const dnsLike = await loadWorkload("dns-like-udp");
    const dnsResult = await runDnsLike(dnsLike.workload, target, 0.1);
    assert.equal(dnsResult.summary.timeout_ratio, 0);
    assert.ok(dnsResult.summary.query_count >= dnsLike.workload.concurrency);

    const idle = await loadWorkload("idle-mobile");
    const idleResult = await runIdle(idle.workload, target, 0.05);
    assert.equal(idleResult.summary.idle_bytes_sent, 0);
    assert.equal(idleResult.summary.idle_bytes_received, 0);
  } finally {
    await server.close();
  }
});
