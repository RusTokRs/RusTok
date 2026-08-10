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
const rolloutActualizationPath = "docs/modules/pages-page-builder-rollout-plan-actualization-2026-08-08.md";
const gatePath = "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json";
const gateAcceptancePath = "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json";
const forumManifestPath = "crates/rustok-forum/rustok-module.toml";
const forumWavePath = "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json";
const forumWaveAdmissionPath = "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json";
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
const parityActualization = read(parityActualizationPath);
const rolloutActualization = read(rolloutActualizationPath);
const gate = parseJson(read(gatePath), gatePath);
const gateAcceptance = parseJson(read(gateAcceptancePath), gateAcceptancePath);
const forumManifest = read(forumManifestPath);
const forumWave = parseJson(read(forumWavePath), forumWavePath);
const forumWaveAdmission = parseJson(read(forumWaveAdmissionPath), forumWaveAdmissionPath);

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
]) requireText(sharedPlan, marker, `${sharedPlanPath}: status`);
forbidText(sharedPlan, "forum-fly-adapter-open", `${sharedPlanPath}: status`);

for (const marker of [
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
  "acceptance remains `false`",
]) requireText(sharedCurrent, marker, `${sharedPlanPath}: current reconciliation`);

for (const stale of [
  "adapter_state = \"pending\"",
  "because Forum has no real Fly component registry or `ContributionAdapter` yet",
  "The next contribution source cursor is the real Forum Fly adapter/component-registry slice",
]) forbidText(sharedCurrent, stale, `${sharedPlanPath}: current reconciliation`);

for (const marker of [
  "Maintainer executes and accepts `pages_reference_consumer_gate`",
  "execute the retained Forum browser/runtime/deployment-attestation packets",
  "health remains `unobserved`",
  "Promote FFA/FBA only after",
]) requireText(sharedNext, marker, `${sharedPlanPath}: next cursor`);

for (const marker of [
  "Forum is the second production consumer",
  "pages_reference_consumer_gate",
  "accepted = false",
  "execute the retained Forum browser/runtime/deployment-attestation",
]) requireText(localPlan, marker, localPlanPath);
forbidText(
  localOpen,
  "Connect the next production consumer's concrete tenant-scoped store",
  `${localPlanPath}: open results`,
);

for (const marker of [
  "Forum Fly adapter/component registry: source-ready",
  "Forum owner preview transport/Pages host composition: source-ready",
  "Forum owner-backed property editing: source-ready",
]) requireText(centralPlan, marker, centralPlanPath);

for (const marker of [
  "pages-reference-consumer-rollout-source-ready",
  "forum-wave-admission-source-ready",
  "docs/modules/pages-page-builder-rollout-plan-actualization-2026-08-08.md",
  "docs/modules/forum-page-builder-wave-admission-actualization-2026-08-10.md",
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
  failures.push(`${gatePath}: provider health must remain unobserved`);
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
  !(forumWave.observed_run?.required_evidence ?? []).includes("admission")
) failures.push(`${forumWavePath}: Forum Wave accepted-gate/admission cursor drifted`);

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

if (failures.length > 0) {
  console.error("Pages / Page Builder plan parity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Pages / Page Builder plan parity verification passed");
