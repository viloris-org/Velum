#!/usr/bin/env node
import { readdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { validateResultDirectory } from "./validator.mjs";

const root = dirname(fileURLToPath(import.meta.url));
const explicit = process.argv.slice(2);
const directories = explicit.length > 0
  ? explicit.map(resolve)
  : (await readdir(root, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory())
      .map((entry) => resolve(root, entry.name));

let failures = 0;
for (const directory of directories) {
  const result = await validateResultDirectory(directory);
  if (result.errors.length === 0) continue;
  failures += 1;
  console.error(`${result.directory}: invalid result (${result.errors.length}):`);
  for (const error of result.errors) console.error(`- ${error}`);
}

if (failures > 0) process.exitCode = 1;
else console.log(`Stage 0 results valid (${directories.length} run directories).`);
