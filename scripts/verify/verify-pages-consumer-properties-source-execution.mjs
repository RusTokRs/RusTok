#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json",
  recorder: "scripts/evidence/record-pages-consumer-properties-source-execution.mjs",
  workflow: ".github/workflows/pages-consumer-properties-source-evidence.yml",
  actualization:
    "docs/modules/pages-consumer-properties-source-execution-actualization-2026-08-13.md",
  consumer: "crates/rustok-page-builder/contracts/page-builder-consumer-properties.json",
  registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
  revision:
    "crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json",
  published:
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json",
  browser:
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  metadata: "crates/rustok-pages/admin/src/metadata_properties.rs",
  standalone: "crates/rustok-pages/admin/src/standalone_metadata.rs",
  panel: "crates/rustok-page-builder/admin/src/editor/consumer_properties.rs",
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

const contract = json(files.contract);
const consumer = json(files.consumer);
const registry = json(files.registry);
const revision = json(files.revision);
const published = json(files.published);
const browser = json(files.browser);
const recorder = read(files.recorder);
const workflow = read(files.workflow);
const actualization = read(files.actualization);
const metadata = read(files.metadata);
const standalone = read(files.standalone);
const panel = read(files.panel);

requireValue(
  contract.format === "pages_consumer_properties_source_execution_source_v1" &&
    contract.status === "source_ready_main_execution_pending" &&
    contract.scope === "pages_page_builder_consumer_properties_rust_source_execution",
  `${files.contract}: identity drifted`,
);
requireValue(
  contract.consumer_contract?.path === files.consumer &&
    contract.consumer_contract?.required_format === "page_builder_consumer_properties_v1" &&
    contract.consumer_contract?.required_status === "metadata_surface_cutover_complete" &&
    contract.consumer_contract?.executed_evidence_json_pointer === "/executed_evidence" &&
    contract.consumer_contract?.required_before_value === "pending" &&
    contract.consumer_contract?.mutation_by_workflow === false,
  `${files.contract}: consumer target boundary drifted`,
);
requireValue(
  consumer.format === contract.consumer_contract?.required_format &&
    consumer.status === contract.consumer_contract?.required_status &&
    pointerValue(consumer, contract.consumer_contract?.executed_evidence_json_pointer) === "pending",
  `${files.consumer}: executed_evidence must remain pending before Rust source execution`,
);
requireValue(
  contract.fba_registry?.path === files.registry &&
    contract.fba_registry?.required_status === "boundary_ready" &&
    contract.fba_registry?.executed_evidence_json_pointer ===
      "/provider/consumer_properties_contract/executed_evidence" &&
    contract.fba_registry?.required_before_value === "pending" &&
    contract.fba_registry?.mutation_by_workflow === false,
  `${files.contract}: FBA target boundary drifted`,
);
requireValue(
  registry.status === "boundary_ready" &&
    pointerValue(registry, contract.fba_registry?.executed_evidence_json_pointer) === "pending",
  `${files.registry}: consumer-properties FBA evidence must remain pending`,
);

requireValue(
  revision.status === "pages_metadata_revision_isolation_source_unvalidated" &&
    revision.source_contract?.stale_revision_rejected_before_patch_transport === true &&
    revision.source_contract?.dirty_fly_state_mutated_by_metadata_port === false &&
    revision.source_contract?.metadata_revision_advances_independently === true &&
    Array.isArray(revision.execution) &&
    revision.execution.length === 0,
  `${files.revision}: revision/isolation source boundary drifted`,
);
requireValue(
  published.status === "pages_published_metadata_surface_source_unvalidated" &&
    published.source_contract?.published_page_admits_registered_surface === true &&
    published.source_contract?.browser_execution_required_for_executed_evidence === true &&
    published.browser_execution?.state === "source_ready_maintainer_execution_pending" &&
    Array.isArray(published.execution) &&
    published.execution.length === 0,
  `${files.published}: published metadata source boundary drifted`,
);
requireValue(
  browser.status === "source_ready_maintainer_execution_pending" &&
    browser.output?.format === "pages_published_metadata_browser_execution_v1" &&
    browser.output?.status === "browser_execution_passed_consumer_properties_admission_pending" &&
    browser.deployment_identity?.browser_independent_digest_to_deployment_attestation === false,
  `${files.browser}: browser evidence must remain execution-pending`,
);

const expectedTests = [
  "cargo test --locked -p rustok-pages-admin published_page_admits_registered_metadata_surface -- --nocapture",
  "cargo test --locked -p rustok-pages-admin non_published_or_missing_page_hides_registered_metadata_surface -- --nocapture",
  "cargo test --locked -p rustok-pages-admin stale_metadata_revision_short_circuits_before_patch_transport -- --nocapture",
  "cargo test --locked -p rustok-pages-admin metadata_save_is_document_free_and_preserves_dirty_fly_state -- --nocapture",
];
const expectedVerifiers = [
  "node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs",
  "node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs",
  "node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs",
  "node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-evidence-harness.mjs",
  "node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-execution-workflow.mjs",
  "node scripts/verify/verify-pages-consumer-properties-source-execution.mjs",
];
requireValue(
  contract.execution?.workflow === files.workflow &&
    contract.execution?.recorder === files.recorder &&
    contract.execution?.source_verifier ===
      "scripts/verify/verify-pages-consumer-properties-source-execution.mjs" &&
    JSON.stringify(contract.execution?.validation_events) ===
      JSON.stringify(["pull_request", "push", "workflow_dispatch"]) &&
    JSON.stringify(contract.execution?.receipt_events) ===
      JSON.stringify(["push", "workflow_dispatch"]) &&
    contract.execution?.receipt_ref_name === "main" &&
    contract.execution?.pull_request_receipt === "skipped" &&
    contract.execution?.test_list_command ===
      "cargo test --locked -p rustok-pages-admin --lib -- --list" &&
    JSON.stringify(contract.execution?.verifier_commands) === JSON.stringify(expectedVerifiers) &&
    JSON.stringify(contract.execution?.test_commands) === JSON.stringify(expectedTests) &&
    contract.execution?.check_command ===
      "cargo check --locked -p rustok-pages-admin --all-targets" &&
    contract.execution?.artifact_retention_days === 90 &&
    contract.execution?.network_runtime_under_test_required === false &&
    contract.execution?.database_required === false &&
    contract.execution?.browser_required === false,
  `${files.contract}: execution definition drifted`,
);
requireValue(
  contract.execution?.run_index_status?.context ===
      "pages-consumer-properties-source-evidence-index" &&
    JSON.stringify(contract.execution?.run_index_status?.events) ===
      JSON.stringify(["push", "workflow_dispatch"]) &&
    contract.execution?.run_index_status?.permission === "statuses: write" &&
    contract.execution?.run_index_status?.target === "github_actions_run_url" &&
    contract.execution?.run_index_status?.repository_content_mutation === false,
  `${files.contract}: run index status boundary drifted`,
);
requireValue(
  contract.output?.format === "pages_consumer_properties_source_execution_v1" &&
    contract.output?.success_status ===
      "rust_source_execution_passed_browser_evidence_pending" &&
    contract.output?.default_path === "evidence/pages-consumer-properties/receipt.json",
  `${files.contract}: output definition drifted`,
);

for (const key of [
  "execution_packet_is_not_consumer_contract_update",
  "execution_packet_is_not_fba_registry_update",
  "execution_packet_is_not_browser_evidence",
  "execution_packet_is_not_terminal_inventory_completion",
  "execution_packet_is_not_owner_approval",
  "execution_packet_is_not_platform_approval",
  "execution_packet_does_not_promote_ffa_or_fba",
  "consumer_and_registry_pending_values_require_later_evidence_containing_pr",
  "later_admission_must_bind_exact_rust_receipt_browser_packet_and_source_lineage",
  "run_index_mutates_commit_status_only",
]) {
  requireValue(contract.governance_boundary?.[key] === true, `${files.contract}: ${key} must be true`);
}
requireValue(
  contract.governance_boundary?.cryptographic_ci_attestation_claimed === false,
  `${files.contract}: cryptographic CI attestation must remain unclaimed`,
);
for (const [key, value] of Object.entries(contract.non_claims ?? {})) {
  requireValue(value === false, `${files.contract}: non_claims.${key} must remain false`);
}

for (const marker of [
  'process.env.GITHUB_ACTIONS !== "true"',
  "GITHUB_SHA does not equal checkout HEAD",
  'workflow !== "Pages Consumer Properties Source Evidence"',
  'const RECEIPT_EVENTS = new Set(["push", "workflow_dispatch"]);',
  '!RECEIPT_EVENTS.has(eventName) || refName !== "main"',
  "only an exact main push or main workflow dispatch may mint a source execution receipt",
  "consumer properties executed_evidence is no longer pending",
  "FBA consumer-properties executed_evidence is no longer pending",
  "all_commands_passed: true",
  "packet_generated_only_after_test_and_check_steps: true",
  "browser_used: false",
  "browser_evidence_pending: true",
  "consumer_contract_mutated: false",
  "fba_registry_mutated: false",
  "browser_execution_claimed: false",
  "deployment_provenance_verified: false",
  "later_admission_must_bind_rust_browser_and_source_lineage: true",
  "source_sha256: sourceSha256",
]) {
  requireText(recorder, marker, files.recorder);
}
for (const forbidden of [
  "fetch(",
  "http://",
  "https://",
  "git push",
  "git commit",
  "updateModuleSettings",
  "compareAndSwapModuleSettings",
]) {
  forbidText(recorder, forbidden, files.recorder);
}

for (const marker of [
  "name: Pages Consumer Properties Source Evidence",
  "workflow_dispatch:",
  "pull_request:",
  "push:",
  "branches:",
  "- main",
  "Require canonical main manual dispatch",
  'test "$GITHUB_REF" = "refs/heads/main"',
  "permissions:",
  "contents: read",
  "statuses: write",
  "persist-credentials: false",
  ...expectedVerifiers,
  "cargo test --locked -p rustok-pages-admin --lib -- --list",
  "standalone_metadata::tests::published_page_admits_registered_metadata_surface",
  "standalone_metadata::tests::non_published_or_missing_page_hides_registered_metadata_surface",
  "metadata_properties::tests::stale_metadata_revision_short_circuits_before_patch_transport",
  "metadata_properties::tests::metadata_save_is_document_free_and_preserves_dirty_fly_state",
  ...expectedTests,
  "cargo check --locked -p rustok-pages-admin --all-targets",
  "node scripts/evidence/record-pages-consumer-properties-source-execution.mjs",
  "if: github.event_name != 'pull_request'",
  "actions/upload-artifact@v7",
  "retention-days: 90",
  "name: Consumer Properties Source Evidence Gate",
  "Publish exact-main evidence run index",
  "pages-consumer-properties-source-evidence-index",
  'target_url="https://github.com/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"',
  '"https://api.github.com/repos/${GITHUB_REPOSITORY}/statuses/${GITHUB_SHA}"',
]) {
  requireText(workflow, marker, files.workflow);
}
for (const forbidden of [
  "contents: write",
  "pull-requests: write",
  "persist-credentials: true",
  "git push",
  "git commit",
  "gh pr",
]) {
  forbidText(workflow, forbidden, files.workflow);
}

for (const marker of [
  "published_page_admits_registered_metadata_surface",
  "non_published_or_missing_page_hides_registered_metadata_surface",
]) {
  requireText(standalone, marker, files.standalone);
}
for (const marker of [
  "stale_metadata_revision_short_circuits_before_patch_transport",
  "metadata_save_is_document_free_and_preserves_dirty_fly_state",
  "require_current_metadata_version",
  "PageMetadataPatch",
  "transport.patch_metadata(request).await?",
]) {
  requireText(metadata, marker, files.metadata);
}
for (const marker of [
  'data-fly-consumer-properties="ready"',
  '"Save properties"',
]) {
  requireText(panel, marker, files.panel);
}

for (const required of [files.contract, files.recorder, files.workflow, files.actualization]) {
  requireValue(
    Array.isArray(contract.required_source_files) && contract.required_source_files.includes(required),
    `${files.contract}: required_source_files missing ${required}`,
  );
}
for (const marker of [
  "source-ready / exact-main-rust-execution-pending / browser-evidence-pending",
  "rust_source_execution_passed_browser_evidence_pending",
  "four focused Rust regressions",
  "does not execute Chromium",
  "consumer properties executed evidence remains `pending`",
  "manual exact-main revalidation",
  "pages-consumer-properties-source-evidence-index",
]) {
  requireText(actualization, marker, files.actualization);
}
requireValue(
  contract.next_cursor?.rust_source_execution ===
      "await_completed_successful_exact_main_push_or_dispatch_run_and_retained_artifact" &&
    contract.next_cursor?.browser_execution ===
      "maintainer_external_fixture_execution_pending" &&
    contract.next_cursor?.consumer_properties_admission ===
      "blocked_until_rust_and_browser_packets_are_both_admissible",
  `${files.contract}: next cursor drifted`,
);

if (failures.length > 0) {
  console.error("[verify-pages-consumer-properties-source-execution] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-consumer-properties-source-execution] PASS rust_source=ready exact_main_execution=pending browser_evidence=pending consumer_admission=blocked lifecycle=observable",
);
