import dgram from "node:dgram";
import net from "node:net";
import { deterministicPayload, encodeFrame, FrameDecoder } from "./common.mjs";
import { decodeDatagram, encodeDatagram } from "./udp.mjs";

function listen(server, port, host) {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, () => {
      server.off("error", reject);
      resolve();
    });
  });
}

export async function startHarnessServer({ host = "0.0.0.0", tcpPort = 9000, udpPort = 9001 } = {}) {
  const tcp = net.createServer((socket) => {
    socket.setNoDelay(true);
    const decoder = new FrameDecoder();
    socket.on("data", (chunk) => {
      try {
        for (const { header, payload } of decoder.push(chunk)) {
          let responsePayload = Buffer.alloc(0);
          let ok = true;
          if (header.op === "exchange" || header.op === "download") {
            responsePayload = deterministicPayload(header.sequence, header.response_bytes);
          } else if (header.op === "upload") {
            ok = payload.equals(deterministicPayload(header.sequence, payload.length));
          } else if (header.op !== "idle") {
            ok = false;
          }
          socket.write(encodeFrame({ request_id: header.request_id, sequence: header.sequence, ok }, responsePayload));
        }
      } catch (error) {
        socket.destroy(error);
      }
    });
  });

  const udp = dgram.createSocket("udp4");
  udp.on("message", (message, remote) => {
    try {
      const request = decodeDatagram(message);
      const response = encodeDatagram(request.sequence, request.responseBytes, deterministicPayload(request.sequence, request.responseBytes));
      udp.send(response, remote.port, remote.address);
    } catch {
      // Malformed datagrams receive no response.
    }
  });

  await Promise.all([
    listen(tcp, Number(tcpPort), host),
    new Promise((resolve, reject) => {
      udp.once("error", reject);
      udp.bind(Number(udpPort), host, () => {
        udp.off("error", reject);
        resolve();
      });
    }),
  ]);

  return {
    host,
    tcpPort: tcp.address().port,
    udpPort: udp.address().port,
    async close() {
      await Promise.all([
        new Promise((resolve) => tcp.close(resolve)),
        new Promise((resolve) => udp.close(resolve)),
      ]);
    },
  };
}
