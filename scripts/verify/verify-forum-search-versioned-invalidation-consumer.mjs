#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();
const failures = [];

const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-consumer.json",
  note: "crates/rustok-forum/docs/forum-23b2g2b3c-versioned-invalidation-consumer.md",
  publisherContract: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-publisher.json",
  ingress: "crates/rustok-search/src/forum_contract_ingress.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  searchCargo: "crates/rustok-search/Cargo.toml",
  inbox: "crates/rustok-search/src/forum_inbox.rs",
  reconciler: "crates/rustok-search/src/forum_reconciliation.rs",
  worker: "apps/server/src/services/forum_search_contract_consumer.rs",
  workerOwner: "apps/server/src/services/forum_search_inbox_worker.rs",
  bootstrap: "apps/server/src/services/server_bootstrap.rs",
  migrations: "crates/rustok-search/src/migrations/mod.rs",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
};

function target(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  if (!fs.existsSync(target(relativePath))) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return fs.readFileSync(target(relativePath), "utf8");
}

function readJson(relativePath) {
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

const contract = readJson(paths.contract);
const publisherContract = readJson(paths.publisherContract);
const note = read(paths.note);
const ingress = read(paths.ingress);
const searchLib = read(paths.searchLib);
const searchCargo = read(paths.searchCargo);
const inbox = read(paths.inbox);
const reconciler = read(paths.reconciler);
const worker = read(paths.worker);
const workerOwner = read(paths.workerOwner);
const bootstrap = read(paths.bootstrap);
const migrations = read(paths.migrations);
const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);

if (publisherContract?.task !== "FORUM-23B2G2B3B2"
    || publisherContract?.status !== "source_complete_consumer_pending") {
  failures.push(`${paths.publisherContract}: predecessor identity drift`);
}

if (contract) {
  if (contract.task !== "FORUM-23B2G2B3C"
      || contract.status !== "source_complete_runtime_evidence_pending") {
    failures.push(`${paths.contract}: task or status drift`);
  }
  if (contract.transport?.topic !== "domain"
      || contract.transport?.consumer_group !== "rustok-search-forum-projection-v1"
      || contract.transport?.persistent_cursor_required !== true
      || contract.transport?.postgresql_required !== true
      || contract.transport?.delivery_profile_required !== "outbox_iggy") {
    failures.push(`${paths.contract}: persistent transport boundary drift`);
  }
  if (contract.accepted_event?.event_type !== "forum.search_projection.invalidation_issued"
      || contract.accepted_event?.schema_version !== 1
      || contract.accepted_event?.projection_identity !== "ContractEventEnvelope.causation_id"
      || contract.accepted_event?.projection_identity_equals_legacy_root_envelope_id !== true
      || contract.accepted_event?.typed_envelope_id_is_projection_identity !== false) {
    failures.push(`${paths.contract}: event or projection identity drift`);
  }
  if (contract.single_execution_path?.inbox_table !== "search_projection_inbox"
      || contract.single_execution_path?.existing_inbox_reused !== true
      || contract.single_execution_path?.existing_reconciler_reused !== true
      || contract.single_execution_path?.existing_projector_reused !== true
      || contract.single_execution_path?.second_inbox_created !== false
      || contract.single_execution_path?.second_reconciler_created !== false
      || contract.single_execution_path?.second_projector_created !== false
      || contract.single_execution_path?.duplicate_recognition_requires_exact_tenant_scope_and_payload_match !== true) {
    failures.push(`${paths.contract}: single execution path drift`);
  }
  if (contract.ordering?.search_owned_clock !== "search_projection_inbox.ingest_sequence"
      || contract.ordering?.forum_owner_clock !== "owner_revision"
      || contract.ordering?.owner_revision_compared_numerically_with_ingest_sequence !== false
      || contract.ordering?.owner_revision_stored_as_search_ingest_sequence !== false) {
    failures.push(`${paths.contract}: independent clock contract drift`);
  }
  if (contract.poison_policy?.receipt_store !== "iggy_connector_consumer_poison_receipts"
      || contract.poison_policy?.deterministic_dlq_message_id !== true
      || contract.poison_policy?.process_local_fallback !== false
      || contract.poison_policy?.new_terminal_result_allowed_when_dlq_disabled !== false) {
    failures.push(`${paths.contract}: poison receipt policy drift`);
  }
  if (contract.follow_up?.task !== "FORUM-23B2G2B3D") {
    failures.push(`${paths.contract}: evidence follow-up drift`);
  }
}

requireAll(note, [
  "# FORUM-23B2G2B3C versioned Search invalidation consumer",
  "source_complete_runtime_evidence_pending",
  "rustok-search-forum-projection-v1",
  "ContractEventEnvelope.causation_id",
  "search_projection_inbox.event_id",
  "No second inbox, reconciler, projector",
  "forum.search_projection.contract_inbox_identity_conflict",
  "iggy_connector_consumer_poison_receipts",
  "FORUM-23B2G2B3D",
], paths.note);

requireAll(ingress, [
  "pub struct ForumSearchContractIngress",
  "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
  "forum.search_projection.invalidation_issued",
  "ContractEventPayload::ForumSearchProjection",
  ".causation_id()",
  "DomainEvent::ReindexRequested",
  "ForumProjectionInbox::new",
  ".enqueue(&adapted.root_envelope, &adapted.scope)",
  "FROM search_projection_inbox",
  "stored_envelope.id == adapted.root_event_id",
  "stored_envelope.correlation_id == adapted.root_event_id",
  "stored_envelope.event == *expected_event",
  "InboxIdentityConflict",
], paths.ingress);
rejectAll(ingress, [
  "owner_revision > ingest_sequence",
  "owner_revision < ingest_sequence",
  "owner_revision == ingest_sequence",
  "typed_envelope_id",
], `${paths.ingress} independent clock boundary`);

requireAll(searchLib, [
  "mod forum_contract_ingress;",
  "ForumSearchContractIngress",
  "FORUM_SEARCH_CONTRACT_CONSUMER_GROUP",
], paths.searchLib);
rejectAll(searchCargo, [
  "rustok-iggy",
  "rustok-iggy-connector",
], `${paths.searchCargo} transport-neutral boundary`);

requireAll(inbox, [
  "INSERT INTO search_projection_inbox",
  "ON CONFLICT (event_id) DO NOTHING",
  "ORDER BY ingest_sequence ASC",
], `${paths.inbox} shared inbox`);
requireAll(reconciler, [
  "pub struct ForumProjectionReconciler",
  "ForumProjectionInbox",
  "ForumSearchProjector",
], `${paths.reconciler} existing execution owner`);

requireAll(worker, [
  "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
  "PersistentContractConsumerGroup",
  "PersistentContractDelivery::Event",
  "PersistentContractDelivery::DecodeFailure",
  "ForumSearchContractIngress",
  "ConsumerPoisonReceiptStore",
  "ConsumerPoisonIdentity",
  "ConsumerPoisonPublishClaim::Claimed",
  "move_to_dlq",
  "mark_published",
  "acknowledge_decode_failure",
  "acknowledge_event",
  "broker offset remains uncommitted",
], paths.worker);
requireAll(workerOwner, [
  "#[path = \"forum_search_contract_consumer.rs\"]",
  "start_forum_search_contract_consumer_if_enabled",
], `${paths.workerOwner} single host owner`);
requireAll(bootstrap, [
  "start_forum_search_inbox_worker_if_ready",
  "start_forum_search_contract_consumer_if_enabled",
  ".await?;",
], paths.bootstrap);

rejectAll(migrations, [
  "create_forum_search_contract_inbox",
  "create_forum_search_contract_cursor",
  "create_forum_search_contract_poison",
], `${paths.migrations} no parallel persistence`);

requireAll(forumPlan, [
  "| `FORUM-23` | `in_progress` |",
  "LINK-FORUM-03",
], paths.forumPlan);
requireAll(searchPlan, [
  "Add owner-issued Forum projection revisions",
  "LINK-FORUM-03",
], paths.searchPlan);

if (failures.length > 0) {
  console.error("Forum Search versioned invalidation consumer verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search versioned invalidation consumer source contract verified.");
