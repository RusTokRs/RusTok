#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..", "..");

const evidencePath =
  "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json";
const pagesManifestPath = "crates/modules/rustok-pages/rustok-module.toml";
const forumManifestPath = "crates/modules/rustok-forum/rustok-module.toml";
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing marker '${marker}'`);
}

function requireExact(values, expected, label) {
  if (!Array.isArray(values)) {
    failures.push(`${label}: expected array`);
    return;
  }
  if (values.length !== expected.length || values.some((value, index) => value !== expected[index])) {
    failures.push(`${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(values)}`);
  }
}

function parseJson(source, relativePath) {
  try {
    return JSON.parse(source);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

const evidence = parseJson(read(evidencePath), evidencePath);
const pagesManifest = read(pagesManifestPath);
const forumManifest = read(forumManifestPath);

if (evidence.schema_version !== 1) failures.push(`${evidencePath}: expected schema_version 1`);
if (evidence.artifact !== "pages_reference_consumer_gate_source") {
  failures.push(`${evidencePath}: artifact identity drifted`);
}
if (evidence.module_slug !== "pages" || evidence.provider_module !== "page-builder") {
  failures.push(`${evidencePath}: expected Pages -> Page Builder identity`);
}
if (evidence.mode !== "source_ready") failures.push(`${evidencePath}: mode must remain source_ready`);
if (evidence.execution_status !== "not_run_by_implementation_agent") {
  failures.push(`${evidencePath}: source packet must not claim maintainer execution`);
}
if (evidence.accepted !== false) failures.push(`${evidencePath}: source packet must not be accepted`);
if (evidence.gate?.id !== "pages_reference_consumer_gate") {
  failures.push(`${evidencePath}: gate id drifted`);
}
if (evidence.gate?.contract_version !== "1.1") {
  failures.push(`${evidencePath}: Pages gate must remain on Page Builder contract 1.1`);
}
requireExact(
  evidence.gate?.required_profiles,
  ["all_on", "publish_off", "preview_off", "builder_off"],
  `${evidencePath}: required profiles`,
);
if (evidence.current_boundary?.source_gate !== "ready") {
  failures.push(`${evidencePath}: source gate must be ready`);
}
if (evidence.current_boundary?.execution_gate !== "pending") {
  failures.push(`${evidencePath}: execution gate must remain pending`);
}
if (evidence.current_boundary?.forum_wave_blocker !== "pages_reference_consumer_gate") {
  failures.push(`${evidencePath}: Forum Wave blocker drifted`);
}
if (evidence.current_boundary?.provider_health !== "unobserved") {
  failures.push(`${evidencePath}: provider health must remain unobserved`);
}
if (evidence.current_boundary?.ffa_fba_promotion !== "not_claimed") {
  failures.push(`${evidencePath}: source packet must not promote FFA/FBA`);
}

, marker, actualizationPath);
}

for (const forbidden of [
  '"accepted": true',
  '"execution_status": "maintainer_verified"',
  '"provider_health": "healthy"',
  '"forum_wave": "accepted"',
]) {
  if (JSON.stringify(evidence).includes(forbidden)) {
    failures.push(`${evidencePath}: forbidden live/accepted claim '${forbidden}'`);
  }
}

if (failures.length > 0) {
  console.error("Pages reference-consumer gate source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Pages reference-consumer gate source verification passed");
