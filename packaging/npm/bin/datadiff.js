#!/usr/bin/env node
// Thin shim: hand every argument to the native binary and mirror its exit
// code, which callers rely on (1 = changes found, 2 = bad input).
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const binary = path.join(
  __dirname,
  process.platform === "win32" ? "datadiff.exe" : "datadiff",
);
const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  console.error(`datadiff: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
