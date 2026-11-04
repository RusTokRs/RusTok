#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  coreRollout: "crates/modules/rustok-page-builder/src/rollout.rs",
  status: "crates/modules/rustok-page-builder/admin/src/provider_status.rs",
  facade: "crates/modules/rustok-page-builder/admin/src/transport/mod.rs",
  canvas: "crates/modules/rustok-page-builder/admin/src/editor/modular_canvas.rs",
  policyPanel: "crates/modules/rustok-page-builder/admin/src/editor/capability_controls.rs",
  preview: "crates/modules/rustok-page-builder/admin/src/editor/server_preview.rs",
  enLocale: "crates/modules/rustok-page-builder/admin/locales/en.ftl",
  ruLocale: "crates/modules/rustok-page-builder/admin/locales/ru.ftl",
  pagesFacade: "crates/modules/rustok-pages/admin/src/builder.rs",
  pagesRollout: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
  evidence: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-admin-provider-status-source.json",
  consumerEvidence: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  localPlan: "crates/modules/rustok-page-builder/docs/implementation-plan.md",
  centralPlan: "docs/modules/page-builder-implementation-plan.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-page-builder-admin-provider-status] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const consumerEvidence = JSON.parse(sources.consumerEvidence);
if (evidence.format !== "page_builder_admin_provider_status_source_v1") failures.push("evidence format drifted");
if (evidence.status !== "page_builder_admin_provider_status_source_unvalidated") failures.push("evidence status drifted");
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push("execution evidence must remain empty");
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("validation fields must remain false");
}
, marker, "provider status actualization");
, marker, "Pages provider-health consumer continuation");
for (const marker of ["admin-provider-status-source-ready", "provider-health", "observed health"]) need(sources.localPlan, marker, "local plan actualization");
for (const marker of ["Provider status/degraded controls", "observed provider-health evidence", "next production consumer"]) need(sources.centralPlan, marker, "central plan actualization");

for (const marker of ["fallback editor", "ProviderHealthSnapshot::evaluate(ProviderSloObservations::default())"]) {
  forbid(sources.canvas, marker, "admin canvas must not fabricate fallback/health");
  forbid(sources.pagesFacade, marker, "Pages facade must not fabricate fallback/health");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-admin-provider-status] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-page-builder-admin-provider-status] PASS source_ready=true runtime_narrowing=shared execution=pending");
