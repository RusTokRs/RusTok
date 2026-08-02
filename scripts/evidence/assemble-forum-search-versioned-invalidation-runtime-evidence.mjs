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
const parentContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const outputPath =
  "target/forum-search-versioned-invalidation-runtime-evidence.json";
const canonicalConsumerGroup = "rustok-search-forum-projection-v1";
const canonicalTopic = "domain";
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
    iggy: false,
  },
  {
    task: "FORUM-23B2G2B3D3",
    contract: "forum_search_versioned_invalidation_ack_restart_evidence_v1",
    path: "target/forum-search-versioned-invalidation-ack-restart-evidence.json",
    scenarios: ["acknowledgement_failure_restart"],
    iggy: true,
  },
  {
    task: "FORUM-23B2G2B3D4",
    contract: "forum_search_versioned_invalidation_raw_poison_evidence_v1",
    path: "target/forum-search-versioned-invalidation-raw-poison-evidence.json",
    scenarios: ["raw_poison_dlq_redelivery"],
    iggy: true,
  },
  {
    task: "FORUM-23B2G2B3D5",
    contract: "forum_search_versioned_invalidation_semantic_poison_evidence_v1",
    path: "target/forum-search-versioned-invalidation-semantic-poison-evidence.json",
    scenarios: ["semantic_poison_identity_conflict"],
    iggy: true,
  },
  {
    task: "FORUM-23B2G2B3D6",
    contract: "forum_search_versioned_invalidation_missing_delivery_repair_evidence_v1",
    path: "target/forum-search-versioned-invalidation-missing-delivery-repair-evidence.json",
    scenarios: ["missing_delivery_owner_repair"],
    iggy: false,
  },
  {
    task: "FORUM-23B2G2B3D7",
    contract: "forum_search_versioned_invalidation_multi_process_evidence_v1",
    path: "target/forum-search-versioned-invalidation-multi-process-evidence.json",
    scenarios: ["multi_process_serialization"],
    iggy: false,
  },
  {
    task: "FORUM-23B2G2B3D8",
    contract: "forum_search_versioned_invalidation_deletion_acl_ordering_evidence_v1",
    path: "target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json",
    scenarios: ["deletion_acl_ordering"],
    iggy: false,
  },
  {
    task: "FORUM-23B2G2B3D9",
    contract: "forum_search_versioned_invalidation_search_disabled_recovery_evidence_v1",
    path: "target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json",
    scenarios: ["search_disabled_profile"],
    iggy: false,
  },
  {
    task: "FORUM-23B2G2B3D10",
    contract: "forum_search_versioned_invalidation_normal_delivery_evidence_v1",
    path: "target/forum-search-versioned-invalidation-normal-delivery-evidence.json",
    scenarios: ["normal_delivery"],
    iggy: true,
  },
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
  ["multi_process_serialization", ["FORUM-23B2G2B3D7", "multi_process_serialization"]],
  ["deletion_acl_ordering", ["FORUM-23B2G2B3D8", "deletion_acl_ordering"]],
  ["search_disabled_profile", ["FORUM-23B2G2B3D9", "search_disabled_profile"]],
]);

function fail(message) {
  throw new Error(`Forum Search aggregate evidence assembly failed: ${message}`);
}

function readBytes(path) {
  try {
    return readFileSync(resolve(root, path));
  } catch (error) {
    fail(`required artifact ${path} is unavailable: ${error.message}`);
  }
}

function parseJson(path, bytes) {
  try {
    return JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    fail(`required artifact ${path} is not valid JSON: ${error.message}`);
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

function requireNonEmptyFacts(value, label) {
  requireObject(value, label);
  if (Object.keys(value).length === 0) {
    fail(`${label} must not be empty`);
  }
}

function requireSourceCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    fail(`${label} must be one lowercase forty-character Git commit SHA`);
  }
}

function requireGeneratedAt(value, label) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) {
    fail(`${label} must be an ISO timestamp`);
  }
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
  requireSourceCommit(value, "git rev-parse HEAD");
  return value;
}

function validateParentContract(parent) {
  requireObject(parent, "D0 parent contract");
  if (parent.contract !== "forum_search_versioned_invalidation_runtime_evidence_v1") {
    fail("D0 parent contract identity drifted");
  }
  if (parent.task !== "FORUM-23B2G2B3D0") {
    fail("D0 parent task identity drifted");
  }
  if (parent.status !== "source_ready_maintainer_execution_pending") {
    fail("D0 parent must remain source_ready_maintainer_execution_pending before assembly");
  }
  const parentScenarios = parent.required_scenarios?.map(({ id }) => id);
  if (JSON.stringify(parentScenarios) !== JSON.stringify(frozenScenarioIds)) {
    fail("D0 frozen required scenario order or membership drifted");
  }
  if (parent.evidence_artifact?.path !== outputPath) {
    fail("D0 aggregate evidence output path drifted");
  }
  const requiredFields = new Set(parent.evidence_artifact?.required_fields ?? []);
  for (const field of [
    "contract",
    "source_commit",
    "database_backend",
    "delivery_profile",
    "consumer_group",
    "scenario_results",
    "owner_revision_rows",
    "typed_and_root_event_ids",
    "search_inbox_rows",
    "ingest_sequences",
    "owner_checkpoints",
    "poison_receipts",
    "dlq_receipts",
    "storefront_visibility_assertions",
  ]) {
    if (!requiredFields.has(field)) {
      fail(`D0 aggregate required field ${field} is missing`);
    }
  }
  const registered = new Map(
    (parent.source_ready_subproofs ?? []).map((entry) => [entry.task, entry]),
  );
  for (const entry of manifest) {
    const registration = registered.get(entry.task);
    if (!registration || registration.evidence_artifact !== entry.path) {
      fail(`${entry.task} is not registered with its exact evidence artifact in D0`);
    }
  }
}

function validateArtifact(entry, bytes, artifact, head) {
  requireObject(artifact, entry.path);
  if (artifact.contract !== entry.contract) {
    fail(`${entry.path} contract must be ${entry.contract}`);
  }
  if (artifact.task !== entry.task) {
    fail(`${entry.path} task must be ${entry.task}`);
  }
  requireSourceCommit(artifact.source_commit, `${entry.path}.source_commit`);
  if (artifact.source_commit !== head) {
    fail(`${entry.path} was generated from ${artifact.source_commit}, not current HEAD ${head}`);
  }
  requireGeneratedAt(artifact.generated_at, `${entry.path}.generated_at`);
  if (artifact.database_backend !== "postgresql") {
    fail(`${entry.path} must report database_backend postgresql`);
  }
  if (entry.iggy) {
    if (artifact.delivery_profile !== "outbox_iggy") {
      fail(`${entry.path} must report delivery_profile outbox_iggy`);
    }
    if (artifact.consumer_group !== canonicalConsumerGroup) {
      fail(`${entry.path} must report consumer group ${canonicalConsumerGroup}`);
    }
    if (artifact.topic !== canonicalTopic) {
      fail(`${entry.path} must report topic ${canonicalTopic}`);
    }
    if (typeof artifact.stream !== "string" || artifact.stream.trim().length === 0) {
      fail(`${entry.path} must report a non-empty external Iggy stream`);
    }
  }
  if (!Array.isArray(artifact.scenario_results)) {
    fail(`${entry.path}.scenario_results must be an array`);
  }
  const actualIds = artifact.scenario_results.map(({ id }) => id);
  if (JSON.stringify(actualIds) !== JSON.stringify(entry.scenarios)) {
    fail(`${entry.path} scenario order or membership drifted`);
  }
  const scenarioMap = new Map();
  for (const scenario of artifact.scenario_results) {
    requireObject(scenario, `${entry.path} scenario`);
    if (scenarioMap.has(scenario.id)) {
      fail(`${entry.path} repeats scenario ${scenario.id}`);
    }
    if (scenario.result !== "passed") {
      fail(`${entry.path} scenario ${scenario.id} did not pass`);
    }
    requireNonEmptyFacts(scenario.facts, `${entry.path} scenario ${scenario.id} facts`);
    scenarioMap.set(scenario.id, scenario);
  }
  return {
    entry,
    artifact,
    scenarioMap,
    digest: sha256(bytes),
    byteLength: bytes.length,
  };
}

function evidenceBundle(validatedByTask, sources) {
  return sources.map(([task, scenarioId]) => {
    const validated = validatedByTask.get(task);
    const scenario = validated?.scenarioMap.get(scenarioId);
    if (!validated || !scenario) {
      fail(`aggregate evidence bundle cannot resolve ${task}:${scenarioId}`);
    }
    return {
      source_task: task,
      source_contract: validated.entry.contract,
      source_artifact: validated.entry.path,
      source_scenario_id: scenarioId,
      facts: scenario.facts,
    };
  });
}

if (process.argv.length !== 2) {
  fail("this assembler accepts no command-line arguments");
}

const head = currentHead();
const parentBytes = readBytes(parentContractPath);
const parent = parseJson(parentContractPath, parentBytes);
validateParentContract(parent);

const validated = manifest.map((entry) => {
  const bytes = readBytes(entry.path);
  return validateArtifact(entry, bytes, parseJson(entry.path, bytes), head);
});
const validatedByTask = new Map(validated.map((item) => [item.entry.task, item]));
if (validatedByTask.size !== manifest.length) {
  fail("aggregate manifest contains duplicate task identities");
}

const scenarioResults = frozenScenarioIds.map((id) => {
  const source = frozenScenarioSources.get(id);
  if (!source) {
    fail(`frozen scenario ${id} has no registered source`);
  }
  const [task, sourceScenarioId] = source;
  const validatedSource = validatedByTask.get(task);
  const scenario = validatedSource?.scenarioMap.get(sourceScenarioId);
  if (!validatedSource || !scenario) {
    fail(`frozen scenario ${id} cannot resolve ${task}:${sourceScenarioId}`);
  }
  return {
    id,
    result: "passed",
    source_task: task,
    source_contract: validatedSource.entry.contract,
    source_artifact: validatedSource.entry.path,
    source_scenario_id: sourceScenarioId,
    facts: scenario.facts,
  };
});

const sourceArtifacts = validated.map(({ entry, artifact, digest, byteLength }) => ({
  task: entry.task,
  contract: entry.contract,
  path: entry.path,
  source_commit: artifact.source_commit,
  generated_at: artifact.generated_at,
  sha256: digest,
  byte_length: byteLength,
  scenario_ids: entry.scenarios,
}));

const aggregate = {
  contract: "forum_search_versioned_invalidation_runtime_evidence_v1",
  task: "FORUM-23B2G2B3D0",
  status: "runtime_evidence_assembled",
  source_commit: head,
  generated_at: new Date().toISOString(),
  database_backend: "postgresql",
  delivery_profile: "outbox_iggy",
  consumer_group: canonicalConsumerGroup,
  topic: canonicalTopic,
  scenario_results: scenarioResults,
  owner_revision_rows: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D6", "missing_delivery_owner_repair"],
    ["FORUM-23B2G2B3D7", "multi_process_serialization"],
    ["FORUM-23B2G2B3D8", "deletion_acl_ordering"],
    ["FORUM-23B2G2B3D9", "search_disabled_profile"],
    ["FORUM-23B2G2B3D10", "normal_delivery"],
  ]),
  typed_and_root_event_ids: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D2", "typed_ingress_admission"],
    ["FORUM-23B2G2B3D3", "acknowledgement_failure_restart"],
    ["FORUM-23B2G2B3D5", "semantic_poison_identity_conflict"],
    ["FORUM-23B2G2B3D8", "deletion_acl_ordering"],
    ["FORUM-23B2G2B3D9", "search_disabled_profile"],
    ["FORUM-23B2G2B3D10", "normal_delivery"],
  ]),
  search_inbox_rows: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D2", "typed_ingress_admission"],
    ["FORUM-23B2G2B3D2", "legacy_first_duplicate"],
    ["FORUM-23B2G2B3D2", "typed_first_duplicate"],
    ["FORUM-23B2G2B3D3", "acknowledgement_failure_restart"],
    ["FORUM-23B2G2B3D5", "semantic_poison_identity_conflict"],
    ["FORUM-23B2G2B3D8", "deletion_acl_ordering"],
    ["FORUM-23B2G2B3D10", "normal_delivery"],
  ]),
  ingest_sequences: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D2", "typed_ingress_admission"],
    ["FORUM-23B2G2B3D2", "legacy_first_duplicate"],
    ["FORUM-23B2G2B3D2", "typed_first_duplicate"],
    ["FORUM-23B2G2B3D3", "acknowledgement_failure_restart"],
    ["FORUM-23B2G2B3D8", "deletion_acl_ordering"],
    ["FORUM-23B2G2B3D10", "normal_delivery"],
  ]),
  owner_checkpoints: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D6", "missing_delivery_owner_repair"],
    ["FORUM-23B2G2B3D7", "multi_process_serialization"],
    ["FORUM-23B2G2B3D9", "search_disabled_profile"],
    ["FORUM-23B2G2B3D10", "normal_delivery"],
  ]),
  poison_receipts: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D4", "raw_poison_dlq_redelivery"],
    ["FORUM-23B2G2B3D5", "semantic_poison_identity_conflict"],
  ]),
  dlq_receipts: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D4", "raw_poison_dlq_redelivery"],
    ["FORUM-23B2G2B3D5", "semantic_poison_identity_conflict"],
  ]),
  storefront_visibility_assertions: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D8", "deletion_acl_ordering"],
    ["FORUM-23B2G2B3D10", "normal_delivery"],
  ]),
  supporting_scenario_results: evidenceBundle(validatedByTask, [
    ["FORUM-23B2G2B3D2", "typed_ingress_admission"],
    ["FORUM-23B2G2B3D2", "semantic_identity_conflict"],
  ]),
  source_artifacts: sourceArtifacts,
  assembly: {
    assembler:
      "scripts/evidence/assemble-forum-search-versioned-invalidation-runtime-evidence.mjs",
    parent_contract: parentContractPath,
    parent_contract_sha256: sha256(parentBytes),
    input_artifact_count: sourceArtifacts.length,
    frozen_scenario_count: scenarioResults.length,
    all_inputs_same_source_commit: true,
    source_commit_matches_current_head: true,
    output_written_after_complete_validation: true,
  },
};

for (const field of parent.evidence_artifact.required_fields) {
  if (!(field in aggregate)) {
    fail(`assembled output is missing D0 required field ${field}`);
  }
}

const absoluteOutput = resolve(root, outputPath);
mkdirSync(dirname(absoluteOutput), { recursive: true });
const temporaryOutput = `${absoluteOutput}.${process.pid}.${Date.now()}.tmp`;
try {
  writeFileSync(temporaryOutput, `${JSON.stringify(aggregate, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  renameSync(temporaryOutput, absoluteOutput);
} catch (error) {
  rmSync(temporaryOutput, { force: true });
  fail(`atomic aggregate output write failed: ${error.message}`);
}

console.log(`wrote validated Forum Search aggregate runtime evidence to ${outputPath}`);
