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
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-assembler.json";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const d13ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-evidence-assembler.json";
const d14ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-translation-moderation-proof.json";
const d15ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-private-trusted-exclusion-proof.json";
const d16ContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-topic-move-proof.json";
const d13ArtifactPath =
  "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json";
const d14ArtifactPath =
  "target/forum-search-link-forum-03-translation-moderation-evidence.json";
const d15ArtifactPath =
  "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json";
const d16ArtifactPath =
  "target/forum-search-link-forum-03-topic-move-evidence.json";
const outputPath =
  "target/link-forum-03-forum-index-search-complete-evidence.json";

const d13ScenarioOrder = [
  "deletion_acl_ordering",
  "search_disabled_profile",
  "normal_delivery",
];
const coreScenarioIds = [
  "normal_delivery",
  "deletion_acl_ordering",
  "search_disabled_profile",
];
const completeScenarioIds = [
  ...coreScenarioIds,
  "translation_and_moderation_approval",
  "private_and_trusted_channel_exclusion",
  "topic_move_category_scope",
];
const extensionSpecs = [
  {
    task: "FORUM-23B2G2B3D14",
    contract: "forum_search_link_forum_03_translation_moderation_evidence_v1",
    path: d14ArtifactPath,
    scenario: "translation_and_moderation_approval",
  },
  {
    task: "FORUM-23B2G2B3D15",
    contract: "forum_search_link_forum_03_private_trusted_exclusion_proof_v1",
    path: d15ArtifactPath,
    scenario: "private_and_trusted_channel_exclusion",
  },
  {
    task: "FORUM-23B2G2B3D16",
    contract: "forum_search_link_forum_03_topic_move_evidence_v1",
    path: d16ArtifactPath,
    scenario: "topic_move_category_scope",
  },
];

function fail(message) {
  throw new Error(`LINK-FORUM-03 complete evidence assembly failed: ${message}`);
}

function readBytes(path) {
  try {
    return readFileSync(resolve(root, path));
  } catch (error) {
    fail(`required artifact ${path} is unavailable: ${error.message}`);
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

function requireDigest(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    fail(`${label} must be a lowercase SHA-256 digest`);
  }
}

function requireIsoDate(value, label) {
  requireString(value, label);
  if (!Number.isFinite(Date.parse(value))) {
    fail(`${label} must be an ISO timestamp`);
  }
}

function requireExactArray(actual, expected, label) {
  if (!Array.isArray(actual) || JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} drifted: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function requireTrueFields(object, fields, label) {
  requireObject(object, label);
  for (const field of fields) {
    if (object[field] !== true) {
      fail(`${label}.${field} must be true`);
    }
  }
}

function currentHead() {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  }).trim();
  if (!/^[0-9a-f]{40}$/.test(value)) {
    fail("git rev-parse HEAD returned an invalid commit SHA");
  }
  return value;
}

function validatePlan(plan) {
  for (const marker of [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "| `FORUM-23` | `in_progress` |",
    "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |",
  ]) {
    if (!plan.includes(marker)) {
      fail(`canonical plan is missing marker: ${marker}`);
    }
  }
  for (const forbidden of [
    "| `FORUM-23` | `done` |",
    "| `LINK-FORUM-03` | `done` |",
    "FORUM-23B2G2B3D17 closes LINK-FORUM-03",
  ]) {
    if (plan.includes(forbidden)) {
      fail(`canonical plan contains forbidden promotion marker: ${forbidden}`);
    }
  }
}

function validateMachineContract(contract) {
  requireObject(contract, "D17 contract");
  if (
    contract.contract !== "forum_search_link_forum_03_complete_evidence_assembler_v1" ||
    contract.task !== "FORUM-23B2G2B3D17" ||
    contract.target_link !== "LINK-FORUM-03" ||
    contract.coverage !== "complete_canonical_runtime_scope_review_pending" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.assembler !==
      "scripts/evidence/assemble-link-forum-03-complete-forum-search-evidence.mjs" ||
    contract.verifier !==
      "scripts/verify/verify-link-forum-03-complete-forum-search-evidence.mjs"
  ) {
    fail("D17 machine-contract identity drifted");
  }
  requireExactArray(contract.required_inputs, [
    d13ArtifactPath,
    d14ArtifactPath,
    d15ArtifactPath,
    d16ArtifactPath,
  ], "D17 required inputs");
  requireExactArray(contract.required_scenarios, completeScenarioIds, "D17 scenarios");
  if (
    contract.output_artifact?.path !== outputPath ||
    contract.output_artifact?.status !==
      "complete_runtime_evidence_assembled_review_pending" ||
    contract.output_artifact?.hand_editing_forbidden !== true ||
    contract.output_artifact?.source_commit_required !== true ||
    contract.output_artifact?.same_commit_inputs_required !== true ||
    contract.output_artifact?.atomic_replace !== true ||
    contract.output_artifact?.automatic_canonical_source_mutation !== false
  ) {
    fail("D17 output boundary drifted");
  }
}

function validateSourceContracts() {
  const d13 = parseJson(d13ContractPath);
  if (
    d13.contract !== "forum_search_link_forum_03_evidence_assembler_v1" ||
    d13.task !== "FORUM-23B2G2B3D13" ||
    d13.status !== "source_ready_maintainer_execution_pending" ||
    d13.output_artifact?.path !== d13ArtifactPath
  ) {
    fail("D13 source contract drifted");
  }
  const specs = [
    [
      d14ContractPath,
      "forum_search_link_forum_03_translation_moderation_proof_v1",
      "FORUM-23B2G2B3D14",
      d14ArtifactPath,
    ],
    [
      d15ContractPath,
      "forum_search_link_forum_03_private_trusted_exclusion_proof_v1",
      "FORUM-23B2G2B3D15",
      d15ArtifactPath,
    ],
    [
      d16ContractPath,
      "forum_search_link_forum_03_topic_move_proof_v1",
      "FORUM-23B2G2B3D16",
      d16ArtifactPath,
    ],
  ];
  for (const [path, contractName, task, artifactPath] of specs) {
    const contract = parseJson(path);
    if (
      contract.contract !== contractName ||
      contract.task !== task ||
      contract.target_link !== "LINK-FORUM-03" ||
      contract.status !== "source_ready_maintainer_execution_pending" ||
      contract.evidence_artifact !== artifactPath
    ) {
      fail(`${task} source contract drifted`);
    }
  }
}

function validateD13(bytes, artifact, head) {
  requireObject(artifact, "D13 artifact");
  if (
    artifact.contract !==
      "link_forum_03_forum_index_search_ordering_visibility_evidence_v1" ||
    artifact.task !== "LINK-FORUM-03" ||
    artifact.source_slice !== "FORUM-23B2G2B3D13" ||
    artifact.status !== "partial_runtime_evidence_assembled" ||
    artifact.coverage !== "ordering_visibility_and_search_disabled_core_only" ||
    artifact.source_commit !== head
  ) {
    fail("D13 partial artifact identity or source commit drifted");
  }
  requireIsoDate(artifact.generated_at, "D13 generated_at");
  requireObject(artifact.selected_scenario_evidence, "D13 selected scenarios");
  requireExactArray(
    Object.keys(artifact.selected_scenario_evidence),
    d13ScenarioOrder,
    "D13 selected scenario order",
  );
  for (const scenarioId of d13ScenarioOrder) {
    const scenario = artifact.selected_scenario_evidence[scenarioId];
    requireObject(scenario, `D13 ${scenarioId}`);
    requireString(scenario.source_task, `D13 ${scenarioId}.source_task`);
    requireString(scenario.source_contract, `D13 ${scenarioId}.source_contract`);
    requireString(scenario.source_artifact, `D13 ${scenarioId}.source_artifact`);
    requireDigest(scenario.source_sha256, `D13 ${scenarioId}.source_sha256`);
    requireObject(scenario.facts, `D13 ${scenarioId}.facts`);
    if (Object.keys(scenario.facts).length === 0) {
      fail(`D13 ${scenarioId} facts are empty`);
    }
  }
  requireObject(artifact.retained_lineage, "D13 retained lineage");
  for (const field of [
    "canonical_plan_sha256",
    "d0_parent_sha256",
    "aggregate_sha256",
    "promotion_candidate_sha256",
    "retained_aggregate_sha256",
  ]) {
    requireDigest(artifact.retained_lineage[field], `D13 retained_lineage.${field}`);
  }
  requireString(artifact.retained_lineage.reviewer, "D13 reviewer");
  requireString(
    artifact.retained_lineage.retention_reference,
    "D13 retention reference",
  );
  if (
    artifact.retained_lineage.external_retention_authentication_performed_by_assembler !==
    false
  ) {
    fail("D13 external-retention boundary drifted");
  }
  requireTrueFields(
    artifact.assertions,
    [
      "real_forum_owner_to_iggy_to_search_to_storefront_trace_passed",
      "projection_completed_before_delivery_covered_checkpoint",
      "out_of_order_and_duplicate_delivery_did_not_restore_denied_content",
      "stale_denied_rows_were_reauthorized_before_items_totals_and_facets",
      "forum_owner_writes_survived_search_disabled_profile",
      "late_search_recovery_rebuilt_from_owner_revision_ledger",
      "selected_proofs_share_one_reviewed_source_commit",
    ],
    "D13 assertions",
  );
  if (
    artifact.assertions.canonical_source_mutated_by_assembler !== false ||
    artifact.canonical_transition?.status_change_allowed_from_this_artifact !== false ||
    artifact.canonical_transition?.closes_forum_23_automatically !== false ||
    artifact.canonical_transition?.closes_link_forum_03_automatically !== false
  ) {
    fail("D13 canonical transition boundary drifted");
  }
  return {
    path: d13ArtifactPath,
    sha256: sha256(bytes),
    byte_length: bytes.length,
    generated_at: artifact.generated_at,
    source_commit: artifact.source_commit,
    scenario_ids: d13ScenarioOrder,
    retained_lineage: artifact.retained_lineage,
    scenario_evidence: artifact.selected_scenario_evidence,
  };
}

function validateExtension(spec, bytes, artifact, head) {
  requireObject(artifact, `${spec.task} artifact`);
  if (
    artifact.contract !== spec.contract ||
    artifact.task !== spec.task ||
    artifact.source_commit !== head ||
    artifact.database_backend !== "postgresql" ||
    artifact.broker_used !== false
  ) {
    fail(`${spec.task} artifact identity or runtime profile drifted`);
  }
  requireIsoDate(artifact.generated_at, `${spec.task} generated_at`);
  if (!Array.isArray(artifact.scenario_results) || artifact.scenario_results.length !== 1) {
    fail(`${spec.task} must contain exactly one scenario result`);
  }
  const scenario = artifact.scenario_results[0];
  requireObject(scenario, `${spec.task} scenario`);
  if (scenario.id !== spec.scenario || scenario.result !== "passed") {
    fail(`${spec.task} scenario identity or result drifted`);
  }
  requireObject(scenario.facts, `${spec.task} facts`);
  if (Object.keys(scenario.facts).length === 0) {
    fail(`${spec.task} facts are empty`);
  }

  if (spec.task === "FORUM-23B2G2B3D14") {
    for (const field of [
      "english_topic_remained_visible",
      "french_topic_became_visible",
      "approved_reply_visible_after_approval",
    ]) {
      if (scenario.facts[field] !== true) {
        fail(`D14 facts.${field} must be true`);
      }
    }
    if (
      scenario.facts.pending_reply_visible_before_approval !== false ||
      scenario.facts.owner_revision_compared_to_ingest_sequence !== false ||
      scenario.facts.caught_up_repeat_performed_work !== false
    ) {
      fail("D14 fail-closed facts drifted");
    }
  }

  if (spec.task === "FORUM-23B2G2B3D15") {
    if (
      scenario.facts.legitimate_private_topic_documents !== 0 ||
      scenario.facts.legitimate_trusted_topic_documents !== 0 ||
      scenario.facts.stale_search_rows_injected !== 2 ||
      scenario.facts.owner_revision_compared_to_ingest_sequence !== false ||
      scenario.facts.caught_up_repeat_performed_work !== false
    ) {
      fail("D15 exclusion facts drifted");
    }
    if (!Array.isArray(scenario.facts.storefront_matrix)) {
      fail("D15 storefront matrix is missing");
    }
    const labels = scenario.facts.storefront_matrix.map(({ label }) => label);
    requireExactArray(
      labels,
      [
        "public_control",
        "public_private_denied",
        "public_trusted_denied",
        "private_explicit_user_allowed",
        "private_outsider_denied",
        "trusted_low_trust_denied",
        "trusted_nonmember_denied",
        "trusted_wrong_route_denied",
        "trusted_exact_member_allowed",
      ],
      "D15 storefront matrix",
    );
  }

  if (spec.task === "FORUM-23B2G2B3D16") {
    for (const field of [
      "topic_identity_retained",
      "reply_identity_retained",
      "source_category_scope_empty_after_move",
      "target_category_scope_contains_topic_and_reply_after_move",
    ]) {
      if (scenario.facts[field] !== true) {
        fail(`D16 facts.${field} must be true`);
      }
    }
    for (const field of [
      "exact_replay_created_new_owner_revision",
      "exact_replay_created_new_transport_event",
      "exact_replay_created_new_inbox_row",
      "owner_revision_compared_to_ingest_sequence",
      "caught_up_repeat_performed_work",
    ]) {
      if (scenario.facts[field] !== false) {
        fail(`D16 facts.${field} must be false`);
      }
    }
  }

  return {
    path: spec.path,
    task: spec.task,
    contract: spec.contract,
    sha256: sha256(bytes),
    byte_length: bytes.length,
    generated_at: artifact.generated_at,
    source_commit: artifact.source_commit,
    database_backend: artifact.database_backend,
    broker_used: artifact.broker_used,
    scenario_id: scenario.id,
    facts: scenario.facts,
  };
}

if (process.argv.length !== 2) {
  fail("this assembler accepts no command-line arguments");
}

const head = currentHead();
validateMachineContract(parseJson(contractPath));
const planBytes = readBytes(planPath);
validatePlan(planBytes.toString("utf8"));
validateSourceContracts();

const d13Bytes = readBytes(d13ArtifactPath);
const d13 = validateD13(d13Bytes, parseJson(d13ArtifactPath, d13Bytes), head);
const extensions = extensionSpecs.map((spec) => {
  const bytes = readBytes(spec.path);
  return validateExtension(spec, bytes, parseJson(spec.path, bytes), head);
});
const extensionByScenario = Object.fromEntries(
  extensions.map((extension) => [extension.scenario_id, extension]),
);
const scenarioEvidence = Object.fromEntries(
  completeScenarioIds.map((scenarioId) => {
    const core = d13.scenario_evidence[scenarioId];
    if (core !== undefined) {
      return [scenarioId, core];
    }
    const extension = extensionByScenario[scenarioId];
    if (extension === undefined) {
      fail(`assembled scenario ${scenarioId} has no validated source`);
    }
    return [
      scenarioId,
      {
        source_task: extension.task,
        source_contract: extension.contract,
        source_artifact: extension.path,
        source_sha256: extension.sha256,
        facts: extension.facts,
      },
    ];
  }),
);
requireExactArray(
  Object.keys(scenarioEvidence),
  completeScenarioIds,
  "assembled scenario order",
);

const output = {
  contract: "link_forum_03_forum_index_search_complete_evidence_v1",
  task: "LINK-FORUM-03",
  source_slice: "FORUM-23B2G2B3D17",
  status: "complete_runtime_evidence_assembled_review_pending",
  coverage: "canonical_link_forum_03_runtime_scope",
  source_commit: head,
  generated_at: new Date().toISOString(),
  assembled_scenario_ids: completeScenarioIds,
  runtime_profiles: {
    reviewed_core: {
      database_backend: "postgresql",
      delivery_profile: "outbox_iggy",
      consumer_group: "rustok-search-forum-projection-v1",
      topic: "domain",
    },
    extension_proofs: {
      database_backend: "postgresql",
      broker_used: false,
      ingress: "ForumSearchContractIngress",
      projector: "ForumProjectionReconciler",
      storefront: "execute_forum_storefront_search",
    },
  },
  scenario_evidence: scenarioEvidence,
  source_artifacts: [
    {
      path: d13.path,
      sha256: d13.sha256,
      byte_length: d13.byte_length,
      generated_at: d13.generated_at,
      source_commit: d13.source_commit,
      scenario_ids: d13.scenario_ids,
    },
    ...extensions.map(({ facts, ...extension }) => extension),
  ],
  reviewed_core_lineage: {
    source_artifact: d13.path,
    source_sha256: d13.sha256,
    inherited_retained_lineage: d13.retained_lineage,
    external_retention_authentication_performed_by_d17: false,
  },
  assertions: {
    reviewed_normal_delivery_ordering_visibility_and_recovery_core_retained: true,
    translation_projection_and_retrieval_passed: true,
    moderation_approval_visibility_transition_passed: true,
    private_and_trusted_channel_exclusion_passed: true,
    topic_move_category_scope_transition_passed: true,
    all_runtime_inputs_share_current_source_commit: true,
    canonical_link_runtime_scope_assembled: true,
    complete_artifact_independently_reviewed: false,
    complete_artifact_retention_attested: false,
    canonical_source_mutated_by_assembler: false,
  },
  remaining_after_assembly: [
    "independent maintainer review and immutable retention attestation for this complete artifact",
    "a separate canonical-source pull request after review",
    "independent FORUM-21 owner-task promotion remains outside this LINK artifact",
  ],
  canonical_transition: {
    link_status_before_review: "planned",
    status_change_allowed_from_this_artifact: false,
    reason: "complete runtime coverage is assembled but not independently reviewed or retained",
    separate_review_gate_required: true,
    separate_canonical_source_pull_request_required: true,
    closes_forum_21_automatically: false,
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
  fail(`atomic complete LINK-FORUM-03 output write failed: ${error.message}`);
}

console.log(`wrote validated complete review-pending LINK-FORUM-03 evidence to ${outputPath}`);
