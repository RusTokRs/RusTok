#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..", "..");

const localPlanPath = "crates/modules/rustok-page-builder/docs/implementation-plan.md";
const gatePath = "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json";
const gateAcceptancePath = "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json";
const forumManifestPath = "crates/modules/rustok-forum/rustok-module.toml";
const forumWavePath = "crates/modules/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json";
const forumWaveAdmissionPath = "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json";
const forumWaveObservedAcceptancePath = "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-wave-observed-acceptance-source.json";
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

const localPlan = read(localPlanPath);
const gate = parseJson(read(gatePath), gatePath);
const gateAcceptance = parseJson(read(gateAcceptancePath), gateAcceptancePath);
const forumManifest = read(forumManifestPath);
const forumWave = parseJson(read(forumWavePath), forumWavePath);
const forumWaveAdmission = parseJson(read(forumWaveAdmissionPath), forumWaveAdmissionPath);
const forumWaveObservedAcceptance = parseJson(
  read(forumWaveObservedAcceptancePath),
  forumWaveObservedAcceptancePath,
);

const localOpen = section(localPlan, "## Open results", "## Verification");

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
  "crates/modules/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs",
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
