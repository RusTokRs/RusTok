#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const verifier = path.join(__dirname, "verify-page-builder-wave-evidence-packet.mjs");
const packet = "crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json";

const result = spawnSync(process.execPath, [verifier, packet, "readiness_draft"], {
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);
