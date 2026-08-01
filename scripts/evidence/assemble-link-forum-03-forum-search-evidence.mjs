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
  "crates/rustok-forum/contracts/forum-search-link-forum-03-evidence-assembler.json";
const forumPlanPath = "crates/rustok-forum/docs/implementation-plan.md";
const d0Path =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const d12ContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-retained-evidence-promotion.json";
const candidatePath =
  "target/forum-search-versioned-invalidation-runtime-promotion-candidate.json";
const aggregatePath =
  "target/forum-search-versioned-invalidation-runtime-evidence.json";
const d8Path =
  "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json";
const d9Path =
  "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json";
const d10Path =
  "target/forum-search-versioned-invalidation-normal-delivery-evidence.json";
const outputPath =
  "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json";

const frozenScenarioIds = [
  "normal_delivery",
  "legacy_first_duplicate",
  "typed_first_duplicate",
  "acknowledgement_failure_restart",
  "raw_poison_dlq_redelivery",
  "semantic_poison_identity_conflict",
  "missing_delivery_owner_repair",
  "multi_process_serialization",
  "deletion_acl_ordering",
  "search_disabled_profile",
];
const sourceTaskOrder = [
  "FORUM-23B2G2B3D2",
  "FORUM-23B2G2B3D3",
  "FORUM-23B2G2B3D4",
  "FORUM-23B2G2B3D5",
  "FORUM-23B2G2B3D6",
  "FORUM-23B2G2B3D7",
  "FORUM-23B2G2B3D8",
  "FORUM-23B2G2B3D9",
  "FORUM-23B2G2B3D10",
];
const selectedSources = [
  {
    task: "FORUM-23B2G2B3D8",
    contract:
      "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1",
    path: d8Path,
    scenario: "deletion_acl_ordering",
    profile: "no_broker",
  },
  {
    task: "FORUM-23B2G2B3D9",
    contract:
      "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1",
    path: d9Path,
    scenario: "search_disabled_profile",
    profile: "no_broker",
  },
  {
    task: "FORUM-23B2G2B3D10",
    contract: "forum_search_versioned_invalidation_normal_delivery_evidence_v1",
    path: d10Path,
    scenario: "normal_delivery",
    profile: "external_iggy",
  },
];
const remainingScope = [
  "translation projection and retrieval",
  "real moderation approval transition into Search visibility",
  "topic move and category-scope projection update",
  "exact private and trusted-channel exclusion runtime profile",
  "separate review of this partial artifact before any canonical plan change",
];

function fail(message) {
  throw new Error(`LINK-FORUM-03 core evidence assembly failed: ${message}`);
}

function readBytes(path) {
  try {
    return readFileSync(resolve(root, path));
  } catch (error) {
    fail(`cannot read ${path}: ${error.message}`);
  }
}

function parseJson(path, bytes) {
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

function requireExactArray(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} order or membership drifted`);
  }
}

function requireNonEmptyString(value, label) {
  if (typeof value !== "string" || value.trim() !== value || value.length === 0) {
    fail(`${label} must be a non-empty trimmed string`);
  }
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    fail(`${label} must be one lowercase forty-character Git SHA`);
  }
}

function requireDigest(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    fail(`${label} must be one lowercase SHA-256 digest`);
  }
}

function requireTimestamp(value, label) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) {
    fail(`${label} must be an ISO timestamp`);
  }
}

function requireFacts(value, label) {
  requireObject(value, label);
  if (Object.keys(value).length === 0) {
    fail(`${label} must not be empty`);
  }
}

function currentHead() {
  let head;
  try {
    head = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  } catch (error) {
    fail(`git rev-parse HEAD failed: ${error.message}`);
  }
  requireCommit(head, "git rev-parse HEAD");
  return head;
}

function validateMachineContract(contract) {
  requireObject(contract, "D13 machine contract");
  if (
    contract.contract !== "forum_search_link_forum_03_evidence_assembler_v1" ||
    contract.task !== "FORUM-23B2G2B3D13" ||
    contract.target_link !== "LINK-FORUM-03" ||
    contract.coverage !== "ordering_visibility_and_search_disabled_core_only" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.canonical_plan !== forumPlanPath ||
    contract.d0_parent !== d0Path ||
    contract.d12_contract !== d12ContractPath ||
    contract.assembler !==
      "scripts/evidence/assemble-link-forum-03-forum-search-evidence.mjs" ||
    contract.output_artifact?.path !== outputPath ||
    contract.output_artifact?.hand_editing_forbidden !== true ||
    contract.output_artifact?.source_commit_required !== true ||
    contract.output_artifact?.atomic_replace !== true ||
    contract.output_artifact?.automatic_canonical_source_mutation !== false
  ) {
    fail("D13 machine contract identity or output policy drifted");
  }
  requireExactArray(
    contract.required_inputs,
    [candidatePath, aggregatePath, d8Path, d9Path, d10Path],
    "D13 required inputs",
  );
  requireExactArray(
    contract.remaining_link_forum_03_runtime_scope,
    remainingScope,
    "D13 remaining LINK scope",
  );
}

function validateCanonicalPlan(plan) {
  for (const marker of [
    "| `FORUM-23` | `in_progress` |",
    "maintainer PostgreSQL/Iggy plus LINK-FORUM-03 runtime evidence remain",
    "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |",
    "## `LINK-FORUM-03` — index and search",
    "**Status:** `planned`",
    "**Dependencies:** FORUM-20/23",
    "Prove publish, translation, moderation approval, move, hide/delete, ACL change,",
    "out-of-order events and search-disabled behavior",
  ]) {
    if (!plan.includes(marker)) {
      fail(`canonical Forum plan is missing pending marker: ${marker}`);
    }
  }
  if (
    plan.includes("| `LINK-FORUM-03` | `done` |") ||
    plan.includes("LINK-FORUM-03 runtime evidence passed")
  ) {
    fail("canonical Forum plan already claims LINK-FORUM-03 completion");
  }
}

function validateD0(d0) {
  requireObject(d0, "D0 parent contract");
  if (
    d0.contract !== "forum_search_versioned_invalidation_runtime_evidence_v1" ||
    d0.task !== "FORUM-23B2G2B3D0" ||
    d0.status !== "source_ready_maintainer_execution_pending" ||
    d0.evidence_artifact?.path !== aggregatePath
  ) {
    fail("D0 parent identity, status or output path drifted");
  }
  requireExactArray(
    d0.required_scenarios?.map(({ id }) => id),
    frozenScenarioIds,
    "D0 frozen scenarios",
  );
}

function validateD12Contract(contract) {
  requireObject(contract, "D12 machine contract");
  if (
    contract.contract !==
      "forum_search_versioned_invalidation_retained_evidence_promotion_v1" ||
    contract.task !== "FORUM-23B2G2B3D12" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.aggregate_artifact !== aggregatePath ||
    contract.promotion_candidate?.path !== candidatePath ||
    contract.proposed_transition?.from !==
      "source_ready_maintainer_execution_pending" ||
    contract.proposed_transition?.to !== "runtime_evidence_reviewed" ||
    contract.proposed_transition?.requires_separate_canonical_source_pull_request !==
      true ||
    contract.proposed_transition?.closes_forum_23 !== false ||
    contract.proposed_transition?.closes_link_forum_03 !== false
  ) {
    fail("D12 contract identity or transition boundary drifted");
  }
}

function validateSource(entry, head) {
  const bytes = readBytes(entry.path);
  const artifact = parseJson(entry.path, bytes);
  requireObject(artifact, entry.path);
  if (
    artifact.task !== entry.task ||
    artifact.contract !== entry.contract ||
    artifact.source_commit !== head ||
    artifact.database_backend !== "postgresql"
  ) {
    fail(`${entry.path} identity, commit or database profile drifted`);
  }
  requireTimestamp(artifact.generated_at, `${entry.path}.generated_at`);
  if (entry.profile === "no_broker") {
    if (artifact.broker_used !== false) {
      fail(`${entry.path} must report broker_used false`);
    }
  } else if (
    artifact.delivery_profile !== "outbox_iggy" ||
    artifact.consumer_group !== "rustok-search-forum-projection-v1" ||
    artifact.topic !== "domain" ||
    typeof artifact.stream !== "string" ||
    artifact.stream.trim().length === 0
  ) {
    fail(`${entry.path} external Iggy profile drifted`);
  }
  if (!Array.isArray(artifact.scenario_results)) {
    fail(`${entry.path}.scenario_results must be an array`);
  }
  requireExactArray(
    artifact.scenario_results.map(({ id }) => id),
    [entry.scenario],
    `${entry.path} scenarios`,
  );
  const scenario = artifact.scenario_results[0];
  if (scenario.result !== "passed") {
    fail(`${entry.path} scenario did not pass`);
  }
  requireFacts(scenario.facts, `${entry.path} scenario facts`);
  return {
    entry,
    artifact,
    scenario,
    bytes,
    digest: sha256(bytes),
  };
}

function findRetained(records, task, label) {
  if (!Array.isArray(records)) {
    fail(`${label} must be an array`);
  }
  const matches = records.filter((record) => record.task === task);
  if (matches.length !== 1) {
    fail(`${label} must retain ${task} exactly once`);
  }
  return matches[0];
}

function validateRetainedRecord(record, source, head, label) {
  if (
    record.task !== source.entry.task ||
    record.contract !== source.entry.contract ||
    record.path !== source.entry.path ||
    record.source_commit !== head ||
    record.generated_at !== source.artifact.generated_at ||
    record.sha256 !== source.digest ||
    record.byte_length !== source.bytes.length
  ) {
    fail(`${label} metadata or digest drifted`);
  }
  requireExactArray(
    record.scenario_ids,
    [source.entry.scenario],
    `${label} scenarios`,
  );
}

function validateAggregate(aggregate, aggregateBytes, d0Bytes, head, sources) {
  requireObject(aggregate, "D0 aggregate artifact");
  if (
    aggregate.contract !== "forum_search_versioned_invalidation_runtime_evidence_v1" ||
    aggregate.task !== "FORUM-23B2G2B3D0" ||
    aggregate.status !== "runtime_evidence_assembled" ||
    aggregate.source_commit !== head ||
    aggregate.database_backend !== "postgresql" ||
    aggregate.delivery_profile !== "outbox_iggy" ||
    aggregate.consumer_group !== "rustok-search-forum-projection-v1" ||
    aggregate.topic !== "domain"
  ) {
    fail("aggregate identity, commit or runtime profile drifted");
  }
  requireTimestamp(aggregate.generated_at, "aggregate.generated_at");
  if (!Array.isArray(aggregate.scenario_results)) {
    fail("aggregate scenario_results must be an array");
  }
  requireExactArray(
    aggregate.scenario_results.map(({ id }) => id),
    frozenScenarioIds,
    "aggregate frozen scenarios",
  );
  for (const scenario of aggregate.scenario_results) {
    if (scenario.result !== "passed") {
      fail(`aggregate scenario ${scenario.id} did not pass`);
    }
    requireFacts(scenario.facts, `aggregate scenario ${scenario.id} facts`);
  }
  if (!Array.isArray(aggregate.source_artifacts)) {
    fail("aggregate source_artifacts must be an array");
  }
  requireExactArray(
    aggregate.source_artifacts.map(({ task }) => task),
    sourceTaskOrder,
    "aggregate source tasks",
  );
  requireObject(aggregate.assembly, "aggregate assembly record");
  if (
    aggregate.assembly.parent_contract !== d0Path ||
    aggregate.assembly.parent_contract_sha256 !== sha256(d0Bytes) ||
    aggregate.assembly.input_artifact_count !== 9 ||
    aggregate.assembly.frozen_scenario_count !== 10 ||
    aggregate.assembly.all_inputs_same_source_commit !== true ||
    aggregate.assembly.source_commit_matches_current_head !== true ||
    aggregate.assembly.output_written_after_complete_validation !== true
  ) {
    fail("aggregate assembly record is incomplete or stale");
  }

  const selected = {};
  for (const source of sources) {
    const retained = findRetained(
      aggregate.source_artifacts,
      source.entry.task,
      "aggregate source_artifacts",
    );
    validateRetainedRecord(retained, source, head, `aggregate ${source.entry.task}`);
    const matches = aggregate.scenario_results.filter(
      ({ id }) => id === source.entry.scenario,
    );
    if (matches.length !== 1) {
      fail(`aggregate must retain ${source.entry.scenario} exactly once`);
    }
    const scenario = matches[0];
    if (
      scenario.source_task !== source.entry.task ||
      scenario.source_contract !== source.entry.contract ||
      scenario.source_artifact !== source.entry.path ||
      scenario.source_scenario_id !== source.entry.scenario ||
      JSON.stringify(scenario.facts) !== JSON.stringify(source.scenario.facts)
    ) {
      fail(`aggregate ${source.entry.scenario} attribution or facts drifted`);
    }
    selected[source.entry.scenario] = scenario;
  }
  return {
    digest: sha256(aggregateBytes),
    generatedAt: aggregate.generated_at,
    selected,
  };
}

function validateCandidate(
  candidate,
  candidateBytes,
  aggregateBytes,
  aggregateReview,
  d0,
  d0Bytes,
  head,
  sources,
) {
  requireObject(candidate, "D12 promotion candidate");
  if (
    candidate.contract !==
      "forum_search_versioned_invalidation_runtime_promotion_candidate_v1" ||
    candidate.task !== "FORUM-23B2G2B3D12" ||
    candidate.status !== "approved_for_canonical_status_promotion" ||
    candidate.source_commit !== head
  ) {
    fail("D12 candidate identity, status or source commit drifted");
  }
  requireTimestamp(candidate.reviewed_at, "candidate.reviewed_at");
  requireNonEmptyString(candidate.reviewer, "candidate.reviewer");
  requireObject(candidate.retention, "candidate.retention");
  requireNonEmptyString(candidate.retention.reference, "candidate.retention.reference");
  requireDigest(candidate.retention.attested_sha256, "candidate retained digest");
  if (
    candidate.retention.matches_reviewed_aggregate !== true ||
    candidate.retention.external_service_authentication_performed_by_script !== false
  ) {
    fail("candidate retention boundary drifted");
  }
  requireObject(candidate.parent_contract, "candidate.parent_contract");
  if (
    candidate.parent_contract.path !== d0Path ||
    candidate.parent_contract.sha256 !== sha256(d0Bytes) ||
    candidate.parent_contract.status_at_review !== d0.status
  ) {
    fail("candidate parent-contract identity or digest drifted");
  }
  requireObject(candidate.aggregate_artifact, "candidate.aggregate_artifact");
  if (
    candidate.aggregate_artifact.path !== aggregatePath ||
    candidate.aggregate_artifact.sha256 !== aggregateReview.digest ||
    candidate.aggregate_artifact.byte_length !== aggregateBytes.length ||
    candidate.aggregate_artifact.generated_at !== aggregateReview.generatedAt ||
    candidate.retention.attested_sha256 !== aggregateReview.digest
  ) {
    fail("candidate aggregate metadata or retained digest drifted");
  }
  requireExactArray(
    candidate.aggregate_artifact.scenario_ids,
    frozenScenarioIds,
    "candidate aggregate scenarios",
  );
  if (!Array.isArray(candidate.source_artifacts)) {
    fail("candidate source_artifacts must be an array");
  }
  requireExactArray(
    candidate.source_artifacts.map(({ task }) => task),
    sourceTaskOrder,
    "candidate source tasks",
  );
  for (const source of sources) {
    const retained = findRetained(
      candidate.source_artifacts,
      source.entry.task,
      "candidate source_artifacts",
    );
    validateRetainedRecord(retained, source, head, `candidate ${source.entry.task}`);
  }
  requireObject(candidate.validation, "candidate.validation");
  for (const field of [
    "all_ten_frozen_scenarios_passed",
    "all_nine_source_artifacts_revalidated",
    "all_source_digests_match_aggregate",
    "aggregate_parent_digest_matches_current_d0",
    "aggregate_source_commit_matches_current_head",
    "retained_digest_attested_by_maintainer",
  ]) {
    if (candidate.validation[field] !== true) {
      fail(`candidate validation.${field} must be true`);
    }
  }
  if (
    candidate.proposed_transition?.from !==
      "source_ready_maintainer_execution_pending" ||
    candidate.proposed_transition?.to !== "runtime_evidence_reviewed" ||
    candidate.proposed_transition?.separate_canonical_source_pull_request_required !==
      true ||
    candidate.proposed_transition?.canonical_source_mutated_by_reviewer !== false ||
    candidate.proposed_transition?.closes_forum_23 !== false ||
    candidate.proposed_transition?.closes_link_forum_03 !== false
  ) {
    fail("candidate proposed transition boundary drifted");
  }
  return {
    digest: sha256(candidateBytes),
    reviewedAt: candidate.reviewed_at,
    reviewer: candidate.reviewer,
    retentionReference: candidate.retention.reference,
    retainedAggregateDigest: candidate.retention.attested_sha256,
  };
}

if (process.argv.length !== 2) {
  fail("this assembler accepts no command-line arguments");
}

const head = currentHead();
const contractBytes = readBytes(contractPath);
validateMachineContract(parseJson(contractPath, contractBytes));
const forumPlanBytes = readBytes(forumPlanPath);
validateCanonicalPlan(forumPlanBytes.toString("utf8"));
const d0Bytes = readBytes(d0Path);
const d0 = parseJson(d0Path, d0Bytes);
validateD0(d0);
const d12ContractBytes = readBytes(d12ContractPath);
validateD12Contract(parseJson(d12ContractPath, d12ContractBytes));

const sources = selectedSources.map((entry) => validateSource(entry, head));
const aggregateBytes = readBytes(aggregatePath);
const aggregate = parseJson(aggregatePath, aggregateBytes);
const aggregateReview = validateAggregate(
  aggregate,
  aggregateBytes,
  d0Bytes,
  head,
  sources,
);
const candidateBytes = readBytes(candidatePath);
const candidate = parseJson(candidatePath, candidateBytes);
const candidateReview = validateCandidate(
  candidate,
  candidateBytes,
  aggregateBytes,
  aggregateReview,
  d0,
  d0Bytes,
  head,
  sources,
);

const evidenceByScenario = Object.fromEntries(
  sources.map((source) => [
    source.entry.scenario,
    {
      source_task: source.entry.task,
      source_contract: source.entry.contract,
      source_artifact: source.entry.path,
      source_sha256: source.digest,
      facts: aggregateReview.selected[source.entry.scenario].facts,
    },
  ]),
);
const output = {
  contract: "link_forum_03_forum_index_search_ordering_visibility_evidence_v1",
  task: "LINK-FORUM-03",
  source_slice: "FORUM-23B2G2B3D13",
  status: "partial_runtime_evidence_assembled",
  coverage: "ordering_visibility_and_search_disabled_core_only",
  source_commit: head,
  generated_at: new Date().toISOString(),
  runtime_profile: {
    database_backend: "postgresql",
    delivery_profile: "outbox_iggy",
    consumer_group: "rustok-search-forum-projection-v1",
    topic: "domain",
  },
  selected_scenario_evidence: evidenceByScenario,
  retained_lineage: {
    canonical_plan_path: forumPlanPath,
    canonical_plan_sha256: sha256(forumPlanBytes),
    d0_parent_path: d0Path,
    d0_parent_sha256: sha256(d0Bytes),
    aggregate_path: aggregatePath,
    aggregate_sha256: aggregateReview.digest,
    promotion_candidate_path: candidatePath,
    promotion_candidate_sha256: candidateReview.digest,
    reviewed_at: candidateReview.reviewedAt,
    reviewer: candidateReview.reviewer,
    retention_reference: candidateReview.retentionReference,
    retained_aggregate_sha256: candidateReview.retainedAggregateDigest,
    external_retention_authentication_performed_by_assembler: false,
  },
  assertions: {
    real_forum_owner_to_iggy_to_search_to_storefront_trace_passed: true,
    projection_completed_before_delivery_covered_checkpoint: true,
    out_of_order_and_duplicate_delivery_did_not_restore_denied_content: true,
    stale_denied_rows_were_reauthorized_before_items_totals_and_facets: true,
    forum_owner_writes_survived_search_disabled_profile: true,
    late_search_recovery_rebuilt_from_owner_revision_ledger: true,
    selected_proofs_share_one_reviewed_source_commit: true,
    canonical_source_mutated_by_assembler: false,
  },
  remaining_link_forum_03_runtime_scope: remainingScope,
  canonical_transition: {
    link_status_before_review: "planned",
    status_change_allowed_from_this_artifact: false,
    reason: "partial ordering, visibility and Search-disabled core evidence only",
    separate_follow_up_runtime_evidence_required: true,
    closes_forum_23_automatically: false,
    closes_link_forum_03_automatically: false,
  },
};

const absoluteOutput = resolve(root, outputPath);
mkdirSync(dirname(absoluteOutput), { recursive: true });
const temporaryOutput = `${absoluteOutput}.${process.pid}.${Date.now()}.tmp`;
try {
  writeFileSync(temporaryOutput, `${JSON.stringify(output, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  renameSync(temporaryOutput, absoluteOutput);
} catch (error) {
  rmSync(temporaryOutput, { force: true });
  fail(`atomic LINK-FORUM-03 output write failed: ${error.message}`);
}

console.log(`wrote validated partial LINK-FORUM-03 evidence to ${outputPath}`);
