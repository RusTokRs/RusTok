#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";

const root = process.cwd();
const contractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-promotion.json";
const d17ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-assembler.json";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const completePath =
  "target/link-forum-03-forum-index-search-complete-evidence.json";
const candidatePath =
  "target/link-forum-03-forum-index-search-complete-promotion-candidate.json";
const reviewerEnv = "RUSTOK_LINK_FORUM_03_EVIDENCE_REVIEWER";
const retentionRefEnv = "RUSTOK_LINK_FORUM_03_EVIDENCE_RETENTION_REF";
const retainedShaEnv = "RUSTOK_LINK_FORUM_03_EVIDENCE_RETAINED_SHA256";

const scenarioIds = [
  "normal_delivery",
  "deletion_acl_ordering",
  "search_disabled_profile",
  "translation_and_moderation_approval",
  "private_and_trusted_channel_exclusion",
  "topic_move_category_scope",
];
const d13StoredScenarioIds = [
  "deletion_acl_ordering",
  "search_disabled_profile",
  "normal_delivery",
];
const sourceSpecs = [
  {
    kind: "core",
    path: "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json",
    contract: "link_forum_03_forum_index_search_ordering_visibility_evidence_v1",
    task: "LINK-FORUM-03",
    sourceSlice: "FORUM-23B2G2B3D13",
    scenarios: scenarioIds.slice(0, 3),
    retainedScenarios: d13StoredScenarioIds,
  },
  {
    kind: "extension",
    path: "target/forum-search-link-forum-03-translation-moderation-evidence.json",
    contract: "forum_search_link_forum_03_translation_moderation_evidence_v1",
    task: "FORUM-23B2G2B3D14",
    scenarios: ["translation_and_moderation_approval"],
  },
  {
    kind: "extension",
    path: "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json",
    contract: "forum_search_link_forum_03_private_trusted_exclusion_proof_v1",
    task: "FORUM-23B2G2B3D15",
    scenarios: ["private_and_trusted_channel_exclusion"],
  },
  {
    kind: "extension",
    path: "target/forum-search-link-forum-03-topic-move-evidence.json",
    contract: "forum_search_link_forum_03_topic_move_evidence_v1",
    task: "FORUM-23B2G2B3D16",
    scenarios: ["topic_move_category_scope"],
  },
];

function fail(message) {
  throw new Error(`LINK-FORUM-03 complete evidence review failed: ${message}`);
}
function readBytes(path) {
  try {
    return readFileSync(resolve(root, path));
  } catch (error) {
    fail(`cannot read ${path}: ${error.message}`);
  }
}
function parseJson(path, bytes = readBytes(path)) {
  try {
    return JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    fail(`${path} is not valid JSON: ${error.message}`);
  }
}
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
function requireObject(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
}
function requireString(value, label) {
  if (typeof value !== "string" || value.trim() === "") {
    fail(`${label} must be a non-empty string`);
  }
}
function requireFacts(value, label) {
  requireObject(value, label);
  if (Object.keys(value).length === 0) fail(`${label} must not be empty`);
}
function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    fail(`${label} must be one lowercase forty-character Git commit SHA`);
  }
}
function requireDigest(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    fail(`${label} must be one lowercase SHA-256 digest`);
  }
}
function requireTimestamp(value, label) {
  requireString(value, label);
  if (!Number.isFinite(Date.parse(value))) fail(`${label} must be an ISO timestamp`);
}
function exactArray(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} order or membership drifted`);
  }
}
function exactJson(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail(`${label} drifted`);
}
function boundedEnv(name, minimum, maximum) {
  const value = process.env[name];
  if (typeof value !== "string") fail(`${name} must be set`);
  if (value.trim() !== value || value.length < minimum || value.length > maximum) {
    fail(`${name} must contain ${minimum}..${maximum} characters without surrounding whitespace`);
  }
  if (value.split("").some((character) => /[\u0000-\u001f\u007f]/.test(character))) {
    fail(`${name} must not contain control characters`);
  }
  return value;
}
function currentHead() {
  let value;
  try {
    value = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  } catch (error) {
    fail(`git rev-parse HEAD failed: ${error.message}`);
  }
  requireCommit(value, "git rev-parse HEAD");
  return value;
}

function validateReviewContract(contract) {
  requireObject(contract, "D18 machine contract");
  if (
    contract.contract !== "forum_search_link_forum_03_complete_evidence_promotion_v1" ||
    contract.task !== "FORUM-23B2G2B3D18" ||
    contract.target_link !== "LINK-FORUM-03" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.complete_assembler_contract !== d17ContractPath ||
    contract.complete_artifact !== completePath ||
    contract.reviewer !== "scripts/evidence/review-link-forum-03-complete-forum-search-evidence.mjs" ||
    contract.promotion_candidate?.path !== candidatePath
  ) {
    fail("D18 machine contract identity or path drifted");
  }
  exactArray(contract.required_scenarios, scenarioIds, "D18 required scenarios");
  exactArray(
    contract.required_source_artifacts,
    sourceSpecs.map(({ path }) => path),
    "D18 source artifacts",
  );
  exactArray(
    contract.required_attestations,
    [reviewerEnv, retentionRefEnv, retainedShaEnv],
    "D18 attestations",
  );
  if (
    contract.proposed_transition?.task !== "LINK-FORUM-03" ||
    contract.proposed_transition?.from !== "planned" ||
    contract.proposed_transition?.to !== "done" ||
    contract.proposed_transition?.requires_separate_canonical_source_pull_request !== true ||
    contract.proposed_transition?.canonical_source_mutated_by_reviewer !== false ||
    contract.proposed_transition?.promotes_forum_21 !== false ||
    contract.proposed_transition?.promotes_forum_23 !== false
  ) {
    fail("D18 proposed transition boundary drifted");
  }
}

function validateD17Contract(contract) {
  requireObject(contract, "D17 machine contract");
  if (
    contract.contract !== "forum_search_link_forum_03_complete_evidence_assembler_v1" ||
    contract.task !== "FORUM-23B2G2B3D17" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.coverage !== "complete_canonical_runtime_scope_review_pending" ||
    contract.output_artifact?.path !== completePath ||
    contract.output_artifact?.status !== "complete_runtime_evidence_assembled_review_pending" ||
    contract.output_artifact?.automatic_canonical_source_mutation !== false
  ) {
    fail("D17 machine contract identity or boundary drifted");
  }
  exactArray(contract.required_scenarios, scenarioIds, "D17 required scenarios");
  exactArray(
    contract.required_inputs,
    sourceSpecs.map(({ path }) => path),
    "D17 required inputs",
  );
}

function validatePlan(plan) {
  for (const marker of [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "| `FORUM-23` | `in_progress` |",
    "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |",
  ]) {
    if (!plan.includes(marker)) fail(`canonical plan is missing marker: ${marker}`);
  }
  if (plan.includes("| `LINK-FORUM-03` | `done` | Forum/index/search ordering and visibility proof. |")) {
    fail("LINK-FORUM-03 was promoted before D18 review");
  }
}

function validateSource(spec, head) {
  const bytes = readBytes(spec.path);
  const artifact = parseJson(spec.path, bytes);
  requireObject(artifact, spec.path);
  if (spec.kind === "core") {
    if (
      artifact.contract !== spec.contract ||
      artifact.task !== spec.task ||
      artifact.source_slice !== spec.sourceSlice ||
      artifact.status !== "partial_runtime_evidence_assembled" ||
      artifact.coverage !== "ordering_visibility_and_search_disabled_core_only" ||
      artifact.source_commit !== head
    ) {
      fail(`${spec.path} identity status or source commit drifted`);
    }
    requireTimestamp(artifact.generated_at, `${spec.path}.generated_at`);
    requireObject(artifact.selected_scenario_evidence, `${spec.path}.selected_scenario_evidence`);
    exactArray(
      Object.keys(artifact.selected_scenario_evidence),
      spec.retainedScenarios,
      `${spec.path} stored scenario order`,
    );
    for (const scenarioId of spec.retainedScenarios) {
      const evidence = artifact.selected_scenario_evidence[scenarioId];
      requireObject(evidence, `${spec.path}.${scenarioId}`);
      requireString(evidence.source_task, `${spec.path}.${scenarioId}.source_task`);
      requireString(evidence.source_contract, `${spec.path}.${scenarioId}.source_contract`);
      requireString(evidence.source_artifact, `${spec.path}.${scenarioId}.source_artifact`);
      requireDigest(evidence.source_sha256, `${spec.path}.${scenarioId}.source_sha256`);
      requireFacts(evidence.facts, `${spec.path}.${scenarioId}.facts`);
    }
    requireObject(artifact.retained_lineage, `${spec.path}.retained_lineage`);
    requireString(artifact.retained_lineage.reviewer, `${spec.path}.retained_lineage.reviewer`);
    requireTimestamp(artifact.retained_lineage.reviewed_at, `${spec.path}.retained_lineage.reviewed_at`);
    requireString(
      artifact.retained_lineage.retention_reference,
      `${spec.path}.retained_lineage.retention_reference`,
    );
    requireDigest(
      artifact.retained_lineage.retained_aggregate_sha256,
      `${spec.path}.retained_lineage.retained_aggregate_sha256`,
    );
    if (
      artifact.retained_lineage.external_retention_authentication_performed_by_assembler !== false ||
      artifact.canonical_transition?.status_change_allowed_from_this_artifact !== false
    ) {
      fail(`${spec.path} retention or canonical boundary drifted`);
    }
  } else {
    if (
      artifact.contract !== spec.contract ||
      artifact.task !== spec.task ||
      artifact.source_commit !== head ||
      artifact.database_backend !== "postgresql" ||
      artifact.broker_used !== false
    ) {
      fail(`${spec.path} identity runtime profile or source commit drifted`);
    }
    requireTimestamp(artifact.generated_at, `${spec.path}.generated_at`);
    if (!Array.isArray(artifact.scenario_results)) {
      fail(`${spec.path}.scenario_results must be an array`);
    }
    exactArray(
      artifact.scenario_results.map(({ id }) => id),
      spec.scenarios,
      `${spec.path} scenarios`,
    );
    const scenario = artifact.scenario_results[0];
    if (scenario.result !== "passed") fail(`${spec.path} scenario did not pass`);
    requireFacts(scenario.facts, `${spec.path}.${scenario.id}.facts`);
  }
  return { spec, bytes, artifact, digest: sha256(bytes) };
}

function validateComplete(artifact, bytes, head, sources) {
  requireObject(artifact, "D17 complete artifact");
  if (
    artifact.contract !== "link_forum_03_forum_index_search_complete_evidence_v1" ||
    artifact.task !== "LINK-FORUM-03" ||
    artifact.source_slice !== "FORUM-23B2G2B3D17" ||
    artifact.status !== "complete_runtime_evidence_assembled_review_pending" ||
    artifact.coverage !== "canonical_link_forum_03_runtime_scope" ||
    artifact.source_commit !== head
  ) {
    fail("D17 complete artifact identity status or source commit drifted");
  }
  requireTimestamp(artifact.generated_at, "D17 complete artifact generated_at");
  exactArray(artifact.assembled_scenario_ids, scenarioIds, "D17 assembled scenarios");
  requireObject(artifact.scenario_evidence, "D17 scenario_evidence");
  exactArray(Object.keys(artifact.scenario_evidence), scenarioIds, "D17 scenario order");
  if (!Array.isArray(artifact.source_artifacts)) fail("D17 source_artifacts must be an array");
  exactArray(
    artifact.source_artifacts.map(({ path }) => path),
    sourceSpecs.map(({ path }) => path),
    "D17 source artifact order",
  );

  const sourceByPath = new Map(sources.map((source) => [source.spec.path, source]));
  for (const retained of artifact.source_artifacts) {
    const source = sourceByPath.get(retained.path);
    if (!source) fail(`D17 retains unknown source ${retained.path}`);
    if (
      retained.sha256 !== source.digest ||
      retained.byte_length !== source.bytes.length ||
      retained.generated_at !== source.artifact.generated_at ||
      retained.source_commit !== head
    ) {
      fail(`D17 source metadata drifted for ${retained.path}`);
    }
    if (source.spec.kind === "core") {
      exactArray(
        retained.scenario_ids,
        source.spec.retainedScenarios,
        `${retained.path} retained scenario_ids`,
      );
    } else if (
      retained.task !== source.spec.task ||
      retained.contract !== source.spec.contract ||
      retained.database_backend !== "postgresql" ||
      retained.broker_used !== false ||
      retained.scenario_id !== source.spec.scenarios[0]
    ) {
      fail(`D17 extension metadata drifted for ${retained.path}`);
    }
  }

  const core = sources[0];
  for (const scenarioId of core.spec.scenarios) {
    exactJson(
      artifact.scenario_evidence[scenarioId],
      core.artifact.selected_scenario_evidence[scenarioId],
      `D17 core scenario ${scenarioId}`,
    );
  }
  for (const source of sources.slice(1)) {
    const scenarioId = source.spec.scenarios[0];
    const sourceScenario = source.artifact.scenario_results[0];
    const evidence = artifact.scenario_evidence[scenarioId];
    requireObject(evidence, `D17 scenario ${scenarioId}`);
    if (
      evidence.source_task !== source.spec.task ||
      evidence.source_contract !== source.spec.contract ||
      evidence.source_artifact !== source.spec.path ||
      evidence.source_sha256 !== source.digest
    ) {
      fail(`D17 extension attribution drifted for ${scenarioId}`);
    }
    exactJson(evidence.facts, sourceScenario.facts, `D17 extension facts ${scenarioId}`);
  }

  requireObject(artifact.reviewed_core_lineage, "D17 reviewed_core_lineage");
  if (
    artifact.reviewed_core_lineage.source_artifact !== core.spec.path ||
    artifact.reviewed_core_lineage.source_sha256 !== core.digest ||
    artifact.reviewed_core_lineage.external_retention_authentication_performed_by_d17 !== false
  ) {
    fail("D17 reviewed core lineage drifted");
  }
  exactJson(
    artifact.reviewed_core_lineage.inherited_retained_lineage,
    core.artifact.retained_lineage,
    "D17 inherited retained lineage",
  );
  if (
    artifact.assertions?.canonical_link_runtime_scope_assembled !== true ||
    artifact.assertions?.complete_artifact_independently_reviewed !== false ||
    artifact.assertions?.complete_artifact_retention_attested !== false ||
    artifact.assertions?.canonical_source_mutated_by_assembler !== false ||
    artifact.canonical_transition?.status_change_allowed_from_this_artifact !== false ||
    artifact.canonical_transition?.separate_review_gate_required !== true ||
    artifact.canonical_transition?.separate_canonical_source_pull_request_required !== true ||
    artifact.canonical_transition?.closes_forum_21_automatically !== false ||
    artifact.canonical_transition?.closes_forum_23_automatically !== false ||
    artifact.canonical_transition?.closes_link_forum_03_automatically !== false
  ) {
    fail("D17 review or canonical boundary drifted");
  }
  return sha256(bytes);
}

if (process.argv.length !== 2) fail("this reviewer accepts no command-line arguments");
const reviewer = boundedEnv(reviewerEnv, 3, 128);
const retentionReference = boundedEnv(retentionRefEnv, 8, 2048);
const retainedSha = boundedEnv(retainedShaEnv, 64, 64);
requireDigest(retainedSha, retainedShaEnv);

const head = currentHead();
const contractBytes = readBytes(contractPath);
validateReviewContract(parseJson(contractPath, contractBytes));
const d17ContractBytes = readBytes(d17ContractPath);
validateD17Contract(parseJson(d17ContractPath, d17ContractBytes));
const planBytes = readBytes(planPath);
validatePlan(planBytes.toString("utf8"));
const sources = sourceSpecs.map((spec) => validateSource(spec, head));
const completeBytes = readBytes(completePath);
const completeArtifact = parseJson(completePath, completeBytes);
const completeDigest = validateComplete(completeArtifact, completeBytes, head, sources);
if (retainedSha !== completeDigest) {
  fail(`${retainedShaEnv} does not equal the exact complete artifact digest`);
}

const candidate = {
  contract: "link_forum_03_forum_index_search_complete_promotion_candidate_v1",
  task: "FORUM-23B2G2B3D18",
  target_link: "LINK-FORUM-03",
  status: "approved_for_canonical_status_promotion",
  source_commit: head,
  reviewed_at: new Date().toISOString(),
  reviewer,
  retention: {
    reference: retentionReference,
    attested_sha256: retainedSha,
    matches_reviewed_complete_artifact: true,
    external_service_authentication_performed_by_script: false,
    cryptographic_signature_created_by_script: false,
  },
  complete_artifact: {
    path: completePath,
    sha256: completeDigest,
    byte_length: completeBytes.length,
    generated_at: completeArtifact.generated_at,
    status_at_review: completeArtifact.status,
    coverage: completeArtifact.coverage,
    scenario_ids: scenarioIds,
  },
  assembler_contract: {
    path: d17ContractPath,
    sha256: sha256(d17ContractBytes),
    status_at_review: "source_ready_maintainer_execution_pending",
  },
  review_contract: {
    path: contractPath,
    sha256: sha256(contractBytes),
    status_at_review: "source_ready_maintainer_execution_pending",
  },
  canonical_plan: {
    path: planPath,
    sha256: sha256(planBytes),
    forum_21_status_at_review: "planned",
    forum_23_status_at_review: "in_progress",
    link_forum_03_status_at_review: "planned",
  },
  source_artifacts: sources.map(({ spec, artifact, bytes, digest }) => ({
    path: spec.path,
    task: spec.task,
    contract: spec.contract,
    source_commit: artifact.source_commit,
    generated_at: artifact.generated_at,
    sha256: digest,
    byte_length: bytes.length,
    scenario_ids: spec.scenarios,
  })),
  inherited_core_retention: {
    source_artifact: sources[0].spec.path,
    source_sha256: sources[0].digest,
    lineage: sources[0].artifact.retained_lineage,
    external_retention_authentication_performed_by_d18: false,
  },
  validation: {
    all_six_canonical_scenarios_revalidated: true,
    all_four_source_artifacts_revalidated: true,
    all_source_digests_match_complete_artifact: true,
    all_scenario_facts_match_retained_sources: true,
    complete_artifact_source_commit_matches_current_head: true,
    complete_artifact_was_review_pending_before_review: true,
    retained_digest_attested_by_maintainer: true,
    canonical_plan_remained_unmodified: true,
  },
  proposed_transition: {
    task: "LINK-FORUM-03",
    from: "planned",
    to: "done",
    separate_canonical_source_pull_request_required: true,
    canonical_source_mutated_by_reviewer: false,
    promotes_forum_21: false,
    promotes_forum_23: false,
  },
};

const absoluteCandidate = resolve(root, candidatePath);
mkdirSync(dirname(absoluteCandidate), { recursive: true });
const temporaryCandidate = `${absoluteCandidate}.${process.pid}.${Date.now()}.tmp`;
try {
  writeFileSync(temporaryCandidate, `${JSON.stringify(candidate, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  renameSync(temporaryCandidate, absoluteCandidate);
} catch (error) {
  rmSync(temporaryCandidate, { force: true });
  fail(`atomic promotion-candidate write failed: ${error.message}`);
}
console.log(`wrote reviewed LINK-FORUM-03 promotion candidate to ${candidatePath}`);
