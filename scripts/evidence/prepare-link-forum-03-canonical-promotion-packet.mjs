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
  "crates/rustok-forum/contracts/forum-search-link-forum-03-canonical-promotion-packet.json";
const reviewContractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-promotion.json";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const candidatePath =
  "target/link-forum-03-forum-index-search-complete-promotion-candidate.json";
const completePath =
  "target/link-forum-03-forum-index-search-complete-evidence.json";
const outputPath = "target/link-forum-03-canonical-promotion-packet.json";

const plannedRow =
  "| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |";
const doneRow =
  "| `LINK-FORUM-03` | `done` | D13-D18 provide reviewed and retained Forum/index/search ordering, recovery, multilingual, moderation, private/trusted exclusion and topic-move evidence. |";
const forum21Row =
  "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |";
const forum23Marker = "| `FORUM-23` | `in_progress` |";

const scenarioIds = [
  "normal_delivery",
  "deletion_acl_ordering",
  "search_disabled_profile",
  "translation_and_moderation_approval",
  "private_and_trusted_channel_exclusion",
  "topic_move_category_scope",
];

const sourceSpecs = [
  {
    path: "target/link-forum-03-forum-index-search-ordering-visibility-evidence.json",
    task: "LINK-FORUM-03",
    contract: "link_forum_03_forum_index_search_ordering_visibility_evidence_v1",
    scenarios: scenarioIds.slice(0, 3),
  },
  {
    path: "target/forum-search-link-forum-03-translation-moderation-evidence.json",
    task: "FORUM-23B2G2B3D14",
    contract: "forum_search_link_forum_03_translation_moderation_evidence_v1",
    scenarios: ["translation_and_moderation_approval"],
  },
  {
    path: "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json",
    task: "FORUM-23B2G2B3D15",
    contract: "forum_search_link_forum_03_private_trusted_exclusion_proof_v1",
    scenarios: ["private_and_trusted_channel_exclusion"],
  },
  {
    path: "target/forum-search-link-forum-03-topic-move-evidence.json",
    task: "FORUM-23B2G2B3D16",
    contract: "forum_search_link_forum_03_topic_move_evidence_v1",
    scenarios: ["topic_move_category_scope"],
  },
];

function fail(message) {
  throw new Error(`LINK-FORUM-03 canonical promotion packet failed: ${message}`);
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

function requireTimestamp(value, label) {
  requireString(value, label);
  if (!Number.isFinite(Date.parse(value))) {
    fail(`${label} must be an ISO timestamp`);
  }
}

function requireDigest(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
    fail(`${label} must be a lowercase SHA-256 digest`);
  }
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    fail(`${label} must be a lowercase forty-character Git commit SHA`);
  }
}

function exactArray(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} order or membership drifted`);
  }
}

function exactJson(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} drifted`);
  }
}

function countOccurrences(text, marker) {
  return text.split(marker).length - 1;
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

function validatePacketContract(contract) {
  requireObject(contract, "D19 contract");
  if (
    contract.contract !==
      "forum_search_link_forum_03_canonical_promotion_packet_v1" ||
    contract.task !== "FORUM-23B2G2B3D19" ||
    contract.target_link !== "LINK-FORUM-03" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.canonical_plan !== planPath ||
    contract.review_contract !== reviewContractPath ||
    contract.promotion_candidate !== candidatePath ||
    contract.complete_artifact !== completePath ||
    contract.packet_builder !==
      "scripts/evidence/prepare-link-forum-03-canonical-promotion-packet.mjs" ||
    contract.verifier !==
      "scripts/verify/verify-link-forum-03-canonical-promotion-packet.mjs"
  ) {
    fail("D19 contract identity or paths drifted");
  }
  if (
    contract.output_packet?.path !== outputPath ||
    contract.output_packet?.status !==
      "ready_for_separate_canonical_source_pull_request" ||
    contract.output_packet?.hand_editing_forbidden !== true ||
    contract.output_packet?.source_commit_required !== true ||
    contract.output_packet?.atomic_replace !== true ||
    contract.output_packet?.automatic_canonical_source_mutation !== false
  ) {
    fail("D19 output packet boundary drifted");
  }
  if (
    contract.required_ledger_transition?.before !== plannedRow ||
    contract.required_ledger_transition?.after !== doneRow ||
    contract.required_ledger_transition?.exact_before_occurrences !== 1 ||
    contract.required_ledger_transition?.exact_after_occurrences_before_promotion !== 0
  ) {
    fail("D19 required ledger transition drifted");
  }
  if (
    contract.proposed_transition?.task !== "LINK-FORUM-03" ||
    contract.proposed_transition?.from !== "planned" ||
    contract.proposed_transition?.to !== "done" ||
    contract.proposed_transition?.requires_separate_canonical_source_pull_request !== true ||
    contract.proposed_transition?.canonical_source_mutated_by_builder !== false ||
    contract.proposed_transition?.promotes_forum_21 !== false ||
    contract.proposed_transition?.promotes_forum_23 !== false
  ) {
    fail("D19 proposed transition drifted");
  }
}

function validateReviewContract(contract) {
  requireObject(contract, "D18 review contract");
  if (
    contract.contract !==
      "forum_search_link_forum_03_complete_evidence_promotion_v1" ||
    contract.task !== "FORUM-23B2G2B3D18" ||
    contract.target_link !== "LINK-FORUM-03" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.canonical_plan !== planPath ||
    contract.promotion_candidate?.path !== candidatePath ||
    contract.complete_artifact !== completePath
  ) {
    fail("D18 review contract identity or paths drifted");
  }
  exactArray(contract.required_scenarios, scenarioIds, "D18 required scenarios");
  if (
    contract.proposed_transition?.task !== "LINK-FORUM-03" ||
    contract.proposed_transition?.from !== "planned" ||
    contract.proposed_transition?.to !== "done" ||
    contract.proposed_transition?.requires_separate_canonical_source_pull_request !== true ||
    contract.proposed_transition?.canonical_source_mutated_by_reviewer !== false ||
    contract.proposed_transition?.promotes_forum_21 !== false ||
    contract.proposed_transition?.promotes_forum_23 !== false
  ) {
    fail("D18 transition boundary drifted");
  }
}

function validatePlan(plan, planDigest) {
  if (countOccurrences(plan, plannedRow) !== 1) {
    fail("canonical plan must contain the exact planned LINK-FORUM-03 row once");
  }
  if (countOccurrences(plan, doneRow) !== 0) {
    fail("canonical plan already contains the D19 done row");
  }
  if (!plan.includes(forum21Row) || !plan.includes(forum23Marker)) {
    fail("canonical plan no longer preserves FORUM-21 planned and FORUM-23 in_progress");
  }
  return {
    path: planPath,
    sha256_before: planDigest,
    statuses_before: {
      forum_21: "planned",
      forum_23: "in_progress",
      link_forum_03: "planned",
    },
  };
}

function validateCompleteArtifact(candidate, head) {
  const bytes = readBytes(completePath);
  const artifact = parseJson(completePath, bytes);
  requireObject(artifact, "D17 complete artifact");
  if (
    artifact.contract !== "link_forum_03_forum_index_search_complete_evidence_v1" ||
    artifact.task !== "LINK-FORUM-03" ||
    artifact.source_slice !== "FORUM-23B2G2B3D17" ||
    artifact.status !== "complete_runtime_evidence_assembled_review_pending" ||
    artifact.coverage !== "canonical_link_forum_03_runtime_scope" ||
    artifact.source_commit !== head
  ) {
    fail("D17 complete artifact identity status coverage or source commit drifted");
  }
  requireTimestamp(artifact.generated_at, "D17 complete artifact generated_at");
  exactArray(artifact.assembled_scenario_ids, scenarioIds, "D17 complete scenarios");
  const digest = sha256(bytes);
  if (
    candidate.complete_artifact?.path !== completePath ||
    candidate.complete_artifact?.sha256 !== digest ||
    candidate.complete_artifact?.byte_length !== bytes.length ||
    candidate.complete_artifact?.generated_at !== artifact.generated_at ||
    candidate.complete_artifact?.status_at_review !== artifact.status ||
    candidate.complete_artifact?.coverage !== artifact.coverage
  ) {
    fail("D18 candidate complete-artifact metadata drifted");
  }
  exactArray(candidate.complete_artifact.scenario_ids, scenarioIds, "candidate scenarios");
  return {
    path: completePath,
    sha256: digest,
    byte_length: bytes.length,
    generated_at: artifact.generated_at,
    status: artifact.status,
    coverage: artifact.coverage,
    scenario_ids: scenarioIds,
  };
}

function validateSources(candidate, head) {
  if (!Array.isArray(candidate.source_artifacts)) {
    fail("D18 candidate source_artifacts must be an array");
  }
  exactArray(
    candidate.source_artifacts.map(({ path }) => path),
    sourceSpecs.map(({ path }) => path),
    "D18 candidate source-artifact paths",
  );
  return sourceSpecs.map((spec, index) => {
    const bytes = readBytes(spec.path);
    const artifact = parseJson(spec.path, bytes);
    const retained = candidate.source_artifacts[index];
    requireObject(artifact, spec.path);
    if (
      artifact.contract !== spec.contract ||
      artifact.task !== spec.task ||
      artifact.source_commit !== head
    ) {
      fail(`${spec.path} identity or source commit drifted`);
    }
    requireTimestamp(artifact.generated_at, `${spec.path}.generated_at`);
    const digest = sha256(bytes);
    if (
      retained.path !== spec.path ||
      retained.task !== spec.task ||
      retained.contract !== spec.contract ||
      retained.source_commit !== head ||
      retained.generated_at !== artifact.generated_at ||
      retained.sha256 !== digest ||
      retained.byte_length !== bytes.length
    ) {
      fail(`D18 retained metadata drifted for ${spec.path}`);
    }
    exactArray(retained.scenario_ids, spec.scenarios, `${spec.path} scenarios`);
    return {
      path: spec.path,
      task: spec.task,
      contract: spec.contract,
      source_commit: head,
      generated_at: artifact.generated_at,
      sha256: digest,
      byte_length: bytes.length,
      scenario_ids: spec.scenarios,
    };
  });
}

function validateCandidate(candidate, candidateBytes, reviewContractBytes, planBytes, head) {
  requireObject(candidate, "D18 promotion candidate");
  if (
    candidate.contract !==
      "link_forum_03_forum_index_search_complete_promotion_candidate_v1" ||
    candidate.task !== "FORUM-23B2G2B3D18" ||
    candidate.target_link !== "LINK-FORUM-03" ||
    candidate.status !== "approved_for_canonical_status_promotion" ||
    candidate.source_commit !== head
  ) {
    fail("D18 promotion candidate identity status or source commit drifted");
  }
  requireTimestamp(candidate.reviewed_at, "candidate.reviewed_at");
  requireString(candidate.reviewer, "candidate.reviewer");
  requireObject(candidate.retention, "candidate.retention");
  requireString(candidate.retention.reference, "candidate.retention.reference");
  requireDigest(candidate.retention.attested_sha256, "candidate retained digest");
  if (
    candidate.retention.matches_reviewed_complete_artifact !== true ||
    candidate.retention.external_service_authentication_performed_by_script !== false ||
    candidate.retention.cryptographic_signature_created_by_script !== false
  ) {
    fail("D18 candidate retention boundary drifted");
  }
  const planDigest = sha256(planBytes);
  if (
    candidate.canonical_plan?.path !== planPath ||
    candidate.canonical_plan?.sha256 !== planDigest ||
    candidate.canonical_plan?.forum_21_status_at_review !== "planned" ||
    candidate.canonical_plan?.forum_23_status_at_review !== "in_progress" ||
    candidate.canonical_plan?.link_forum_03_status_at_review !== "planned"
  ) {
    fail("D18 candidate canonical-plan identity status or digest drifted");
  }
  if (
    candidate.review_contract?.path !== reviewContractPath ||
    candidate.review_contract?.sha256 !== sha256(reviewContractBytes) ||
    candidate.review_contract?.status_at_review !==
      "source_ready_maintainer_execution_pending"
  ) {
    fail("D18 candidate review-contract metadata drifted");
  }
  if (
    candidate.proposed_transition?.task !== "LINK-FORUM-03" ||
    candidate.proposed_transition?.from !== "planned" ||
    candidate.proposed_transition?.to !== "done" ||
    candidate.proposed_transition?.separate_canonical_source_pull_request_required !== true ||
    candidate.proposed_transition?.canonical_source_mutated_by_reviewer !== false ||
    candidate.proposed_transition?.promotes_forum_21 !== false ||
    candidate.proposed_transition?.promotes_forum_23 !== false
  ) {
    fail("D18 candidate transition boundary drifted");
  }
  for (const field of [
    "all_six_canonical_scenarios_revalidated",
    "all_four_source_artifacts_revalidated",
    "all_source_digests_match_complete_artifact",
    "all_scenario_facts_match_retained_sources",
    "complete_artifact_source_commit_matches_current_head",
    "complete_artifact_was_review_pending_before_review",
    "retained_digest_attested_by_maintainer",
    "canonical_plan_remained_unmodified",
  ]) {
    if (candidate.validation?.[field] !== true) {
      fail(`D18 candidate validation.${field} must be true`);
    }
  }
  const complete = validateCompleteArtifact(candidate, head);
  if (candidate.retention.attested_sha256 !== complete.sha256) {
    fail("D18 retained digest does not equal the exact complete artifact digest");
  }
  const sources = validateSources(candidate, head);
  return {
    candidate: {
      path: candidatePath,
      sha256: sha256(candidateBytes),
      byte_length: candidateBytes.length,
      source_commit: candidate.source_commit,
      reviewed_at: candidate.reviewed_at,
      reviewer: candidate.reviewer,
      retention: candidate.retention,
    },
    complete,
    sources,
  };
}

if (process.argv.length !== 2) {
  fail("this packet builder accepts no command-line arguments");
}

const head = currentHead();
const contractBytes = readBytes(contractPath);
validatePacketContract(parseJson(contractPath, contractBytes));
const reviewContractBytes = readBytes(reviewContractPath);
validateReviewContract(parseJson(reviewContractPath, reviewContractBytes));
const planBytes = readBytes(planPath);
const planText = planBytes.toString("utf8");
const plan = validatePlan(planText, sha256(planBytes));
const candidateBytes = readBytes(candidatePath);
const validated = validateCandidate(
  parseJson(candidatePath, candidateBytes),
  candidateBytes,
  reviewContractBytes,
  planBytes,
  head,
);

const output = {
  contract: "link_forum_03_canonical_promotion_packet_v1",
  task: "FORUM-23B2G2B3D19",
  target_link: "LINK-FORUM-03",
  status: "ready_for_separate_canonical_source_pull_request",
  source_commit: head,
  generated_at: new Date().toISOString(),
  promotion_candidate: validated.candidate,
  complete_artifact: validated.complete,
  source_artifacts: validated.sources,
  review_contract: {
    path: reviewContractPath,
    sha256: sha256(reviewContractBytes),
  },
  packet_contract: {
    path: contractPath,
    sha256: sha256(contractBytes),
  },
  canonical_plan: plan,
  required_ledger_edit: {
    before: plannedRow,
    after: doneRow,
    exact_before_occurrences: 1,
    exact_after_occurrences_before_promotion: 0,
    automatic_application_performed: false,
  },
  required_completion_record: {
    evidence_paths: [candidatePath, completePath, ...sourceSpecs.map(({ path }) => path)],
    scenario_ids: scenarioIds,
    reviewer: validated.candidate.reviewer,
    reviewed_at: validated.candidate.reviewed_at,
    retention_reference: validated.candidate.retention.reference,
    retained_complete_artifact_sha256:
      validated.candidate.retention.attested_sha256,
    explicit_boundaries: {
      forum_21_remains_planned: true,
      forum_23_remains_in_progress: true,
      link_forum_03_is_the_only_proposed_status_change: true,
    },
  },
  canonical_pull_request_requirements: {
    revalidate_packet_against_exact_head: true,
    update_ledger_and_completion_evidence_together: true,
    preserve_forum_21_status: "planned",
    preserve_forum_23_status: "in_progress",
    cite_reviewer_and_retention_attestation: true,
    claim_runtime_execution_only_from_retained_candidate: true,
  },
  assertions: {
    d18_candidate_approved_and_revalidated: true,
    complete_and_source_artifact_bytes_match_candidate: true,
    candidate_plan_digest_matches_current_plan: true,
    exact_planned_ledger_row_present_once: true,
    exact_done_ledger_row_absent: true,
    canonical_source_mutated_by_builder: false,
    promotes_forum_21: false,
    promotes_forum_23: false,
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
  fail(`atomic promotion-packet write failed: ${error.message}`);
}

console.log(`wrote LINK-FORUM-03 canonical promotion packet to ${outputPath}`);
