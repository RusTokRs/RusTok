#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const contractPath = path.join(repoRoot, "crates", "rustok-page-builder", "contracts", "page-builder-adapter-seams.json");
const servicePath = path.join(repoRoot, "crates", "rustok-page-builder", "src", "service.rs");
const readmePath = path.join(repoRoot, "crates", "rustok-page-builder", "docs", "README.md");
const planPath = path.join(repoRoot, "crates", "rustok-page-builder", "docs", "implementation-plan.md");
const centralPlanPath = path.join(repoRoot, "docs", "modules", "page-builder-implementation-plan.md");

function fail(message) {
  console.error(`[verify-page-builder-adapter-seams] ${message}`);
  process.exit(1);
}

const contract = JSON.parse(fs.readFileSync(contractPath, "utf8"));
const service = fs.readFileSync(servicePath, "utf8");
const readme = fs.readFileSync(readmePath, "utf8");
const plan = fs.readFileSync(planPath, "utf8");
const centralPlan = fs.readFileSync(centralPlanPath, "utf8");

for (const marker of [
  "PageBuilderProjectStore",
  "PageBuilderRenderingAdapter",
  "ReferencePageBuilderRenderingAdapter",
  "AdapterBackedPageBuilderService",
  "PageBuilderAdapterCallStatus",
  "PageBuilderAdapterTelemetry",
  "NoopPageBuilderAdapterTelemetry",
  "extract_tree_nodes",
]) {
  if (!service.includes(marker)) fail(`service.rs missing ${marker}`);
}

for (const entrypoint of contract.canonical_entrypoints ?? []) {
  const source = entrypoint.includes("handle_page_builder_") ? readme + plan : service + readme + plan;
  if (!source.includes(entrypoint)) fail(`canonical entrypoint '${entrypoint}' is not documented or implemented`);
}

for (const seam of [contract.seams?.persistence?.trait, contract.seams?.rendering?.trait, contract.seams?.adapter_backed_service?.type]) {
  if (!seam || !service.includes(seam)) fail(`adapter seam '${seam}' is not implemented in service.rs`);
  if (!readme.includes(seam) || !plan.includes(seam)) fail(`adapter seam '${seam}' is not documented in local docs`);
}

for (const field of contract.seams?.adapter_backed_service?.operation_evidence_fields ?? []) {
  if (!service.includes(field)) fail(`adapter operation evidence field '${field}' is not implemented in service.rs`);
}

for (const status of contract.seams?.adapter_backed_service?.operation_statuses ?? []) {
  const variant = status
    .split("_")
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join("");
  if (!service.includes(variant)) fail(`adapter operation status '${status}' is not implemented in service.rs`);
  if (!readme.includes(status) || !plan.includes(status)) fail(`adapter operation status '${status}' is not documented in local docs`);
}

for (const blocked of contract.blocked_patterns ?? []) {
  if (!centralPlan.includes(blocked) && !readme.includes(blocked) && !plan.includes(blocked)) {
    fail(`blocked pattern '${blocked}' is not anchored in docs`);
  }
}

console.log("[verify-page-builder-adapter-seams] PASS");
