#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-rollout-execution-contract.json",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-rollout-evidence-harness-source.json",
  browserContract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json",
  assembler: "scripts/evidence/assemble-pages-inline-edit-rollout-evidence.mjs",
  packet:
    "docs/modules/pages-page-builder-inline-edit-rollout-evidence-harness-packet-2026-08-06.md",
  executionPlan: "docs/modules/pages-page-builder-inline-edit-execution-plan.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const exact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) failures.push(`${label} drifted`);
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
  console.error("[verify-pages-inline-edit-rollout-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const browserContract = JSON.parse(read(files.browserContract));
const assembler = read(files.assembler);
const packet = read(files.packet);
const plan = read(files.executionPlan);

if (contract.schema_version !== 1) failures.push("contract schema_version must be 1");
if (contract.module !== "pages") failures.push("contract module must be pages");
if (contract.packet !== "pages-inline-edit-rollout-execution") {
  failures.push("contract packet identity drifted");
}
if (contract.status !== "source_ready_maintainer_execution_pending") {
  failures.push("contract source status drifted");
}
if (
  contract.runtime_control_boundary !==
  "observes_external_rollout_without_mutating_configuration_or_deployment"
) {
  failures.push("runtime control boundary drifted");
}
exact(
  contract.browser_input,
  {
    format: "pages_inline_edit_browser_execution_v1",
    status: "browser_execution_passed_rollout_pending",
    same_source_commit_required: true,
    deployment_digest_required: true,
    rollout_boundary_must_be_open: true,
  },
  "browser input contract",
);
if (
  contract.browser_input.format !== browserContract.output?.format ||
  contract.browser_input.status !== browserContract.output?.status
) {
  failures.push("rollout browser input is not tied to the browser output contract");
}
if (contract.observation_input?.format !== "pages_inline_edit_rollout_observation_v1") {
  failures.push("observation input format drifted");
}
if (contract.observation_input?.flag_key !== "pages.builder.inline_edit.enabled") {
  failures.push("rollout flag key drifted");
}
exact(
  contract.observation_input?.phases,
  ["ffa", "fba"],
  "rollout observation phases",
);
exact(
  contract.observation_input?.required_admission_facts,
  [
    "pages_module_enabled_required",
    "direct_user_required",
    "authenticated_session_required",
    "pages_update_required",
  ],
  "rollout admission facts",
);
exact(
  contract.observation_input?.required_monitoring_series,
  [
    "save_conflicts",
    "authorization_denials",
    "grant_verification_failures",
    "client_load_failures",
  ],
  "rollout monitoring series",
);
if (contract.observation_input?.raw_monitoring_logs_allowed !== false) {
  failures.push("raw monitoring logs must remain forbidden");
}
if (contract.phases?.ffa?.output_status !== "ffa_observation_passed_fba_pending") {
  failures.push("FFA output status drifted");
}
if (contract.phases?.ffa?.previous_ffa_packet_required !== false) {
  failures.push("FFA must not require a previous FFA packet");
}
if (contract.phases?.ffa?.rollback_rehearsal_required !== false) {
  failures.push("FFA must not claim the later rollback rehearsal");
}
if (contract.phases?.fba?.output_status !== "fba_rollout_evidence_complete") {
  failures.push("FBA output status drifted");
}
for (const key of [
  "previous_ffa_packet_required",
  "positive_observation_window_required",
  "non_empty_enabled_cohort_required",
  "non_empty_disabled_control_cohort_required",
  "rollback_owner_and_image_required",
  "rollback_rehearsal_required",
  "ffa_window_must_precede_fba_window",
]) {
  if (contract.phases?.fba?.[key] !== true) failures.push(`FBA ${key} must be true`);
}
exact(
  contract.output,
  {
    default_ffa_path: "target/pages-inline-edit-rollout-ffa-evidence.json",
    default_fba_path: "target/pages-inline-edit-rollout-fba-evidence.json",
    format: "pages_inline_edit_rollout_execution_v1",
    atomic_replace: true,
    automatic_configuration_mutation: false,
    automatic_deployment_mutation: false,
    automatic_promotion: false,
    automatic_rollback: false,
  },
  "rollout output contract",
);
if (contract.cli?.script !== files.assembler) failures.push("rollout assembler path drifted");
exact(
  contract.cli?.required_arguments,
  ["--phase", "--browser", "--observation", "--output"],
  "rollout required CLI arguments",
);
if (contract.cli?.fba_only_argument !== "--ffa") failures.push("FBA CLI argument drifted");
for (const relativePath of Object.values(files)) {
  if (relativePath === files.browserContract) continue;
  if (!contract.required_source_files?.includes(relativePath)) {
    failures.push(`required_source_files is missing ${relativePath}`);
  }
}
if (!contract.required_source_files?.includes(files.browserContract)) {
  failures.push(`required_source_files is missing ${files.browserContract}`);
}
for (const forbiddenValue of [
  "raw_tenant_id",
  "tenant_name",
  "authorization_header",
  "cookie_header",
  "session_id",
  "authorization_proof",
  "grant",
  "signing_key",
  "database_url",
  "deployment_credentials",
  "configuration_secret",
  "raw_monitoring_log",
  "raw_alert_payload",
  "raw_browser_html",
  "raw_request_body",
  "raw_response_body",
]) {
  if (!contract.privacy_boundary?.forbidden_persisted_values?.includes(forbiddenValue)) {
    failures.push(`privacy boundary is missing ${forbiddenValue}`);
  }
}

if (evidence.format !== "pages_inline_edit_rollout_evidence_harness_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "pages_inline_edit_rollout_evidence_harness_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}
for (const key of [
  "machine_rollout_execution_contract_added",
  "browser_evidence_is_required_first",
  "browser_source_commit_and_deployment_digest_are_rechecked",
  "external_rollout_observation_is_required",
  "runtime_configuration_is_not_mutated",
  "deployment_is_not_mutated",
  "promotion_is_not_automated",
  "rollback_is_not_automated",
  "ffa_and_fba_are_separate_phases",
  "fba_requires_previous_ffa_packet",
  "ffa_window_must_precede_fba_window",
  "tenant_cohort_uses_sha256_identities_only",
  "enabled_and_control_cohorts_must_be_non_empty_and_disjoint",
  "pages_module_direct_user_authenticated_session_and_pages_update_are_required",
  "save_conflict_authorization_grant_and_client_load_series_are_required",
  "observed_counts_must_not_exceed_reviewed_thresholds",
  "rollback_owner_and_immutable_image_are_required",
  "fba_requires_successful_rollback_rehearsal",
  "raw_monitoring_logs_and_alert_payloads_are_not_persisted",
  "raw_tenant_environment_profile_and_owner_values_are_not_persisted",
  "output_is_atomically_replaced",
  "canonical_source_is_not_mutated_automatically",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "static_verifiers_run",
  "cargo_run",
  "npm_or_playwright_run",
  "browser_run",
  "http_requests_run",
  "deployment_or_configuration_mutation_run",
  "monitoring_queries_run",
  "rollout_run",
  "rollback_rehearsal_run",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must remain false`);
  }
}

for (const marker of [
  'const contractPath = path.join(',
  '"pages-inline-edit-rollout-execution-contract.json"',
  'execFileSync("git", ["rev-parse", "HEAD"]',
  'document.format !== contract.browser_input.format',
  'document.status !== contract.browser_input.status',
  'document.source_commit !== head',
  'target.deployment_image_digest',
  'boundaries.tenant_rollout_executed !== false',
  'cohort.flag_key !== contract.observation_input.flag_key',
  'enabled and disabled control tenant cohorts overlap',
  'for (const key of contract.observation_input.required_admission_facts)',
  'for (const series of contract.observation_input.required_monitoring_series)',
  'observed > threshold',
  'rollback image digest must differ from the active deployment image digest',
  'FBA evidence requires a successful rollback rehearsal',
  'FBA evidence requires reviewed FFA evidence',
  'FFA observation window must end before the FBA observation window starts',
  'previous FFA packet identity, status, phase, or source commit drifted',
  'rollout evidence output must remain inside repository target/',
  'writeAtomic(output, document)',
  'configuration_mutated_by_assembler: false',
  'deployment_mutated_by_assembler: false',
  'promotion_performed_by_assembler: false',
  'rollback_performed_by_assembler: false',
  'raw_tenant_ids_persisted: false',
  'raw_monitoring_logs_or_alert_payloads_persisted: false',
]) need(assembler, marker, "rollout assembler");
for (const marker of [
  "fetch(",
  "axios",
  "child_process.spawn",
  "kubectl",
  "helm ",
  "docker ",
  "terraform",
  "Authorization",
  "Cookie",
  "process.env.DATABASE_URL",
  "automatic_promotion: true",
  "configuration_mutated_by_assembler: true",
]) forbid(assembler, marker, "rollout assembler mutation/privacy boundary");

for (const marker of [
  "source-ready / maintainer-execution-pending",
  "pages_inline_edit_rollout_observation_v1",
  "ffa_observation_passed_fba_pending",
  "fba_rollout_evidence_complete",
  "does not change deployment or configuration",
  "enabled and disabled control cohorts",
  "rollback rehearsal",
  "raw monitoring logs",
  "No rollout execution is claimed",
]) need(packet, marker, "rollout evidence packet");
for (const marker of [
  "inline-edit-rollout-evidence-harness-source-ready",
  "verify-pages-inline-edit-rollout-evidence-harness.mjs",
  "assemble-pages-inline-edit-rollout-evidence.mjs",
  "ffa_observation_passed_fba_pending",
  "fba_rollout_evidence_complete",
  "rollout evidence harness: source-ready",
  "rollout execution: pending",
]) need(plan, marker, "active execution plan");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-rollout-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-inline-edit-rollout-evidence-harness] PASS rollout_harness_source_ready=true execution=pending ffa=false fba=false",
);
