#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const count = (text, marker) => text.split(marker).length - 1;
const needCount = (text, marker, expected, label) => {
  const actual = count(text, marker);
  if (actual !== expected) failures.push(`${label}: expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
};
// Action counts are about *which* action a step uses, not about one commit: the pin (a 40-character
// SHA) is enforced by verify:workflow-action-pins, and hard-coding a bumpable commit here reported a
// correctly pinned workflow as broken.
const needActionCount = (text, action, expected, label) => {
  const actual = text.split("\n").filter((line) => {
    const uses = /^\s*(?:-\s+)?uses:\s*(\S+)/.exec(line);
    return uses !== null && uses[1].startsWith(`${action}@`);
  }).length;
  if (actual !== expected) failures.push(`${label}: expected ${expected} use(s) of ${action}, found ${actual}`);
};

const files = {
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-release-composition-source.json",
  adminBuilder: "scripts/build/build-embedded-admin.sh",
  deploymentBuilder: "scripts/build/build-pages-inline-edit-deployment.sh",
  serverBuilder: "scripts/build/build-pages-inline-edit-server.sh",
  clientBuilder: "apps/storefront/scripts/build-pages-inline-edit-client.mjs",
  release: ".github/workflows/release.yml",
  infrastructure: ".github/workflows/release-infrastructure.yml",
  hardening: ".github/workflows/hardening-gates.yml",
  serverDocker: "apps/server/Dockerfile",
  runtimeDocker: "apps/server/Dockerfile.release",
  standaloneAdminDocker: "apps/admin/Dockerfile",
  approval: "scripts/verify/verify-release-infrastructure-approval.mjs",
  supplyChain: "scripts/verify/verify-release-supply-chain-contract.mjs",
  readinessGuard: "scripts/verify/verify-release-readiness-contract.mjs",
  readinessChecklist: "docs/release/RELEASE_READINESS_CHECKLIST.md",
  localPlan: "crates/modules/rustok-pages/docs/implementation-plan.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(path.join(root, relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-release-composition] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const adminBuilder = read(files.adminBuilder);
const deploymentBuilder = read(files.deploymentBuilder);
const serverBuilder = read(files.serverBuilder);
const clientBuilder = read(files.clientBuilder);
const release = read(files.release);
const infrastructure = read(files.infrastructure);
const hardening = read(files.hardening);
const serverDocker = read(files.serverDocker);
const runtimeDocker = read(files.runtimeDocker);
const standaloneAdminDocker = read(files.standaloneAdminDocker);
const approval = read(files.approval);
const supplyChain = read(files.supplyChain);
const readinessGuard = read(files.readinessGuard);
const readinessChecklist = read(files.readinessChecklist);
const localPlan = read(files.localPlan);
const plan = read(files.plan);

if (evidence.format !== "pages_inline_edit_release_composition_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_inline_edit_release_composition_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "release composition packet");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-release-composition] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-inline-edit-release-composition] PASS source_ready=true release_execution=pending docker_execution=pending browser=pending rollout=pending",
);
