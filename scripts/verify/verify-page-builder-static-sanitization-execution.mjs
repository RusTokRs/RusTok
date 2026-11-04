#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { pinnedActionProblem } from './lib/workflow-pins.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];
const files = {
  contract:
    "crates/modules/rustok-page-builder/contracts/evidence/page-builder-static-sanitization-execution-source.json",
  recorder: "scripts/evidence/record-page-builder-static-sanitization-execution.mjs",
  workflow: ".github/workflows/page-builder-static-sanitization-evidence.yml",
  registry: "crates/modules/rustok-page-builder/contracts/page-builder-fba-registry.json",
  sanitization: "crates/modules/rustok-page-builder/src/publish_sanitization.rs",
  policy: "crates/modules/rustok-page-builder/src/static_publish_policy.rs",
  resourceLimits: "crates/modules/rustok-page-builder/src/static_publish_resource_limits.rs",
  existingVerifier:
    "crates/modules/rustok-page-builder/scripts/verify/verify-page-builder-static-publish-resource-limits.mjs",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  try {
    const location = absolute(relativePath);
    const stat = fs.lstatSync(location);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      failures.push(`${relativePath}: must be a regular non-symlink file`);
      return "";
    }
    return fs.readFileSync(location, "utf8");
  } catch (error) {
    failures.push(`${relativePath}: ${error.message}`);
    return "";
  }
}

function json(relativePath) {
  const source = read(relativePath);
  try {
    const document = JSON.parse(source);
    if (document === null || typeof document !== "object" || Array.isArray(document)) {
      failures.push(`${relativePath}: JSON root must be an object`);
      return {};
    }
    return document;
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

function requireValue(condition, message) {
  if (!condition) failures.push(message);
}

function requireText(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

function forbidText(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden '${marker}'`);
}

function pointerValue(document, pointer) {
  if (typeof pointer !== "string" || !pointer.startsWith("/")) return undefined;
  let current = document;
  for (const rawToken of pointer.slice(1).split("/")) {
    const token = rawToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object" || !Object.hasOwn(current, token)) {
      return undefined;
    }
    current = current[token];
  }
  return current;
}

const contract = json(files.contract);
const registry = json(files.registry);
const recorder = read(files.recorder);
const workflow = read(files.workflow);
const sanitization = read(files.sanitization);
const policy = read(files.policy);
const resourceLimits = read(files.resourceLimits);
const existingVerifier = read(files.existingVerifier);

requireValue(
  contract.format === "page_builder_static_sanitization_execution_source_v1" &&
    contract.status === "source_ready_maintainer_execution_pending" &&
    contract.scope === "page_builder_fba_static_sanitization",
  `${files.contract}: identity drifted`,
);
requireValue(
  contract.target?.fba_registry === files.registry &&
    contract.target?.registry_required_status === "boundary_ready" &&
    contract.target?.executed_evidence_json_pointer ===
      "/provider/static_sanitization_contract/executed_evidence" &&
    contract.target?.required_before_value === "pending" &&
    contract.target?.registry_mutation_by_workflow === false,
  `${files.contract}: target boundary drifted`,
);
requireValue(registry.status === "boundary_ready", `${files.registry}: status must remain boundary_ready`);
requireValue(
  ["pending", "verified"].includes(pointerValue(registry, contract.target?.executed_evidence_json_pointer)),
  `${files.registry}: static sanitization executed_evidence is no longer pending; actualize this source gate`,
);

const expectedCommands = [
  "cargo test --locked -p rustok-page-builder --lib publish_sanitization::tests:: -- --nocapture",
  "cargo test --locked -p rustok-page-builder --lib static_publish_policy::tests:: -- --nocapture",
  "cargo test --locked -p rustok-page-builder --lib static_publish_resource_limits::tests:: -- --nocapture",
];
requireValue(
  contract.execution?.workflow === files.workflow &&
    contract.execution?.recorder === files.recorder &&
    contract.execution?.test_list_command ===
      "cargo test --locked -p rustok-page-builder --lib -- --list" &&
    JSON.stringify(contract.execution?.test_commands) === JSON.stringify(expectedCommands) &&
    contract.execution?.artifact_retention_days === 90 &&
    contract.execution?.network_runtime_under_test_required === false &&
    contract.execution?.database_required === false &&
    contract.execution?.browser_required === false,
  `${files.contract}: execution definition drifted`,
);
requireValue(
  contract.output?.format === "page_builder_static_sanitization_execution_v1" &&
    contract.output?.success_status ===
      "static_sanitization_execution_passed_registry_update_pending" &&
    contract.output?.default_path === "evidence/page-builder-static-sanitization/receipt.json",
  `${files.contract}: output definition drifted`,
);
, marker, files.actualization);
}

for (const required of [files.contract, files.recorder, files.workflow, files.actualization]) {
  requireValue(
    Array.isArray(contract.required_source_files) && contract.required_source_files.includes(required),
    `${files.contract}: required_source_files is missing ${required}`,
  );
}
requireValue(
  contract.next_cursor?.static_sanitization_execution ===
      "source_ready_maintainer_execution_pending" &&
    contract.next_cursor?.static_sanitization_registry_update ===
      "blocked_on_successful_exact_source_execution_packet",
  `${files.contract}: next cursor drifted`,
);

if (failures.length > 0) {
  console.error("[verify-page-builder-static-sanitization-execution] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-page-builder-static-sanitization-execution] PASS");
console.log(
  "target=/provider/static_sanitization_contract/executed_evidence; current=pending; execution_source=ready; registry_update=pending",
);
