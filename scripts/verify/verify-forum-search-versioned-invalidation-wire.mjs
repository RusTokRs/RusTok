#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json",
  publisherContract: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-publisher.json",
  note: "crates/rustok-forum/docs/forum-23b2g2b3a-versioned-invalidation-wire-contract.md",
  decision: "DECISIONS/2026-07-31-forum-search-versioned-invalidation-rollout.md",
  checkpointContract: "crates/rustok-forum/contracts/forum-search-owner-revision-checkpoint.json",
  ledgerContract: "crates/rustok-forum/contracts/forum-search-owner-revision-ledger.json",
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

function target(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  if (!existsSync(target(relativePath))) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target(relativePath), "utf8");
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
const publisherDelivered = existsSync(target(paths.publisherContract));
const publisherContract = publisherDelivered ? parseJson(paths.publisherContract) : null;
const checkpointContract = parseJson(paths.checkpointContract);
const ledgerContract = parseJson(paths.ledgerContract);
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
  "FORUM-23B2G2B3C",
], paths.note);
requireAll(outbox, [
  "publish_contract_in_tx_with_causation",
  "ContractEventEnvelope::new_caused_by",
  "write_contract_to_outbox",
], `${paths.outbox} typed publication boundary`);
requireAll(searchInbox, [
  "ON CONFLICT (event_id) DO NOTHING",
  "envelope.id.into()",
  "ORDER BY ingest_sequence ASC",
], `${paths.searchInbox} one-inbox predecessor`);

if (contract) {
  if (contract.task !== "FORUM-23B2G2B3A"
      || contract.status !== "contract_frozen_implementation_pending") {
    failures.push(`${paths.contract}: historical freeze identity drift`);
  }
  const event = contract.event_contract;
  if (event?.rust_family !== "ForumSearchProjectionEvent"
      || event?.transport_family !== "forum_search_projection"
      || event?.variant !== "InvalidationIssued"
      || event?.event_type !== "forum.search_projection.invalidation_issued"
      || event?.schema_version !== 1
      || event?.payload?.owner_revision?.minimum !== 1) {
    failures.push(`${paths.contract}: event contract drift`);
  }
  if (JSON.stringify(event?.payload?.target_type?.allowed ?? []) !== JSON.stringify([
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
  if (contract.owner_transaction_protocol?.postgresql_only !== true
      || contract.owner_transaction_protocol?.legacy_root_publication_required !== true
      || contract.owner_transaction_protocol?.failure_rolls_back_owner_state_root_typed_and_ledger !== true) {
    failures.push(`${paths.contract}: owner transaction protocol drift`);
  }
  if (contract.single_projection_path?.inbox_table !== "search_projection_inbox"
      || contract.single_projection_path?.typed_ingress_identity !== "ContractEventEnvelope.causation_id"
      || contract.single_projection_path?.second_inbox_forbidden !== true
      || contract.single_projection_path?.second_projector_forbidden !== true) {
    failures.push(`${paths.contract}: one-inbox execution contract drift`);
  }
  if (contract.ordering?.owner_revision_compared_numerically_with_ingest_sequence !== false
      || contract.ordering?.checkpoint_advances_only_after_projection_success !== true) {
    failures.push(`${paths.contract}: independent ordering contract drift`);
  }
  for (const [name, value] of Object.entries(contract.current_slice_changes ?? {})) {
    if (value !== false) failures.push(`${paths.contract}: historical ${name} must remain false`);
  }
}

if (checkpointContract?.task !== "FORUM-23B2G2B2"
    || ledgerContract?.task !== "FORUM-23B2G2A"
    || sourceContract?.task !== "FORUM-23B2G2B1"
    || ingestContract?.task !== "FORUM-23B2G1") {
  failures.push("versioned wire predecessor contract drift");
}

const prePublisherDigest = {
  format_version: 1,
  registry: "sha256:18fdee49d915a22ed3dd709ec6cc1826d6d47a59ddfe659fbd07ede7f6cd3d07",
  root_event: "sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87",
  root_envelope: "sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d",
  contract_payload: "sha256:5f1f9577bc9429b76bbfe5420d1bd71249efd6aea702ec0a103e4a402702cd02",
  contract_envelope: "sha256:b29a8c7809045e14f1233db4e2f9dba9cedf96352df3ea9e87ca1e86f6a59eb8",
};

if (!publisherDelivered) {
  for (const [source, label] of [
    [eventLib, paths.eventLib],
    [eventPayload, paths.eventPayload],
    [forumPublisher, paths.forumPublisher],
  ]) {
    rejectAll(source, [
      "ForumSearchProjectionEvent",
      "forum_search_projection",
      "forum.search_projection.invalidation_issued",
    ], `${label} pre-publisher boundary`);
  }
  if (digests && JSON.stringify(digests) !== JSON.stringify(prePublisherDigest)) {
    failures.push(`${paths.eventDigests}: digest changed before publisher delivery`);
  }
} else {
  if (publisherContract?.task !== "FORUM-23B2G2B3B2"
      || publisherContract?.status !== "source_complete_consumer_pending") {
    failures.push(`${paths.publisherContract}: downstream publisher identity drift`);
  }
  requireAll(eventLib, [
    "ForumSearchProjectionEvent",
    "FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS",
    "forum_search_projection_event_schema",
  ], paths.eventLib);
  requireAll(eventPayload, [
    "ForumSearchProjection(ForumSearchProjectionEvent)",
    "#[serde(rename = \"forum_search_projection\")]",
  ], paths.eventPayload);
  requireAll(forumPublisher, [
    "ForumSearchProjectionEvent::InvalidationIssued",
    "publish_contract_in_tx_with_causation",
    "publish_contract_direct_in_tx_with_causation_and_envelope_id",
    "record_projection_revision_in_tx",
  ], paths.forumPublisher);
  const expected = {
    format_version: 1,
    ...publisherContract.digest_generation.new_values,
  };
  if (digests && JSON.stringify(digests) !== JSON.stringify(expected)) {
    failures.push(`${paths.eventDigests}: downstream publisher digest drift`);
  }
}

requireAll(forumPlan, [
  "| `FORUM-23` | `in_progress` |",
  "owner-issued revision reconciliation plus maintainer runtime evidence remain",
], `${paths.forumPlan} canonical open boundary`);
requireAll(searchPlan, [
  "This is not the final Forum-owner-issued projection revision",
  "owner contract and rollout reconciliation remain pending",
], `${paths.searchPlan} canonical open boundary`);

if (failures.length > 0) {
  console.error("FORUM-23B2G2B3A versioned invalidation wire verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2B3A versioned invalidation wire contract is consistent.");
