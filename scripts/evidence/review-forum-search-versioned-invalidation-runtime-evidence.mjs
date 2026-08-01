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
const parentPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const contractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-retained-evidence-promotion.json";
const aggregatePath =
  "target/forum-search-versioned-invalidation-runtime-evidence.json";
const candidatePath =
  "target/forum-search-versioned-invalidation-runtime-promotion-candidate.json";
const reviewerEnv = "RUSTOK_FORUM_EVIDENCE_REVIEWER";
const retentionRefEnv = "RUSTOK_FORUM_EVIDENCE_RETENTION_REF";
const retainedShaEnv = "RUSTOK_FORUM_EVIDENCE_RETAINED_SHA256";
const canonicalConsumerGroup = "rustok-search-forum-projection-v1";
const canonicalTopic = "domain";

const frozenScenarios = [
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

const frozenScenarioSources = new Map([
  ["normal_delivery", ["FORUM-23B2G2B3D10", "normal_delivery"]],
  ["legacy_first_duplicate", ["FORUM-23B2G2B3D2", "legacy_first_duplicate"]],
  ["typed_first_duplicate", ["FORUM-23B2G2B3D2", "typed_first_duplicate"]],
  [
    "acknowledgement_failure_restart",
    ["FORUM-23B2G2B3D3", "acknowledgement_failure_restart"],
  ],
  ["raw_poison_dlq_redelivery", ["FORUM-23B2G2B3D4", "raw_poison_dlq_redelivery"]],
  [
    "semantic_poison_identity_conflict",
    ["FORUM-23B2G2B3D5", "semantic_poison_identity_conflict"],
  ],
  [
    "missing_delivery_owner_repair",
    ["FORUM-23B2G2B3D6", "missing_delivery_owner_repair"],
  ],
  [
    "multi_process_serialization",
    ["FORUM-23B2G2B3D7", "multi_process_serialization"],
  ],
  ["deletion_acl_ordering", ["FORUM-23B2G2B3D8", "deletion_acl_ordering"]],
  ["search_disabled_profile", ["FORUM-23B2G2B3D9", "search_disabled_profile"]],
]);

const manifest = [
  {
    task: "FORUM-23B2G2B3D2",
    contract: "forum_search_versioned_invalidation_postgres_ingress_evidence_v1",
    path: "target/forum-search-versioned-invalidation-postgres-ingress-evidence.json",
    scenarios: [
      "typed_ingress_admission",
      "legacy_first_duplicate",
      "typed_first_duplicate",
      "semantic_identity_conflict",
    ],
  },
  {
    task: "FORUM-23B2G2B3D3",
    contract: "forum_search_versioned_invalidation_ack_restart_evidence_v1",
    path: "target/forum-search-versioned-invalidation-ack-restart-evidence.json",
    scenarios: ["acknowledgement_failure_restart"],
  },
  {
    task: "FORUM-23B2G2B3D4",
    contract: "forum_search_versioned_invalidation_raw_poison_evidence_v1",
    path: "target/forum-search-versioned-invalidation-raw-poison-evidence.json",
    scenarios: ["raw_poison_dlq_redelivery"],
  },
  {
    task: "FORUM-23B2G2B3D5",
    contract: "forum_search_versioned_invalidation_semantic_poison_evidence_v1",
    path: "target/forum-search-versioned-invalidation-semantic-poison-evidence.json",
    scenarios: ["semantic_poison_identity_conflict"],
  },
  {
    task: "FORUM-23B2G2B3D6",
    contract: "forum_search_versioned_invalidation_missing_delivery_repair_evidence_v1",
    path: "target/forum-search-versioned-invalidation-missing-delivery-repair-evidence.json",
    scenarios: ["missing_delivery_owner_repair"],
  },
  {
    task: "FORUM-23B2G2B3D7",
    contract: "forum_search_versioned_invalidation_multi_process_evidence_v1",
    path: "target/forum-search-versioned-invalidation-multi-process-evidence.json",
    scenarios: ["multi_process_serialization"],
  },
  {
    task: "FORUM-23B2G2B3D8",
    contract: "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1",
    path: "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json",
    scenarios: ["deletion_acl_ordering"],
  },
  {
    task: "FORUM-23B2G2B3D9",
    contract: "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1",
    path: "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json",
    scenarios: ["search_disabled_profile"],
  },
  {
    task: "FORUM-23B2G2B3D10",
    contract: "forum_search_versioned_invalidation_normal_delivery_evidence_v1",
    path: "target/forum-search-versioned-invalidation-normal-delivery-evidence.json",
    scenarios: ["normal_delivery"],
  },
];

const groupedFields = [
  "owner_revision_rows",
  "typed_and_root_event_ids",
  "search_inbox_rows",
  "ingest_sequences",
  "owner_checkpoints",
  "poison_receipts",
  "dlq_receipts",
  "storefront_visibility_assertions",
];

function fail(message) {
  throw new Error(`Forum Search retained evidence review failed: ${message}`);
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

function requireFacts(value, label) {
  requireObject(value, label);
  if (Object.keys(value).length === 0) {
    fail(`${label} must not be empty`);
  }
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    fail(`${label} must be one lowercase forty-character Git commit SHA`);
  }
}

function requireTimestamp(value, label) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) {
    fail(`${label} must be an ISO timestamp`);
  }
}

function boundedEnv(name, minimum, maximum) {
  const value = process.env[name];
  if (typeof value !== "string") {
    fail(`${name} must be set`);
  }
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

function exactArray(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} order or membership drifted`);
  }
}

function validateParent(parent, aggregatePathValue) {
  requireObject(parent, "D0 parent contract");
  if (parent.contract !== "forum_search_versioned_invalidation_runtime_evidence_v1") {
    fail("D0 parent contract identity drifted");
  }
  if (parent.task !== "FORUM-23B2G2B3D0") {
    fail("D0 parent task identity drifted");
  }
  if (parent.status !== "source_ready_maintainer_execution_pending") {
    fail("D0 parent must remain pending before a promotion candidate is created");
  }
  exactArray(
    parent.required_scenarios?.map(({ id }) => id),
    frozenScenarios,
    "D0 frozen scenarios",
  );
  if (parent.evidence_artifact?.path !== aggregatePathValue) {
    fail("D0 aggregate evidence path drifted");
  }
  const d12 = (parent.source_ready_subproofs ?? []).find(
    ({ task }) => task === "FORUM-23B2G2B3D12",
  );
  if (!d12 || d12.contract !== contractPath || d12.reviewer !== import.meta.filename?.replace(`${root}/`, "")) {
    fail("D0 does not register the exact D12 retained-evidence reviewer");
  }
}

function validateSourceArtifact(entry, head) {
  const bytes = readBytes(entry.path);
  const artifact = parseJson(entry.path, bytes);
  requireObject(artifact, entry.path);
  if (artifact.contract !== entry.contract || artifact.task !== entry.task) {
    fail(`${entry.path} contract or task identity drifted`);
  }
  if (artifact.source_commit !== head) {
    fail(`${entry.path} source_commit does not equal current HEAD`);
  }
  requireTimestamp(artifact.generated_at, `${entry.path}.generated_at`);
  if (artifact.database_backend !== "postgresql") {
    fail(`${entry.path} must report PostgreSQL`);
  }
  if (!Array.isArray(artifact.scenario_results)) {
    fail(`${entry.path}.scenario_results must be an array`);
  }
  exactArray(
    artifact.scenario_results.map(({ id }) => id),
    entry.scenarios,
    `${entry.path} scenarios`,
  );
  const scenarios = new Map();
  for (const scenario of artifact.scenario_results) {
    requireObject(scenario, `${entry.path} scenario`);
    if (scenario.result !== "passed") {
      fail(`${entry.path} scenario ${scenario.id} did not pass`);
    }
    requireFacts(scenario.facts, `${entry.path} scenario ${scenario.id} facts`);
    if (scenarios.has(scenario.id)) {
      fail(`${entry.path} repeats scenario ${scenario.id}`);
    }
    scenarios.set(scenario.id, scenario);
  }
  return {
    entry,
    artifact,
    scenarios,
    bytes,
    digest: sha256(bytes),
  };
}

function validateGroupedEvidence(aggregate, sourceByTask) {
  const allowed = new Set(manifest.map(({ task, path }) => `${task}\u0000${path}`));
  for (const field of groupedFields) {
    const values = aggregate[field];
    if (!Array.isArray(values) || values.length === 0) {
      fail(`aggregate ${field} must be a non-empty array`);
    }
    for (const value of values) {
      requireObject(value, `aggregate ${field} entry`);
      const source = sourceByTask.get(value.source_task);
      if (!source || !allowed.has(`${value.source_task}\u0000${value.source_artifact}`)) {
        fail(`aggregate ${field} contains an unregistered source artifact`);
      }
      if (value.source_contract !== source.entry.contract) {
        fail(`aggregate ${field} source contract drifted`);
      }
      const scenario = source.scenarios.get(value.source_scenario_id);
      if (!scenario) {
        fail(`aggregate ${field} references an absent source scenario`);
      }
      requireFacts(value.facts, `aggregate ${field} retained facts`);
      if (JSON.stringify(value.facts) !== JSON.stringify(scenario.facts)) {
        fail(`aggregate ${field} facts differ from the retained source artifact`);
      }
    }
  }
}

function validateAggregate(aggregate, aggregateBytes, parentBytes, head, sources) {
  requireObject(aggregate, "aggregate D0 artifact");
  if (
    aggregate.contract !== "forum_search_versioned_invalidation_runtime_evidence_v1" ||
    aggregate.task !== "FORUM-23B2G2B3D0" ||
    aggregate.status !== "runtime_evidence_assembled"
  ) {
    fail("aggregate D0 identity or assembled status drifted");
  }
  if (aggregate.source_commit !== head) {
    fail("aggregate source_commit does not equal current HEAD");
  }
  requireTimestamp(aggregate.generated_at, "aggregate.generated_at");
  if (
    aggregate.database_backend !== "postgresql" ||
    aggregate.delivery_profile !== "outbox_iggy" ||
    aggregate.consumer_group !== canonicalConsumerGroup ||
    aggregate.topic !== canonicalTopic
  ) {
    fail("aggregate runtime profile drifted");
  }
  if (!Array.isArray(aggregate.scenario_results)) {
    fail("aggregate scenario_results must be an array");
  }
  exactArray(
    aggregate.scenario_results.map(({ id }) => id),
    frozenScenarios,
    "aggregate frozen scenarios",
  );
  for (const scenario of aggregate.scenario_results) {
    requireObject(scenario, `aggregate scenario ${scenario.id}`);
    if (scenario.result !== "passed") {
      fail(`aggregate scenario ${scenario.id} did not pass`);
    }
    const expectedSource = frozenScenarioSources.get(scenario.id);
    if (
      !expectedSource ||
      scenario.source_task !== expectedSource[0] ||
      scenario.source_scenario_id !== expectedSource[1]
    ) {
      fail(`aggregate scenario ${scenario.id} source attribution drifted`);
    }
    requireFacts(scenario.facts, `aggregate scenario ${scenario.id} facts`);
  }

  requireObject(aggregate.assembly, "aggregate assembly record");
  if (
    aggregate.assembly.parent_contract !== parentPath ||
    aggregate.assembly.parent_contract_sha256 !== sha256(parentBytes) ||
    aggregate.assembly.input_artifact_count !== manifest.length ||
    aggregate.assembly.frozen_scenario_count !== frozenScenarios.length ||
    aggregate.assembly.all_inputs_same_source_commit !== true ||
    aggregate.assembly.source_commit_matches_current_head !== true ||
    aggregate.assembly.output_written_after_complete_validation !== true
  ) {
    fail("aggregate assembly record is incomplete or stale");
  }

  if (!Array.isArray(aggregate.source_artifacts)) {
    fail("aggregate source_artifacts must be an array");
  }
  exactArray(
    aggregate.source_artifacts.map(({ task }) => task),
    manifest.map(({ task }) => task),
    "aggregate source artifacts",
  );
  const sourceByTask = new Map(sources.map((source) => [source.entry.task, source]));
  for (const retained of aggregate.source_artifacts) {
    const source = sourceByTask.get(retained.task);
    if (!source) {
      fail(`aggregate retains unknown source task ${retained.task}`);
    }
    if (
      retained.contract !== source.entry.contract ||
      retained.path !== source.entry.path ||
      retained.source_commit !== head ||
      retained.generated_at !== source.artifact.generated_at ||
      retained.sha256 !== source.digest ||
      retained.byte_length !== source.bytes.length
    ) {
      fail(`aggregate retained metadata drifted for ${retained.task}`);
    }
    exactArray(retained.scenario_ids, source.entry.scenarios, `${retained.task} retained scenarios`);
  }

  for (const scenario of aggregate.scenario_results) {
    const source = sourceByTask.get(scenario.source_task);
    const retainedScenario = source?.scenarios.get(scenario.source_scenario_id);
    if (!retainedScenario || JSON.stringify(scenario.facts) !== JSON.stringify(retainedScenario.facts)) {
      fail(`aggregate scenario ${scenario.id} facts differ from retained source evidence`);
    }
  }
  validateGroupedEvidence(aggregate, sourceByTask);

  if (!Array.isArray(aggregate.supporting_scenario_results) || aggregate.supporting_scenario_results.length === 0) {
    fail("aggregate supporting_scenario_results must retain D2 supporting facts");
  }
  validateGroupedEvidence(
    { ...aggregate, ...Object.fromEntries(groupedFields.map((field) => [field, aggregate[field]])) },
    sourceByTask,
  );
  return sha256(aggregateBytes);
}

if (process.argv.length !== 2) {
  fail("this reviewer accepts no command-line arguments");
}

const reviewer = boundedEnv(reviewerEnv, 3, 128);
const retentionReference = boundedEnv(retentionRefEnv, 8, 2048);
const retainedSha = boundedEnv(retainedShaEnv, 64, 64);
if (!/^[0-9a-f]{64}$/.test(retainedSha)) {
  fail(`${retainedShaEnv} must be one lowercase SHA-256 digest`);
}

const head = currentHead();
const contractBytes = readBytes(contractPath);
const contract = parseJson(contractPath, contractBytes);
if (
  contract.contract !== "forum_search_versioned_invalidation_retained_evidence_promotion_v1" ||
  contract.task !== "FORUM-23B2G2B3D12" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) {
  fail("D12 machine contract identity or status drifted");
}

const parentBytes = readBytes(parentPath);
const parent = parseJson(parentPath, parentBytes);
validateParent(parent, aggregatePath);

const sources = manifest.map((entry) => validateSourceArtifact(entry, head));
const aggregateBytes = readBytes(aggregatePath);
const aggregate = parseJson(aggregatePath, aggregateBytes);
const aggregateDigest = validateAggregate(
  aggregate,
  aggregateBytes,
  parentBytes,
  head,
  sources,
);
if (retainedSha !== aggregateDigest) {
  fail(`${retainedShaEnv} does not equal the exact aggregate artifact digest`);
}

const candidate = {
  contract: "forum_search_versioned_invalidation_runtime_promotion_candidate_v1",
  task: "FORUM-23B2G2B3D12",
  status: "approved_for_canonical_status_promotion",
  source_commit: head,
  reviewed_at: new Date().toISOString(),
  reviewer,
  retention: {
    reference: retentionReference,
    attested_sha256: retainedSha,
    matches_reviewed_aggregate: true,
    external_service_authentication_performed_by_script: false,
  },
  aggregate_artifact: {
    path: aggregatePath,
    sha256: aggregateDigest,
    byte_length: aggregateBytes.length,
    generated_at: aggregate.generated_at,
    scenario_ids: frozenScenarios,
  },
  parent_contract: {
    path: parentPath,
    sha256: sha256(parentBytes),
    status_at_review: parent.status,
  },
  source_artifacts: sources.map(({ entry, artifact, digest, bytes }) => ({
    task: entry.task,
    contract: entry.contract,
    path: entry.path,
    source_commit: artifact.source_commit,
    generated_at: artifact.generated_at,
    sha256: digest,
    byte_length: bytes.length,
    scenario_ids: entry.scenarios,
  })),
  validation: {
    all_ten_frozen_scenarios_passed: true,
    all_nine_source_artifacts_revalidated: true,
    all_source_digests_match_aggregate: true,
    aggregate_parent_digest_matches_current_d0: true,
    aggregate_source_commit_matches_current_head: true,
    retained_digest_attested_by_maintainer: true,
  },
  proposed_transition: {
    from: "source_ready_maintainer_execution_pending",
    to: "runtime_evidence_reviewed",
    separate_canonical_source_pull_request_required: true,
    canonical_source_mutated_by_reviewer: false,
    closes_forum_23: false,
    closes_link_forum_03: false,
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

console.log(`wrote reviewed Forum Search promotion candidate to ${candidatePath}`);
