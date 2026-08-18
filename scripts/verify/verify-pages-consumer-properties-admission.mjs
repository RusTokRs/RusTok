#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];
const files = {
  admission:
    "crates/rustok-pages/contracts/evidence/pages-consumer-properties-admission-source.json",
  runner: "scripts/evidence/admit-pages-consumer-properties.mjs",
  test: "scripts/evidence/admit-pages-consumer-properties.test.mjs",
  actualization:
    "docs/modules/pages-consumer-properties-admission-actualization-2026-08-18.md",
  source:
    "crates/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json",
  sourceWorkflow: ".github/workflows/pages-consumer-properties-source-evidence.yml",
  browser:
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  browserWorkflow: ".github/workflows/pages-published-metadata-browser-evidence.yml",
  consumer: "crates/rustok-page-builder/contracts/page-builder-consumer-properties.json",
  registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);

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

const admission = json(files.admission);
const source = json(files.source);
const browser = json(files.browser);
const consumer = json(files.consumer);
const registry = json(files.registry);
const runner = read(files.runner);
const test = read(files.test);
const sourceWorkflow = read(files.sourceWorkflow);
const browserWorkflow = read(files.browserWorkflow);
const actualization = read(files.actualization);

requireValue(
  admission.format === "pages_consumer_properties_admission_source_v1" &&
    admission.status === "source_ready_maintainer_evidence_pending" &&
    admission.scope === "pages_page_builder_consumer_properties_executed_evidence_admission",
  `${files.admission}: identity drifted`,
);

requireValue(
  admission.source_execution_input?.format === "pages_consumer_properties_source_execution_v1" &&
    admission.source_execution_input?.required_status ===
      "rust_source_execution_passed_browser_evidence_pending" &&
    admission.source_execution_input?.source_contract === files.source &&
    admission.source_execution_input?.source_commit_must_equal_checkout_head === true &&
    admission.source_execution_input?.retained_source_hashes_must_match_contract_and_checkout ===
      true &&
    admission.source_execution_input?.repository_must_equal === "RusTokRs/RusTok" &&
    admission.source_execution_input?.workflow_must_equal ===
      "Pages Consumer Properties Source Evidence" &&
    admission.source_execution_input?.run_index_context ===
      "pages-consumer-properties-source-evidence-index",
  `${files.admission}: source receipt boundary drifted`,
);

requireValue(
  admission.browser_input?.format === "pages_published_metadata_browser_execution_v1" &&
    admission.browser_input?.required_status ===
      "browser_execution_passed_consumer_properties_admission_pending" &&
    admission.browser_input?.source_contract === files.browser &&
    admission.browser_input?.source_commit_must_equal_checkout_head_and_source_receipt === true &&
    admission.browser_input?.deployment_digest_must_be_immutable_repo_digest === true &&
    admission.browser_input?.retained_source_hashes_must_match_contract_and_checkout === true &&
    JSON.stringify(admission.browser_input?.required_profiles) ===
      JSON.stringify(["published", "draft", "archived", "missing"]) &&
    admission.browser_input?.run_index_context ===
      "pages-published-metadata-browser-evidence-index",
  `${files.admission}: browser packet boundary drifted`,
);

requireValue(
  admission.deployment_provenance_input?.format ===
      "pages_consumer_properties_deployment_provenance_v1" &&
    admission.deployment_provenance_input?.required_status ===
      "maintainer_reviewed_deployment_identity" &&
    admission.deployment_provenance_input?.origin_to_repo_digest_binding_classification ===
      "maintainer_reviewed_external_fact" &&
    admission.deployment_provenance_input
      ?.cryptographic_origin_to_repo_digest_binding_must_be_false === true &&
    admission.deployment_provenance_input?.profile_url_sha256_must_equal_browser_packet === true &&
    admission.deployment_provenance_input?.source_workflow_run_id_must_equal_source_receipt ===
      true &&
    admission.deployment_provenance_input?.source_workflow_index_context ===
      "pages-consumer-properties-source-evidence-index" &&
    admission.deployment_provenance_input?.browser_workflow_index_context ===
      "pages-published-metadata-browser-evidence-index" &&
    admission.deployment_provenance_input
      ?.both_workflow_indexes_must_be_reviewed_success_on_exact_commit === true,
  `${files.admission}: reviewed deployment provenance boundary drifted`,
);

const consumerSpec = admission.target_preconditions?.consumer_contract;
const registrySpec = admission.target_preconditions?.fba_registry;
requireValue(
  consumerSpec?.path === files.consumer &&
    consumerSpec?.required_format === "page_builder_consumer_properties_v1" &&
    consumerSpec?.required_status === "metadata_surface_cutover_complete" &&
    consumerSpec?.executed_evidence_json_pointer === "/executed_evidence" &&
    consumerSpec?.required_before_value === "pending" &&
    consumer.format === consumerSpec.required_format &&
    consumer.status === consumerSpec.required_status &&
    pointerValue(consumer, consumerSpec.executed_evidence_json_pointer) === "pending",
  `${files.consumer}: consumer properties evidence must remain pending`,
);
requireValue(
  registrySpec?.path === files.registry &&
    registrySpec?.required_status === "boundary_ready" &&
    registrySpec?.executed_evidence_json_pointer ===
      "/provider/consumer_properties_contract/executed_evidence" &&
    registrySpec?.required_before_value === "pending" &&
    registry.status === "boundary_ready" &&
    pointerValue(registry, registrySpec.executed_evidence_json_pointer) === "pending",
  `${files.registry}: FBA consumer properties evidence must remain pending`,
);

requireValue(
  source.output?.format === admission.source_execution_input?.format &&
    source.output?.success_status === admission.source_execution_input?.required_status &&
    source.execution?.run_index_status?.context ===
      admission.source_execution_input?.run_index_context,
  `${files.source}: source receipt/index identity drifted`,
);
requireValue(
  browser.output?.format === admission.browser_input?.format &&
    browser.output?.status === admission.browser_input?.required_status &&
    browser.workflow_execution?.run_index_status?.context ===
      admission.browser_input?.run_index_context &&
    browser.deployment_identity?.browser_independent_digest_to_deployment_attestation === false &&
    browser.deployment_identity?.deployment_provenance_must_be_verified_outside_this_browser_packet ===
      true,
  `${files.browser}: browser/deployment boundary drifted`,
);

requireValue(
  source.admission?.source_contract === files.admission &&
    source.admission?.runner === files.runner &&
    source.admission?.source_verifier ===
      "scripts/verify/verify-pages-consumer-properties-admission.mjs" &&
    source.admission?.synthetic_test === files.test &&
    source.admission?.actualization === files.actualization &&
    source.admission?.status === "source_ready_maintainer_evidence_pending" &&
    source.admission?.browser_and_deployment_required === true &&
    source.admission?.repository_mutation === false,
  `${files.source}: admission source binding drifted`,
);

requireValue(
  admission.runner?.path === files.runner &&
    admission.runner?.network_requests === false &&
    admission.runner?.browser_execution === false &&
    admission.runner?.cargo_execution === false &&
    admission.runner?.workflow_dispatch === false &&
    admission.runner?.repository_mutation === false &&
    admission.runner?.registry_mutation === false,
  `${files.admission}: runner boundary drifted`,
);
requireValue(
  admission.verification?.source_verifier ===
      "scripts/verify/verify-pages-consumer-properties-admission.mjs" &&
    admission.verification?.synthetic_test === files.test &&
    JSON.stringify(admission.verification?.validation_commands) ===
      JSON.stringify([
        "node scripts/verify/verify-pages-consumer-properties-admission.mjs",
        "node scripts/evidence/admit-pages-consumer-properties.test.mjs",
      ]),
  `${files.admission}: validation commands drifted`,
);

requireValue(
  admission.output?.format === "pages_consumer_properties_admission_v1" &&
    admission.output?.status ===
      "consumer_properties_execution_evidence_admitted_registry_update_pending" &&
    admission.output?.default_path === "target/pages-consumer-properties-admission.json" &&
    admission.output?.raw_input_paths_retained === false &&
    admission.output?.raw_profile_urls_retained === false &&
    admission.output?.cryptographic_deployment_binding_claimed === false,
  `${files.admission}: output boundary drifted`,
);
for (const key of [
  "exact_source_commit_required_across_all_inputs",
  "same_immutable_repo_digest_required_across_browser_and_deployment_provenance",
  "source_and_browser_workflow_indexes_are_maintainer_reviewed_external_facts",
  "workflow_run_review_does_not_become_cryptographic_ci_attestation",
  "deployment_digest_equality_does_not_upgrade_origin_binding_to_cryptographic_proof",
  "admission_output_is_not_consumer_contract_update",
  "admission_output_is_not_fba_registry_update",
  "later_evidence_containing_pr_must_review_and_apply_registry_mutation",
]) {
  requireValue(admission.admission_boundary?.[key] === true, `${files.admission}: ${key} must be true`);
}
for (const [key, value] of Object.entries(admission.non_claims ?? {})) {
  requireValue(value === false, `${files.admission}: non_claims.${key} must remain false`);
}

for (const required of [
  files.admission,
  files.runner,
  "scripts/verify/verify-pages-consumer-properties-admission.mjs",
  files.test,
  files.actualization,
  files.source,
  files.sourceWorkflow,
  files.browser,
  files.browserWorkflow,
  files.consumer,
  files.registry,
]) {
  requireValue(
    Array.isArray(admission.required_source_files) &&
      admission.required_source_files.includes(required),
    `${files.admission}: required_source_files missing ${required}`,
  );
  requireValue(
    Array.isArray(source.required_source_files) && source.required_source_files.includes(required),
    `${files.source}: successor receipt source set missing ${required}`,
  );
}

for (const marker of [
  "--source-receipt",
  "--browser-evidence",
  "--deployment-provenance",
  "source execution receipt source_commit does not equal checkout HEAD",
  "source hash set differs from its source contract",
  "browser packet source_commit does not equal checkout HEAD",
  "browser profile set drifted",
  "deployment provenance RepoDigest differs from browser packet",
  "deployment provenance route hashes differ from browser packet",
  "deployment provenance workflow index review drifted",
  "cryptographic_origin_to_repo_digest_binding !== false",
  "consumer properties contract is no longer in the pending admission state",
  "Page Builder FBA consumer-properties evidence is no longer pending",
  "registry_update_ready_for_later_evidence_containing_pr: true",
  "consumer_contract_mutated: false",
  "fba_registry_mutated: false",
  "executed_evidence_verified: false",
]) {
  requireText(runner, marker, files.runner);
}
for (const forbidden of [
  "fetch(",
  "http://",
  "https://",
  "git push",
  "git commit",
  "gh ",
  "curl ",
  "updateModuleSettings",
  "compareAndSwapModuleSettings",
]) {
  forbidText(runner, forbidden, files.runner);
}

for (const marker of [
  "rejects source commit drift",
  "rejects deployment digest drift",
  "rejects failed browser observation",
  "rejects route provenance drift",
  "rejects source workflow run drift",
  "rejects cryptographic deployment overclaim",
]) {
  requireText(test, marker, files.test);
}

for (const marker of [
  `"${files.admission}"`,
  `"${files.runner}"`,
  `"scripts/verify/verify-pages-consumer-properties-admission.mjs"`,
  `"${files.test}"`,
  `"${files.actualization}"`,
  "node scripts/verify/verify-pages-consumer-properties-admission.mjs",
  "node scripts/evidence/admit-pages-consumer-properties.test.mjs",
]) {
  requireText(sourceWorkflow, marker, files.sourceWorkflow);
}
for (const marker of [
  "pages-consumer-properties-source-evidence-index",
  "state=\"failure\"",
  "statuses: write",
]) {
  requireText(sourceWorkflow, marker, files.sourceWorkflow);
}
for (const marker of [
  "pages-published-metadata-browser-evidence-index",
  "deployment provenance and admission pending",
]) {
  requireText(browserWorkflow, marker, files.browserWorkflow);
}

for (const marker of [
  "source-ready / maintainer-browser-and-deployment-evidence-pending / admission-runner-ready",
  "32170986733",
  "2a3f717dc3cac8b0c99c5b1cbe4bee7c8c5492bd",
  "maintainer_reviewed_external_fact",
  "pages-consumer-properties-source-evidence-index",
  "pages-published-metadata-browser-evidence-index",
  "does not query GitHub",
  "does not change `executed_evidence`",
  "successor exact-main source receipt",
]) {
  requireText(actualization, marker, files.actualization);
}

if (failures.length > 0) {
  console.error("[verify-pages-consumer-properties-admission] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-consumer-properties-admission] PASS source=ready browser=pending deployment_provenance=pending registry_mutation=pending",
);
