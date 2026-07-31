#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json",
  note: "crates/rustok-forum/docs/forum-23b2g2b3a-versioned-invalidation-wire-contract.md",
  decision: "DECISIONS/2026-07-31-forum-search-versioned-invalidation-rollout.md",
  checkpointContract: "crates/rustok-forum/contracts/forum-search-owner-revision-checkpoint.json",
  ledgerContract: "crates/rustok-forum/contracts/forum-search-owner-revision-ledger.json",
  hardeningContract: "crates/rustok-forum/contracts/forum-search-owner-revision-counter-hardening.json",
  sourceContract: "crates/rustok-forum/contracts/forum-search-owner-revision-source.json",
  ingestContract: "crates/rustok-forum/contracts/forum-search-durable-ingest-sequence.json",
  eventLib: "crates/rustok-events/src/lib.rs",
  eventPayload: "crates/rustok-events/src/contract.rs",
  eventDigests: "crates/rustok-events/contracts/event-contract-digests.json",
  outbox: "crates/rustok-outbox/src/transactional.rs",
  forumPublisher: "crates/rustok-forum/src/services/projection_invalidation.rs",
  searchInbox: "crates/rustok-search/src/forum_inbox.rs",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const contract = parseJson(paths.contract);
const checkpointContract = parseJson(paths.checkpointContract);
const ledgerContract = parseJson(paths.ledgerContract);
const hardeningContract = parseJson(paths.hardeningContract);
const sourceContract = parseJson(paths.sourceContract);
const ingestContract = parseJson(paths.ingestContract);
const digests = parseJson(paths.eventDigests);

const note = read(paths.note);
const decision = read(paths.decision);
const eventLib = read(paths.eventLib);
const eventPayload = read(paths.eventPayload);
const outbox = read(paths.outbox);
const forumPublisher = read(paths.forumPublisher);
const searchInbox = read(paths.searchInbox);
const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);

requireAll(decision, [
  "# ADR: Forum Search versioned invalidation rollout",
  "- Status: accepted",
  "forum.search_projection.invalidation_issued",
  "causation_id",
  "search_projection_inbox.event_id",
  "The legacy root publication remains mandatory",
  "No process-local fallback or second projector is permitted",
], paths.decision);

requireAll(note, [
  "# FORUM-23B2G2B3A versioned Search invalidation wire contract",
  "contract_frozen_implementation_pending",
  "family: forum_search_projection",
  "ForumSearchProjectionEvent",
  "forum.search_projection.invalidation_issued",
  "typed ingress inbox identity  = ContractEventEnvelope.causation_id",
  "ON CONFLICT (event_id) DO NOTHING",
  "FORUM-23B2G2B3B",
  "FORUM-23B2G2B3C",
  "FORUM-23B2G2B3D",
  "The implementation agent did not run these commands",
], paths.note);

requireAll(outbox, [
  "publish_contract_in_tx_with_envelope_id",
  "ContractEventEnvelope::new",
  "write_contract_to_outbox",
], `${paths.outbox} typed publication predecessor`);

requireAll(forumPublisher, [
  "DomainEvent::ReindexRequested",
  "publish_in_tx_with_envelope_id",
  "forum_projection_revision_ledger",
  "record_projection_revision_in_tx",
], `${paths.forumPublisher} legacy identity predecessor`);

requireAll(searchInbox, [
  "ON CONFLICT (event_id) DO NOTHING",
  "envelope.id.into()",
  "ORDER BY ingest_sequence ASC",
  "use rustok_events::{DomainEvent, EventEnvelope};",
], `${paths.searchInbox} one-inbox predecessor`);

// G2B3A is a contract freeze only. Executable registration and publication are
// intentionally forbidden until G2B3B updates the schema digest in the same PR.
for (const [source, label] of [
  [eventLib, paths.eventLib],
  [eventPayload, paths.eventPayload],
  [forumPublisher, paths.forumPublisher],
]) {
  rejectAll(source, [
    "ForumSearchProjectionEvent",
    "forum_search_projection",
    "forum.search_projection.invalidation_issued",
  ], `${label} G2B3A implementation boundary`);
}

requireAll(forumPlan, [
  "| `FORUM-23` | `in_progress` |",
  "owner-issued revision reconciliation plus maintainer runtime evidence remain",
], `${paths.forumPlan} canonical open boundary`);
requireAll(searchPlan, [
  "This is not the final Forum-owner-issued projection revision",
  "owner contract and rollout reconciliation remain pending",
], `${paths.searchPlan} canonical open boundary`);

if (contract) {
  if (contract.task !== "FORUM-23B2G2B3A") {
    failures.push(`${paths.contract}: unexpected task`);
  }
  if (contract.status !== "contract_frozen_implementation_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  const event = contract.event_contract;
  if (event?.registry_owner !== "rustok-events"
      || event?.publisher_owner !== "rustok-forum"
      || event?.consumer_owner !== "rustok-search") {
    failures.push(`${paths.contract}: event ownership drift`);
  }
  if (event?.rust_family !== "ForumSearchProjectionEvent"
      || event?.transport_family !== "forum_search_projection"
      || event?.variant !== "InvalidationIssued"
      || event?.event_type !== "forum.search_projection.invalidation_issued"
      || event?.schema_version !== 1) {
    failures.push(`${paths.contract}: event identity drift`);
  }
  if (event?.payload?.owner_revision?.minimum !== 1) {
    failures.push(`${paths.contract}: owner revision must be positive`);
  }
  const targets = event?.payload?.target_type?.allowed ?? [];
  if (JSON.stringify(targets) !== JSON.stringify([
    "forum",
    "forum_category",
    "forum_topic",
  ])) {
    failures.push(`${paths.contract}: target type set or order drift`);
  }
  if (event?.envelope?.causation_id_required !== true
      || event?.envelope?.causation_id_equals_legacy_root_envelope_id !== true
      || event?.envelope?.legacy_root_event_type !== "index.reindex_requested") {
    failures.push(`${paths.contract}: legacy causation identity drift`);
  }
  const transaction = contract.owner_transaction_protocol;
  if (transaction?.postgresql_only !== true
      || transaction?.legacy_root_publication_required !== true
      || transaction?.failure_rolls_back_owner_state_root_typed_and_ledger !== true) {
    failures.push(`${paths.contract}: owner transaction protocol drift`);
  }
  const execution = contract.single_projection_path;
  if (execution?.inbox_table !== "search_projection_inbox"
      || execution?.typed_ingress_identity !== "ContractEventEnvelope.causation_id"
      || execution?.shared_identity !== "legacy root envelope ID"
      || execution?.existing_inbox_unique_boundary_collapses_dual_delivery !== true
      || execution?.second_inbox_forbidden !== true
      || execution?.second_projector_forbidden !== true) {
    failures.push(`${paths.contract}: one-inbox execution contract drift`);
  }
  const cursor = contract.consumer_cursor_protocol;
  if (cursor?.valid_event_ack_after_terminal_result !== true
      || cursor?.poison_ack_after_terminal_result !== true
      || cursor?.transient_failure_acknowledged !== false
      || cursor?.process_local_fallback !== false
      || cursor?.exact_cursor_receive_and_ack !== true) {
    failures.push(`${paths.contract}: persistent cursor or poison policy drift`);
  }
  if (contract.ordering?.owner_revision_compared_numerically_with_ingest_sequence !== false
      || contract.ordering?.checkpoint_advances_only_after_projection_success !== true) {
    failures.push(`${paths.contract}: independent ordering contract drift`);
  }
  const slices = (contract.rollout_slices ?? []).map((slice) => slice.task);
  if (JSON.stringify(slices) !== JSON.stringify([
    "FORUM-23B2G2B3A",
    "FORUM-23B2G2B3B",
    "FORUM-23B2G2B3C",
    "FORUM-23B2G2B3D",
  ])) {
    failures.push(`${paths.contract}: rollout slice order drift`);
  }
  const changes = contract.current_slice_changes;
  for (const [name, value] of Object.entries(changes ?? {})) {
    if (value !== false) failures.push(`${paths.contract}: ${name} must remain false in G2B3A`);
  }
}

if (checkpointContract?.task !== "FORUM-23B2G2B2") {
  failures.push(`${paths.checkpointContract}: checkpoint predecessor drift`);
}
if (ledgerContract?.task !== "FORUM-23B2G2A") {
  failures.push(`${paths.ledgerContract}: ledger predecessor drift`);
}
if (hardeningContract?.task !== "FORUM-23B2G2A1") {
  failures.push(`${paths.hardeningContract}: hardening predecessor drift`);
}
if (sourceContract?.task !== "FORUM-23B2G2B1") {
  failures.push(`${paths.sourceContract}: source predecessor drift`);
}
if (ingestContract?.task !== "FORUM-23B2G1") {
  failures.push(`${paths.ingestContract}: ingest predecessor drift`);
}

const expectedDigests = {
  format_version: 1,
  registry: "sha256:18fdee49d915a22ed3dd709ec6cc1826d6d47a59ddfe659fbd07ede7f6cd3d07",
  root_event: "sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87",
  root_envelope: "sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d",
  contract_payload: "sha256:5f1f9577bc9429b76bbfe5420d1bd71249efd6aea702ec0a103e4a402702cd02",
  contract_envelope: "sha256:b29a8c7809045e14f1233db4e2f9dba9cedf96352df3ea9e87ca1e86f6a59eb8",
};
if (digests && JSON.stringify(digests) !== JSON.stringify(expectedDigests)) {
  failures.push(`${paths.eventDigests}: G2B3A must not change the event schema digest baseline`);
}

if (failures.length > 0) {
  console.error("FORUM-23B2G2B3A versioned invalidation wire verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2B3A versioned invalidation wire contract is consistent.");
