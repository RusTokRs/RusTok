#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..");

const sharedPlanPath = "docs/modules/pages-page-builder-parity-continuation-plan.md";
const localPlanPath = "crates/rustok-page-builder/docs/implementation-plan.md";
const centralPlanPath = "docs/modules/page-builder-implementation-plan.md";
const gatePath = "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json";
const forumManifestPath = "crates/rustok-forum/rustok-module.toml";
const forumWavePath = "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json";
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function parseJson(source, relativePath) {
  try {
    return JSON.parse(source);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

function requireText(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing marker '${marker}'`);
}

function forbidText(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: stale marker remains '${marker}'`);
}

function section(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return "";
  const end = endMarker ? source.indexOf(endMarker, start + startMarker.length) : -1;
  return end > start ? source.slice(start, end) : source.slice(start);
}

const sharedPlan = read(sharedPlanPath);
const localPlan = read(localPlanPath);
const centralPlan = read(centralPlanPath);
const gate = parseJson(read(gatePath), gatePath);
const forumManifest = read(forumManifestPath);
const forumWave = parseJson(read(forumWavePath), forumWavePath);

const sharedCurrent = section(
  sharedPlan,
  "## 2026-08-08 current source reconciliation",
  "## Rechecked merged cursor",
);
const sharedNext = section(sharedPlan, "## Next cursor", "## Maintainer validation");
const localOpen = section(localPlan, "## Open results", "## Verification");

for (const marker of [
  "forum-runtime-composition-source-ready",
  "forum-evidence-harness-source-ready",
  "pages-reference-consumer-gate-source-ready",
  "PR #3239",
  "PR #3247",
  "PR #3254",
  "PR #3264",
  "PR #3266",
  "PR #3274",
  "PR #3320",
  "adapter_state = \"fly_contract_ready\"",
  "preview_data_state = \"owner_preview_transport_ready\"",
  "property_data_state = \"owner_property_editor_ready\"",
  "accepted remains `false`",
]) {
  requireText(sharedCurrent, marker, `${sharedPlanPath}: current reconciliation`);
}

for (const stale of [
  "forum-fly-adapter-open",
  "adapter_state = \"pending\"",
  "because Forum has no real Fly component registry or `ContributionAdapter` yet",
  "The next contribution source cursor is the real Forum Fly adapter/component-registry slice",
]) {
  forbidText(sharedCurrent, stale, `${sharedPlanPath}: current reconciliation`);
}

for (const marker of [
  "Maintainer executes and accepts `pages_reference_consumer_gate`",
  "execute the retained Forum browser/runtime/deployment-attestation packets",
  "health remains `unobserved`",
  "Promote FFA/FBA only after",
]) {
  requireText(sharedNext, marker, `${sharedPlanPath}: next cursor`);
}

for (const marker of [
  "Forum is the second production consumer",
  "pages_reference_consumer_gate",
  "accepted = false",
  "execute the retained Forum browser/runtime/deployment-attestation",
]) {
  requireText(localPlan, marker, localPlanPath);
}
forbidText(
  localOpen,
  "Connect the next production consumer's concrete tenant-scoped store",
  `${localPlanPath}: open results`,
);

for (const marker of [
  "Forum Fly adapter/component registry: source-ready",
  "Forum owner preview transport/Pages host composition: source-ready",
  "Forum owner-backed property editing: source-ready",
]) {
  requireText(centralPlan, marker, centralPlanPath);
}

if (gate.artifact !== "pages_reference_consumer_gate_source") {
  failures.push(`${gatePath}: gate artifact identity drifted`);
}
if (gate.mode !== "source_ready" || gate.accepted !== false) {
  failures.push(`${gatePath}: source gate must remain source_ready and accepted=false`);
}
if (gate.current_boundary?.execution_gate !== "pending") {
  failures.push(`${gatePath}: execution gate must remain pending`);
}
if (gate.current_boundary?.provider_health !== "unobserved") {
  failures.push(`${gatePath}: provider health must remain unobserved`);
}
if (gate.current_boundary?.forum_wave_blocker !== "pages_reference_consumer_gate") {
  failures.push(`${gatePath}: Forum Wave blocker drifted`);
}

for (const marker of [
  'id = "rustok.forum.widget-catalog"',
  'id = "rustok.forum.widget-preview"',
  'adapter_state = "fly_contract_ready"',
  'preview_data_state = "owner_preview_transport_ready"',
  'property_data_state = "owner_property_editor_ready"',
  'persistence_owner = "forum"',
  'authorization_owner = "forum"',
]) {
  requireText(forumManifest, marker, forumManifestPath);
}

if (forumWave.mode !== "source_ready" || forumWave.execution_status !== "not_run_by_implementation_agent") {
  failures.push(`${forumWavePath}: Forum Wave must remain source_ready and unexecuted`);
}
if (forumWave.observed_run?.blocked_by !== "pages_reference_consumer_gate") {
  failures.push(`${forumWavePath}: observed Forum Wave must remain blocked by Pages gate`);
}

if (failures.length > 0) {
  console.error("Pages / Page Builder plan parity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Pages / Page Builder plan parity verification passed");
