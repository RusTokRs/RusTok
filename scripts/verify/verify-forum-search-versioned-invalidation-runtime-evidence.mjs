#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const read = (path) => readFileSync(resolve(root, path), "utf8");
const requireAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing required marker: ${marker}`);
  }
};
const forbidAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(!text.includes(marker), `${label} retains stale marker: ${marker}`);
  }
};

const contractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";
const protocolPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d-runtime-evidence.md";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const ownerSourcePath =
  "crates/rustok-forum/docs/forum-23b2g2b1-search-owner-revision-source.md";
const checkpointPath =
  "crates/rustok-forum/docs/forum-23b2g2b2-search-owner-revision-checkpoint.md";
const wirePath =
  "crates/rustok-forum/docs/forum-23b2g2b3a-versioned-invalidation-wire-contract.md";
const publisherPath =
  "crates/rustok-forum/docs/forum-23b2g2b3b2-versioned-invalidation-publisher.md";
const consumerPath =
  "crates/rustok-forum/docs/forum-23b2g2b3c-versioned-invalidation-consumer.md";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_runtime_evidence_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D0");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(
  contract.canonical_plan_reconciliation,
  "completed_by_FORUM-23B2G2B3D1",
);
assert.equal(
  contract.baseline.main_commit,
  "bd25ea3577b164225359beba86f973e907b74bef",
);
assert.deepEqual(contract.baseline.merged_pull_requests, [2731, 2738, 2741, 2749, 2753]);
assert.equal(contract.baseline.runtime_evidence_protocol, "FORUM-23B2G2B3D0");
assert.equal(contract.baseline.canonical_plan_sync, "FORUM-23B2G2B3D1");
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(contract.required_runtime.delivery_profile, "outbox_iggy");
assert.equal(
  contract.required_runtime.consumer_flag,
  "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED=true",
);
assert.equal(contract.evidence_artifact.generation, "executable_runtime_only");
assert.equal(contract.evidence_artifact.hand_editing_forbidden, true);

const expectedScenarios = [
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
assert.deepEqual(
  contract.required_scenarios.map(({ id }) => id),
  expectedScenarios,
);
for (const scenario of contract.required_scenarios) {
  assert.ok(Array.isArray(scenario.requires) && scenario.requires.length >= 3);
  for (const requirement of scenario.requires) {
    assert.equal(typeof requirement, "string");
    assert.ok(requirement.length > 20);
  }
}

const plan = read(planPath);
const forum23Start = plan.indexOf("## `FORUM-23` — search/index integration");
const forum24Start = plan.indexOf("## `FORUM-24` — localized routes", forum23Start);
assert.ok(forum23Start >= 0 && forum24Start > forum23Start);
const forum23 = plan.slice(forum23Start, forum24Start);
requireAll(
  forum23,
  [
    "**Status:** `in_progress`",
    "FORUM-23B2G2A",
    "FORUM-23B2G2A1",
    "FORUM-23B2G2B1",
    "FORUM-23B2G2B2",
    "FORUM-23B2G2B3A",
    "FORUM-23B2G2B3B1",
    "FORUM-23B2G2B3B2",
    "FORUM-23B2G2B3C",
    "FORUM-23B2G2B3D0",
    "FORUM-23B2G2B3D1",
    "source_ready_maintainer_execution_pending",
    "owner/index revisions reconcile",
    "`LINK-FORUM-03` cross-module runtime proof",
    "verify-forum-search-owner-revision-ledger.mjs",
    "verify-forum-search-owner-revision-counter-hardening.mjs",
    "verify-forum-search-owner-revision-source.mjs",
    "verify-forum-search-owner-revision-checkpoint.mjs",
    "verify-forum-search-versioned-invalidation-wire.mjs",
    "verify-forum-search-versioned-invalidation-causation-api.mjs",
    "verify-forum-search-versioned-invalidation-publisher.mjs",
    "verify-forum-search-versioned-invalidation-consumer.mjs",
    "verify-forum-search-versioned-invalidation-runtime-evidence.mjs",
  ],
  "FORUM-23 canonical plan boundary",
);
forbidAll(
  forum23,
  [
    "owner-issued revision reconciliation plus maintainer runtime evidence remain",
    "add Forum-owner-issued monotonic projection revisions and reconcile them",
  ],
  "FORUM-23 canonical plan boundary",
);

const ownerSource = read(ownerSourcePath);
requireAll(
  ownerSource,
  [
    "`source_complete_runtime_evidence_pending`",
    "## Delivered checkpoint and versioned rollout",
    "FORUM-23B2G2B2",
    "FORUM-23B2G2B3C",
    "FORUM-23B2G2B3D0",
    contractPath,
  ],
  "owner revision source handoff",
);
forbidAll(
  ownerSource,
  ["`source_complete_consumer_checkpoint_pending`", "should add the Search-owned checkpoint"],
  "owner revision source handoff",
);

const checkpoint = read(checkpointPath);
requireAll(
  checkpoint,
  [
    "`source_complete_runtime_evidence_pending`",
    "## Delivered versioned transport rollout",
    "## Runtime evidence boundary",
    "FORUM-23B2G2B3D0",
    contractPath,
  ],
  "owner revision checkpoint handoff",
);
forbidAll(
  checkpoint,
  ["planned versioned owner-revision wire contract", "A later bounded slice must add"],
  "owner revision checkpoint handoff",
);

const wire = read(wirePath);
requireAll(
  wire,
  [
    "`source_complete_runtime_evidence_pending`",
    "## Delivered rollout slices",
    "`FORUM-23B2G2B3B1`",
    "`FORUM-23B2G2B3B2`",
    "`FORUM-23B2G2B3C`",
    "`FORUM-23B2G2B3D`",
  ],
  "versioned invalidation wire handoff",
);
forbidAll(
  wire,
  ["`contract_frozen_implementation_pending`", "## Planned implementation slices"],
  "versioned invalidation wire handoff",
);

const publisher = read(publisherPath);
requireAll(
  publisher,
  [
    "`source_complete_runtime_evidence_pending`",
    "## Delivered consumer",
    "FORUM-23B2G2B3C",
    "FORUM-23B2G2B3D0",
    contractPath,
  ],
  "versioned invalidation publisher handoff",
);
forbidAll(
  publisher,
  ["`source_complete_consumer_pending`", "## Next slice"],
  "versioned invalidation publisher handoff",
);

const consumer = read(consumerPath);
requireAll(
  consumer,
  [
    "source_complete_runtime_evidence_pending",
    "FORUM-23B2G2B3D0",
    contractPath,
    protocolPath,
  ],
  "versioned invalidation consumer handoff",
);

const protocol = read(protocolPath);
requireAll(
  protocol,
  [
    "`source_ready_maintainer_execution_pending`",
    "bd25ea3577b164225359beba86f973e907b74bef",
    "PR #2731",
    "PR #2738",
    "PR #2741",
    "PR #2749",
    "PR #2753",
    "## Delivered canonical plan reconciliation",
    "FORUM-23B2G2B3D1",
    "completed_by_FORUM-23B2G2B3D1",
    "target/forum-search-versioned-invalidation-runtime-evidence.json",
    "No command above was run by the implementation agent",
  ],
  "versioned invalidation runtime evidence protocol",
);

const eventFamily = read("crates/rustok-events/src/forum_search_projection.rs");
requireAll(
  eventFamily,
  [
    "ForumSearchProjectionEvent",
    "forum.search_projection.invalidation_issued",
    "owner_revision",
  ],
  "sealed Forum Search event family",
);

const forumPublisher = read("crates/rustok-forum/src/services/projection_invalidation.rs");
requireAll(
  forumPublisher,
  [
    "publish_contract_in_tx_with_causation",
    "ForumSearchProjectionEvent::InvalidationIssued",
    "record_projection_revision_in_tx",
  ],
  "Forum projection invalidation publisher",
);

const ingress = read("crates/rustok-search/src/forum_contract_ingress.rs");
requireAll(
  ingress,
  [
    "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
    "causation_id()",
    "InboxIdentityConflict",
    "search_projection_inbox",
  ],
  "Search typed invalidation ingress",
);

const worker = read("apps/server/src/services/forum_search_contract_consumer.rs");
requireAll(
  worker,
  [
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "EventDeliveryProfile::OutboxIggy",
    "ConsumerPoisonReceiptStore",
    "open_persistent_contract_consumer_group",
  ],
  "server persistent Forum Search consumer",
);

console.log(
  "Forum Search versioned invalidation handoffs, canonical plan, and runtime-evidence protocol are synchronized.",
);
