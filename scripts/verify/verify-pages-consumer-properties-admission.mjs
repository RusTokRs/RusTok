#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];
const files = {
  admission: "crates/rustok-pages/contracts/evidence/pages-consumer-properties-admission-source.json",
  source: "crates/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json",
  browser: "crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  runner: "scripts/evidence/admit-pages-consumer-properties.mjs",
  test: "scripts/evidence/admit-pages-consumer-properties.test.mjs",
  sourceWorkflow: ".github/workflows/pages-consumer-properties-source-evidence.yml",
  browserWorkflow: ".github/workflows/pages-published-metadata-browser-evidence.yml",
  actualization: "docs/modules/pages-consumer-properties-admission-actualization-2026-08-18.md",
  consumer: "crates/rustok-page-builder/contracts/page-builder-consumer-properties.json",
  registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

function read(relative) {
  try {
    const location = path.join(root, relative);
    const stat = fs.lstatSync(location);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error("must be a regular non-symlink file");
    return fs.readFileSync(location, "utf8");
  } catch (error) {
    failures.push(`${relative}: ${error.message}`);
    return "";
  }
}

function json(relative) {
  try {
    const value = JSON.parse(read(relative));
    if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error("root must be an object");
    return value;
  } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
    return {};
  }
}

function ok(condition, message) {
  if (!condition) failures.push(message);
}
function has(text, marker, label) {
  ok(text.includes(marker), `${label}: missing '${marker}'`);
}
function lacks(text, marker, label) {
  ok(!text.includes(marker), `${label}: forbidden '${marker}'`);
}
function pointer(document, value) {
  let current = document;
  for (const raw of String(value ?? "").replace(/^\//u, "").split("/")) {
    if (!raw) continue;
    const key = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object" || !Object.hasOwn(current, key)) return undefined;
    current = current[key];
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

ok(
  admission.format === "pages_consumer_properties_admission_source_v1" &&
    admission.status === "source_ready_maintainer_evidence_pending" &&
    admission.scope === "pages_page_builder_consumer_properties_executed_evidence_admission",
  `${files.admission}: identity drifted`,
);

const s = admission.source_execution_input ?? {};
ok(
  s.format === "pages_consumer_properties_source_execution_v1" &&
    s.source_contract === files.source &&
    s.source_commit_must_be_checkout_head_or_ancestor === true &&
    s.ancestry_must_be_verified_offline_by_git === true &&
    s.retained_source_hashes_must_match_contract_and_checkout === true &&
    s.retained_target_hashes_must_match_current_pending_checkout === true &&
    s.run_index_must_be_reviewed_on_source_receipt_commit === true &&
    s.repository_must_equal === "RusTokRs/RusTok" &&
    s.workflow_must_equal === "Pages Consumer Properties Source Evidence" &&
    s.run_index_context === "pages-consumer-properties-source-evidence-index",
  `${files.admission}: source lineage boundary drifted`,
);

const b = admission.browser_input ?? {};
ok(
  b.format === "pages_published_metadata_browser_execution_v1" &&
    b.source_contract === files.browser &&
    b.source_commit_must_equal_checkout_head === true &&
    b.run_index_must_be_reviewed_on_checkout_head === true &&
    b.deployment_digest_must_be_immutable_repo_digest === true &&
    b.retained_source_hashes_must_match_contract_and_checkout === true &&
    JSON.stringify(b.required_profiles) === JSON.stringify(["published", "draft", "archived", "missing"]) &&
    b.run_index_context === "pages-published-metadata-browser-evidence-index",
  `${files.admission}: browser exact-checkout boundary drifted`,
);

const d = admission.deployment_provenance_input ?? {};
ok(
  d.format === "pages_consumer_properties_deployment_provenance_v1" &&
    d.source_commit_must_equal_checkout_head_and_browser_packet === true &&
    d.source_workflow_commit_must_equal_source_receipt_commit === true &&
    d.browser_workflow_commit_must_equal_checkout_head === true &&
    d.both_workflow_indexes_must_be_reviewed_success_on_their_bound_commits === true &&
    d.source_workflow_index_context === "pages-consumer-properties-source-evidence-index" &&
    d.browser_workflow_index_context === "pages-published-metadata-browser-evidence-index" &&
    d.source_workflow_run_id_must_equal_source_receipt === true &&
    d.input_packet_sha256_must_equal_supplied_packets === true &&
    d.origin_to_repo_digest_binding_classification === "maintainer_reviewed_external_fact" &&
    d.cryptographic_origin_to_repo_digest_binding_must_be_false === true,
  `${files.admission}: deployment provenance boundary drifted`,
);

const consumerSpec = admission.target_preconditions?.consumer_contract ?? {};
const registrySpec = admission.target_preconditions?.fba_registry ?? {};
ok(
  consumerSpec.path === files.consumer && consumerSpec.required_before_value === "pending" &&
    consumer.status === consumerSpec.required_status &&
    pointer(consumer, consumerSpec.executed_evidence_json_pointer) === "pending",
  `${files.consumer}: executed_evidence must remain pending`,
);
ok(
  registrySpec.path === files.registry && registrySpec.required_before_value === "pending" &&
    registry.status === registrySpec.required_status &&
    pointer(registry, registrySpec.executed_evidence_json_pointer) === "pending",
  `${files.registry}: executed_evidence must remain pending`,
);

ok(
  source.output?.format === s.format && source.output?.success_status === s.required_status &&
    source.execution?.run_index_status?.context === s.run_index_context,
  `${files.source}: source receipt/index identity drifted`,
);
ok(
  browser.output?.format === b.format && browser.output?.status === b.required_status &&
    browser.workflow_execution?.run_index_status?.context === b.run_index_context &&
    browser.workflow_execution?.source_commit_must_equal_dispatch_sha === true &&
    browser.deployment_identity?.browser_independent_digest_to_deployment_attestation === false,
  `${files.browser}: browser/deployment boundary drifted`,
);

ok(
  admission.output?.source_receipt_commit_retained === true &&
    admission.output?.checkout_source_commit_retained === true &&
    admission.output?.cryptographic_deployment_binding_claimed === false,
  `${files.admission}: output lineage boundary drifted`,
);
for (const key of [
  "source_receipt_may_precede_checkout_only_if_ancestor_and_current_hash_equivalent",
  "browser_and_deployment_exact_checkout_source_commit_required",
  "source_index_is_reviewed_on_receipt_commit_and_browser_index_on_checkout_commit",
  "workflow_run_review_does_not_become_cryptographic_ci_attestation",
  "admission_output_is_not_consumer_contract_update",
  "admission_output_is_not_fba_registry_update",
]) ok(admission.admission_boundary?.[key] === true, `${files.admission}: ${key} must be true`);

for (const required of [files.admission, files.runner, files.test, files.actualization, files.source, files.sourceWorkflow, files.browser, files.browserWorkflow, files.consumer, files.registry]) {
  ok(admission.required_source_files?.includes(required), `${files.admission}: missing ${required}`);
  ok(source.required_source_files?.includes(required), `${files.source}: successor receipt source set missing ${required}`);
}

for (const marker of [
  '"merge-base", "--is-ancestor"',
  "source execution receipt source_commit is not a locally verifiable ancestor of checkout HEAD",
  "source receipt consumer target does not match current pending checkout",
  "source receipt FBA target does not match current pending checkout",
  "browser packet source_commit does not equal checkout HEAD",
  "deployment provenance source_commit does not equal checkout HEAD",
  "source_receipt_commit: sourceReceipt.sourceCommit",
  "browser_deployment_source_commit: head",
  "source_receipt_ancestor_lineage_bound: true",
  "source_receipt_required_sources_equal_current_checkout: true",
  "browser_and_deployment_exact_source_commit_bound: true",
  "consumer_contract_mutated: false",
  "fba_registry_mutated: false",
  "executed_evidence_verified: false",
]) has(runner, marker, files.runner);
for (const forbidden of ["fetch(", "http://", "https://", "git push", "git commit", "gh ", "curl ", "updateModuleSettings", "compareAndSwapModuleSettings"]) lacks(runner, forbidden, files.runner);

for (const marker of [
  "accepts exact head as valid ancestor lineage",
  "rejects non ancestor source receipt",
  "rejects stale source receipt hash",
  "rejects browser checkout commit drift",
  "rejects deployment digest drift",
  "rejects failed browser observation",
  "rejects route provenance drift",
  "rejects source workflow run drift",
  "rejects source workflow commit review drift",
  "rejects browser packet hash drift",
  "rejects cryptographic deployment overclaim",
  "fail_closed_mutations=10",
]) has(test, marker, files.test);

for (const marker of ["pages-consumer-properties-source-evidence-index", 'state="failure"', "statuses: write"]) has(sourceWorkflow, marker, files.sourceWorkflow);
for (const marker of ["pages-published-metadata-browser-evidence-index", "deployment provenance and admission pending"]) has(browserWorkflow, marker, files.browserWorkflow);
for (const marker of [
  "32177516104",
  "c0a7bd91fc68b5462996a6d4e929bad6e7d6a208",
  "de5eec28762b29ead4389d740e3b3aa3e9743de9",
  "ancestor",
  "byte-identical",
  "maintainer_reviewed_external_fact",
  "does not query GitHub",
  "does not change `executed_evidence`",
  "successor exact-main source receipt",
]) has(actualization, marker, files.actualization);

if (failures.length) {
  console.error("[verify-pages-consumer-properties-admission] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-consumer-properties-admission] PASS source=lineage-ready browser=pending deployment_provenance=pending registry_mutation=pending");
