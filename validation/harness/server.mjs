#!/usr/bin/env node
import { parseOptions } from "./common.mjs";
import { startHarnessServer } from "./server-lib.mjs";

try {
  const options = parseOptions(process.argv.slice(2));
  const server = await startHarnessServer({
    host: options.host ?? "0.0.0.0",
    tcpPort: options["tcp-port"] ?? 9000,
    udpPort: options["udp-port"] ?? 9001,
  });
  console.error(JSON.stringify({ type: "ready", host: server.host, tcp_port: server.tcpPort, udp_port: server.udpPort }));
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.once(signal, async () => {
      await server.close();
      process.exit(0);
    });
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
