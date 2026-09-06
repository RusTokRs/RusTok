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
const parityActualizationPath = "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md";
const basePlanReconciliationPath = "docs/modules/pages-page-builder-base-plan-reconciliation-actualization-2026-08-10.md";
const rolloutActualizationPath = "docs/modules/pages-page-builder-rollout-plan-actualization-2026-08-08.md";
const gatePath = "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json";
const gateAcceptancePath = "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json";
const forumManifestPath = "crates/rustok-forum/rustok-module.toml";
const forumWavePath = "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json";
const forumWaveAdmissionPath = "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json";
const forumWaveObservedAcceptancePath = "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-observed-acceptance-source.json";
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stats = fs.lstatSync(absolutePath);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: required source must be a regular non-symlink file`);
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

function normalizeText(source) {
  return source.replace(/\s+/g, " ").trim();
}

function requireText(source, marker, label) {
  if (!source.includes(marker) && !normalizeText(source).includes(normalizeText(marker))) {
    failures.push(`${label}: missing marker '${marker}'`);
  }
}

function forbidText(source, marker, label) {
  if (source.includes(marker) || normalizeText(source).includes(normalizeText(marker))) {
    failures.push(`${label}: stale marker remains '${marker}'`);
  }
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
const parityActualization = read(parityActualizationPath);
const basePlanReconciliation = read(basePlanReconciliationPath);
const rolloutActualization = read(rolloutActualizationPath);
const gate = parseJson(read(gatePath), gatePath);
const gateAcceptance = parseJson(read(gateAcceptancePath), gateAcceptancePath);
const forumManifest = read(forumManifestPath);
const forumWave = parseJson(read(forumWavePath), forumWavePath);
const forumWaveAdmission = parseJson(read(forumWaveAdmissionPath), forumWaveAdmissionPath);
const forumWaveObservedAcceptance = parseJson(
  read(forumWaveObservedAcceptancePath),
  forumWaveObservedAcceptancePath,
);

const sharedCurrent = section(
  sharedPlan,
  "## 2026-08-10 current source reconciliation",
  "## Rechecked merged cursor",
);
const sharedNext = section(sharedPlan, "## Next cursor", "## Maintainer validation");
const localOpen = section(localPlan, "## Open results", "## Verification");

for (const marker of [
  "provider-health-runtime-source-ready",
  "provider-health-observed-acceptance-source-ready",
  "forum-runtime-composition-source-ready",
  "forum-evidence-harness-source-ready",
  "pages-reference-consumer-gate-source-ready",
  "pages-reference-consumer-gate-acceptance-source-ready",
  "pages-gate-owner-runner-synthetic-ready",
  "forum-wave-admission-source-ready",
  "forum-wave-admission-runner-synthetic-ready",
  "forum-wave-live-lineage-source-ready",
  "forum-wave-observed-owner-acceptance-source-ready",
  "generic-editor-accessibility-source-ready",
  "generic-accessibility-browser-packet-verifier-source-ready",
  "execution-acceptance-pending",
]) requireText(sharedPlan, marker, `${sharedPlanPath}: status`);
for (const stale of [
  "observed-health-open",
  "repository still has no authoritative live Page Builder SLO observation source",
  "No live SLO source exists yet",
  "Connect real provider-health observation only after an authoritative Page Builder SLO source exists",
  "forum-fly-adapter-open",
]) forbidText(sharedPlan, stale, `${sharedPlanPath}: stale current state`);

for (const marker of [
  "PR #3239",
  "PR #3247",
  "PR #3254",
  "PR #3264",
  "PR #3266",
  "PR #3274",
  "PR #3320",
  "PRs #3389, #3391, #3395 and #3399",
  "PRs #3424 and #3426",
  "PR #3429",
  "PR #3435",
  "PR #3453",
  "PR #3456",
  "PR #3458",
  "PR #3459",
  "PR #3460",
  "PR #3461",
  "PR #3464",
  "PR #3465",
  "adapter_state = \"fly_contract_ready\"",
  "preview_data_state = \"owner_preview_transport_ready\"",
  "property_data_state = \"owner_property_editor_ready\"",
  "Current deployment health is still not asserted by source inspection",
  "accepted owner evidence is only eligible input for a separate FFA/FBA promotion review",
]) requireText(sharedCurrent, marker, `${sharedPlanPath}: current reconciliation`);

for (const marker of [
  "Execute exact provider-health",
  "rollout-only reference candidate",
  "run Forum Wave admission",
  "observed control-plane Wave",
  "verify freshness and exact retained-admission lineage",
  "retrospective observed-Wave owner decision",
  "Current provider health is not inferred by this plan",
  "Only after accepted observed-Wave owner evidence",
]) requireText(sharedNext, marker, `${sharedPlanPath}: next cursor`);

for (const marker of [
  "provider-health observation/evaluation/binding",
  "missing, invalid, expired or uninstalled accepted packet remains",
  "pages_reference_consumer_gate_acceptance_v1",
  "pages_builder_provider_health_observed_acceptance_v1",
  "forum_page_builder_wave_admission_v1",
  "Forum Wave admission is also source-ready",
]) requireText(localPlan, marker, localPlanPath);
for (const stale of [
  "current Pages composition has no live SLO snapshot source",
  "live SLO observation is not fabricated by Pages",
  "Supply and retain observed provider-health evidence from a real composition/runtime source",
]) forbidText(localPlan, stale, `${localPlanPath}: stale current state`);
forbidText(
  localOpen,
  "Connect the next production consumer's concrete tenant-scoped store",
  `${localPlanPath}: open results`,
);
for (const marker of [
  "Execute the exact provider-health maintainer chain",
  "take the explicit Pages gate owner + rollback decision",
  "run `forum_page_builder_wave_admission_v1`",
]) requireText(localOpen, marker, `${localPlanPath}: open results`);

for (const marker of [
  "Provider health observation/evaluator/binding/consumer chain: source-ready",
  "Observed-health runtime harness/owner acceptance: source-ready",
  "Pages reference-consumer gate acceptance: source-ready",
  "Forum Fly adapter/component registry: source-ready",
  "Forum owner preview transport/Pages host composition: source-ready",
  "Forum owner-backed property editing: source-ready",
  "Forum Wave admission: source-ready",
  "missing, invalid, expired or uninstalled accepted health packet",
]) requireText(centralPlan, marker, centralPlanPath);
for (const stale of [
  "Live SLO health remains a separate open cursor",
  "Live SLO health is deliberately `unobserved` until a real source exists",
  "Connect a real provider-health observation source to the admin status seam",
]) forbidText(centralPlan, stale, `${centralPlanPath}: stale current state`);

for (const marker of [
  "pages-reference-consumer-rollout-source-ready",
  "forum-wave-admission-source-ready",
  "base-plan-reconciliation-source-ready",
  "docs/modules/pages-page-builder-rollout-plan-actualization-2026-08-08.md",
  "docs/modules/forum-page-builder-wave-admission-actualization-2026-08-10.md",
  "docs/modules/pages-page-builder-base-plan-reconciliation-actualization-2026-08-10.md",
  "shared/local/central base plans expose the same provider-health",
  "server-owned rollout state",
  "FLY_CAPABILITY_DENIED",
  "FEATURE_DISABLED",
  "artifact/HTTP",
  "rollout runtime matrix",
  "owner sign-off + explicit rollback decision",
  "Forum Wave admission [source-ready / maintainer execution pending]",
  "Forum observed control-plane Wave [blocked on admitted exact-source inputs]",
  "No additional Pages/Page Builder rollout architecture slice",
]) requireText(parityActualization, marker, parityActualizationPath);

for (const marker of [
  "base-plan-reconciliation-source-ready",
  "shared-local-central-cursors-synchronized",
  "provider-health source architecture [ready]",
  "current provider health is not asserted by source inspection",
  "pages_reference_consumer_gate.accepted = false",
  "Forum Wave admission [source-ready / maintainer execution pending]",
  "Tests were not run",
]) requireText(basePlanReconciliation, marker, basePlanReconciliationPath);
for (const stale of [
  "observed-health-open",
  "no live SLO source exists yet",
  "current Pages composition has no live SLO snapshot source",
  "Live SLO health remains a separate open cursor",
]) requireText(basePlanReconciliation, stale, `${basePlanReconciliationPath}: documented stale marker`);

for (const marker of [
  "source-parity-current",
  "PR #3333",
  "PR #3337",
  "PR #3345",
  "PR #3353",
  "server-owned persisted rollout state",
  "FLY_CAPABILITY_DENIED",
  "feature-disabled / FEATURE_DISABLED",
  "artifact/HTTP",
  "rollout runtime matrix",
  "canonical rollout feature preflight",
  "reference-consumer candidate",
  "There is no additional source-only rollout architecture task",
  "pages_reference_consumer_gate` remains `accepted = false`",
  "Forum Wave remains blocked",
  "FFA/FBA promotion remains unclaimed",
]) requireText(rolloutActualization, marker, rolloutActualizationPath);
forbidText(
  rolloutActualization,
  "FLY_CAPABILITY_DENIED` is accepted as evidence for the provider `FEATURE_DISABLED",
  rolloutActualizationPath,
);

if (gate.artifact !== "pages_reference_consumer_gate_source") {
  failures.push(`${gatePath}: gate artifact identity drifted`);
}
if (gate.mode !== "source_ready" || gate.accepted !== false) {
  failures.push(`${gatePath}: source gate must remain source_ready and accepted=false`);
}
if (gate.source_recheck?.plan_parity !== "source_ready") {
  failures.push(`${gatePath}: plan parity source state must remain source_ready`);
}
if (!(gate.gate?.required_source_guards ?? []).includes(
  "crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs",
)) {
  failures.push(`${gatePath}: required source guards must include plan parity verification`);
}
if (gate.current_boundary?.execution_gate !== "pending") {
  failures.push(`${gatePath}: execution gate must remain pending`);
}
if (gate.current_boundary?.provider_health !== "unobserved") {
  failures.push(`${gatePath}: rollout-only source gate provider health must remain unobserved`);
}
if (gate.current_boundary?.forum_wave_blocker !== "pages_reference_consumer_gate") {
  failures.push(`${gatePath}: Forum Wave blocker drifted`);
}
if (
  gate.current_boundary?.four_profile_runtime_matrix !==
  "harness_source_ready_maintainer_execution_pending"
) failures.push(`${gatePath}: rollout matrix cursor drifted`);
if (
  gate.current_boundary?.canonical_feature_preflight !==
  "harness_source_ready_maintainer_execution_pending_FEATURE_DISABLED"
) failures.push(`${gatePath}: canonical feature-preflight cursor drifted`);
if (
  gate.current_boundary?.reference_candidate_rollout_matrix_input !==
    "source_ready_required_before_candidate" ||
  gate.current_boundary?.reference_candidate_feature_preflight_input !==
    "source_ready_required_before_candidate"
) failures.push(`${gatePath}: reference candidate rollout-input cursor drifted`);
if (
  gate.execution_harness?.required_inputs?.rollout_matrix !==
    "pages_builder_rollout_runtime_matrix_v1" ||
  gate.execution_harness?.required_inputs?.rollout_feature_preflight !==
    "pages_builder_rollout_feature_preflight_v1" ||
  gate.execution_harness?.canonical_feature_disabled_code_required !== "FEATURE_DISABLED"
) failures.push(`${gatePath}: reference candidate rollout contract drifted`);
if (gate.rollout_matrix_harness?.browser_intent_denial_code !== "FLY_CAPABILITY_DENIED") {
  failures.push(`${gatePath}: browser-intent denial contract drifted`);
}
if (
  gate.rollout_feature_preflight_harness?.feature_disabled_kind !== "feature-disabled" ||
  gate.rollout_feature_preflight_harness?.feature_disabled_code !== "FEATURE_DISABLED"
) failures.push(`${gatePath}: provider feature-disabled contract drifted`);

if (
  gateAcceptance.format !== "pages_reference_consumer_gate_acceptance_source_v1" ||
  gateAcceptance.status !== "source_ready_maintainer_execution_pending" ||
  gateAcceptance.output?.format !== "pages_reference_consumer_gate_acceptance_v1" ||
  gateAcceptance.output?.accepted_status !== "owner_accepted_pages_reference_consumer_gate" ||
  gateAcceptance.next_cursor?.forum_wave_admission !== "source_ready_maintainer_execution_pending" ||
  gateAcceptance.next_cursor?.forum_observed_wave !== "blocked_on_admitted_exact_source_inputs"
) failures.push(`${gateAcceptancePath}: Pages gate acceptance / Forum admission cursor drifted`);

for (const marker of [
  'id = "rustok.forum.widget-catalog"',
  'id = "rustok.forum.widget-preview"',
  'adapter_state = "fly_contract_ready"',
  'preview_data_state = "owner_preview_transport_ready"',
  'property_data_state = "owner_property_editor_ready"',
  'persistence_owner = "forum"',
  'authorization_owner = "forum"',
]) requireText(forumManifest, marker, forumManifestPath);

if (forumWave.mode !== "source_ready" || forumWave.execution_status !== "not_run_by_implementation_agent") {
  failures.push(`${forumWavePath}: Forum Wave must remain source_ready and unexecuted`);
}
if (
  forumWave.observed_run?.blocked_by !== "pages_reference_consumer_gate" ||
  forumWave.observed_run?.accepted_gate_evidence?.format !== "pages_reference_consumer_gate_acceptance_v1" ||
  forumWave.observed_run?.accepted_gate_evidence?.status !== "owner_accepted_pages_reference_consumer_gate" ||
  forumWave.observed_run?.wave_admission?.format !== "forum_page_builder_wave_admission_v1" ||
  forumWave.observed_run?.wave_admission?.status !== "forum_wave_inputs_admitted_observed_control_plane_pending" ||
  !(forumWave.observed_run?.required_evidence ?? []).includes("admission") ||
  forumWave.observed_run?.owner_acceptance?.required !== true ||
  forumWave.observed_run?.owner_acceptance?.source_status !== "source_ready_maintainer_execution_pending" ||
  forumWave.observed_run?.owner_acceptance?.format !== "forum_page_builder_wave_observed_acceptance_v1" ||
  forumWave.observed_run?.owner_acceptance?.accepted_status !== "owner_accepted_observed_control_plane_wave_promotion_review_pending" ||
  forumWave.observed_run?.owner_acceptance?.execution_status !== "maintainer_execution_pending"
) failures.push(`${forumWavePath}: Forum Wave accepted-gate/admission/owner-acceptance cursor drifted`);

if (
  forumWaveAdmission.format !== "forum_page_builder_wave_admission_source_v1" ||
  forumWaveAdmission.status !== "source_ready_maintainer_execution_pending" ||
  forumWaveAdmission.pages_gate_input?.format !== "pages_reference_consumer_gate_acceptance_v1" ||
  forumWaveAdmission.pages_gate_input?.required_status !== "owner_accepted_pages_reference_consumer_gate" ||
  forumWaveAdmission.output?.format !== "forum_page_builder_wave_admission_v1" ||
  forumWaveAdmission.output?.status !== "forum_wave_inputs_admitted_observed_control_plane_pending" ||
  forumWaveAdmission.lineage?.same_exact_source_commit_required_across_all_packets !== true ||
  forumWaveAdmission.lineage?.same_immutable_repo_digest_required_across_pages_gate_browser_and_serverfn !== true ||
  forumWaveAdmission.lineage?.deployment_digest_equality_does_not_upgrade_origin_binding_to_cryptographic_proof !== true ||
  forumWaveAdmission.observed_wave_boundary?.forum_wave_not_accepted !== true
) failures.push(`${forumWaveAdmissionPath}: Forum Wave admission source drifted`);

if (
  forumWaveObservedAcceptance.format !== "forum_page_builder_wave_observed_acceptance_source_v1" ||
  forumWaveObservedAcceptance.status !== "source_ready_maintainer_execution_pending" ||
  forumWaveObservedAcceptance.wave_evidence_input?.mode !== "live" ||
  forumWaveObservedAcceptance.wave_evidence_input?.provenance !== "observed_control_plane" ||
  forumWaveObservedAcceptance.wave_evidence_input?.execution_status !== "maintainer_verified" ||
  forumWaveObservedAcceptance.wave_evidence_input?.source_commit_must_equal_checkout_head !== true ||
  forumWaveObservedAcceptance.admission_input?.format !== "forum_page_builder_wave_admission_v1" ||
  forumWaveObservedAcceptance.admission_input?.status !== "forum_wave_inputs_admitted_observed_control_plane_pending" ||
  forumWaveObservedAcceptance.owner_decision?.runner !== "scripts/evidence/accept-forum-page-builder-wave.mjs" ||
  !(forumWaveObservedAcceptance.owner_decision?.decisions ?? []).includes("accept_observed_wave_evidence") ||
  !(forumWaveObservedAcceptance.owner_decision?.decisions ?? []).includes("reject") ||
  forumWaveObservedAcceptance.output?.format !== "forum_page_builder_wave_observed_acceptance_v1" ||
  forumWaveObservedAcceptance.output?.accepted_status !== "owner_accepted_observed_control_plane_wave_promotion_review_pending" ||
  forumWaveObservedAcceptance.promotion_boundary?.accepted_packet_is_eligible_input_for_explicit_ffa_fba_promotion_review !== true ||
  forumWaveObservedAcceptance.promotion_boundary?.accepted_packet_does_not_itself_promote_ffa_or_fba !== true ||
  forumWaveObservedAcceptance.promotion_boundary?.accepted_packet_does_not_mutate_control_plane_or_rollout !== true ||
  forumWaveObservedAcceptance.next_cursor?.ffa_fba_promotion_review !== "blocked_on_accepted_observed_wave_evidence"
) failures.push(`${forumWaveObservedAcceptancePath}: observed Wave owner-acceptance/promotion boundary drifted`);

if (failures.length > 0) {
  console.error("Pages / Page Builder plan parity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Pages / Page Builder plan parity verification passed");
