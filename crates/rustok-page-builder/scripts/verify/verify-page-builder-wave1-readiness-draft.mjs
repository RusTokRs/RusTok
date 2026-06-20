#!/usr/bin/env node

import fs from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const verifier = path.join(__dirname, "verify-page-builder-wave-evidence-packet.mjs");
const packetRelativePath =
  "crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json";
const packetPath = path.join(repoRoot, packetRelativePath);

function fail(message) {
  console.error("[verify-page-builder-wave1-readiness-draft] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function expect(value, message) {
  if (!value) {
    fail(message);
  }
}

const result = spawnSync(process.execPath, [verifier, packetRelativePath, "readiness_draft"], {
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

const packet = JSON.parse(fs.readFileSync(packetPath, "utf8"));

expect(packet.wave === "1", `expected wave=1, got ${packet.wave}`);
expect(
  packet.control_plane?.change_set?.tenant_id === "pending-wave1-pilot-tenant",
  "readiness draft must not name a real tenant before owner sign-off",
);
expect(
  String(packet.control_plane?.change_set?.change_set_id ?? "").startsWith(
    "draft:pages-wave1-readiness:",
  ),
  "readiness draft change_set_id must remain in draft namespace",
);
expect(
  Object.values(packet.observability?.metrics ?? {}).every((value) =>
    String(value).startsWith("readiness_draft_pending_tenant_measurement:"),
  ),
  "readiness draft metrics must keep pending tenant measurement markers",
);
expect(
  Object.values(packet.approvals ?? {}).every((value) =>
    String(value).startsWith("pending_wave1_readiness_signoff"),
  ),
  "readiness draft approvals must remain pending until real Wave 1 evidence lands",
);
expect(
  String(packet.rollback?.reason ?? "").includes("on hold"),
  "readiness draft rollback reason must explicitly keep Wave 1 on hold",
);
expect(
  (packet.waivers ?? []).length === 0,
  "readiness draft must not carry waivers",
);

console.log("[verify-page-builder-wave1-readiness-draft] PASS");
