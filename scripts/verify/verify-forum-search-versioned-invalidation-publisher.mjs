#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();
const failures = [];

const paths = {
  event: "crates/libs/rustok-events/src/forum_search_projection.rs",
  eventTest: "crates/libs/rustok-events/tests/forum_search_projection_contracts.rs",
  eventLib: "crates/libs/rustok-events/src/lib.rs",
  eventPayload: "crates/libs/rustok-events/src/contract.rs",
  eventApi: "crates/libs/rustok-events/CRATE_API.md",
  digests: "crates/libs/rustok-events/contracts/event-contract-digests.json",
  outbox: "crates/modules/rustok-outbox/src/transactional.rs",
  publisher: "crates/modules/rustok-forum/src/services/projection_invalidation.rs",
  contract: "crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-publisher.json",
  note: "crates/modules/rustok-forum/docs/forum-23b2g2b3b2-versioned-invalidation-publisher.md",
  wireContract: "crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json",
  causationContract: "crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-causation-api.json",
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

// `event-contract-digests.json` is a *generated* artifact: only
// `cargo run --locked -p rustok-events --example event_contract_digests -- --write`
// can prove a value, and the `event-contract-digest-admission` workflow pins that
// command (see `verify-event-contract-digest-admission.mjs`). A source-text guard
// cannot recompute SHA-256 over the Rust event registry.
//
// What this guard can prove is (a) the committed artifact is a well-formed
// generator artifact and (b) it is no longer the pre-publisher snapshot, i.e. the
// publisher's event family actually reached the registry. The third, historical
// claim - exact equality with the values recorded at publisher delivery - is
// deliberately not asserted: every later event family legitimately changes the
// registry digest, so that equality rots into a false failure for correct work.
// Exactness is the admission workflow's job, not this guard's.
const DIGEST_KEYS = [
  "registry",
  "root_event",
  "root_envelope",
  "contract_payload",
  "contract_envelope",
];
const PRE_PUBLISHER_DIGESTS = {
  format_version: 1,
  registry: "sha256:18fdee49d915a22ed3dd709ec6cc1826d6d47a59ddfe659fbd07ede7f6cd3d07",
  root_event: "sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87",
  root_envelope: "sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d",
  contract_payload: "sha256:5f1f9577bc9429b76bbfe5420d1bd71249efd6aea702ec0a103e4a402702cd02",
  contract_envelope: "sha256:b29a8c7809045e14f1233db4e2f9dba9cedf96352df3ea9e87ca1e86f6a59eb8",
};

function checkDigestArtifact(artifact, label, fail) {
  if (!artifact) return;
  if (artifact.format_version !== 1) fail(`${label}: format_version must be 1`);
  const keys = Object.keys(artifact)
    .filter((key) => key !== "format_version")
    .sort();
  if (JSON.stringify(keys) !== JSON.stringify([...DIGEST_KEYS].sort())) {
    fail(`${label}: unexpected digest keys ${JSON.stringify(keys)}`);
    return;
  }
  for (const key of DIGEST_KEYS) {
    if (!/^sha256:[0-9a-f]{64}$/.test(artifact[key] ?? "")) {
      fail(`${label}: ${key} is not a sha256 digest`);
    }
  }
}

function assertNotPrePublisher(artifact, label, fail) {
  if (!artifact) return;
  if (DIGEST_KEYS.every((key) => artifact[key] === PRE_PUBLISHER_DIGESTS[key])) {
    fail(
      `${label}: still equals the pre-publisher digest snapshot; the delivered event family never reached the registry`,
    );
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

function functionBlock(source, startMarker, endMarker, label) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0 || end <= start) {
    failures.push(`${label}: function boundary not found`);
    return "";
  }
  return source.slice(start, end);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing ordered marker ${marker}`);
      return;
    }
    if (index <= previous) {
      failures.push(`${label}: marker order drift at ${marker}`);
      return;
    }
    previous = index;
  }
}

const event = read(paths.event);
const eventTest = read(paths.eventTest);
const eventLib = read(paths.eventLib);
const eventPayload = read(paths.eventPayload);
const eventApi = read(paths.eventApi);
const outbox = read(paths.outbox);
const publisher = read(paths.publisher);
const note = read(paths.note);
const contract = readJson(paths.contract);
const wireContract = readJson(paths.wireContract);
const causationContract = readJson(paths.causationContract);
const digests = readJson(paths.digests);

requireAll(event, [
  "pub enum ForumSearchProjectionEvent",
  "InvalidationIssued",
  "owner_revision: i64",
  "target_type: String",
  "target_id: Option<Uuid>",
  "forum.search_projection.invalidation_issued",
  "FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS",
  "impl sealed::Sealed for ForumSearchProjectionEvent",
  "impl EventContract for ForumSearchProjectionEvent",
  "ContractEventPayload::ForumSearchProjection(self)",
  "validators::validate_range(\"owner_revision\", *owner_revision, 1, i64::MAX)",
  "(\"forum\", None) => Ok(())",
  "(\"forum_category\" | \"forum_topic\", Some(target_id))",
  "EventValidationError::MissingField(\"target_id\")",
  "must be forum, forum_category, or forum_topic",
], paths.event);
rejectAll(event, [
  "ingest_sequence",
  "locale",
  "visibility_snapshot",
  "document_payload",
  "claims",
  "roles",
], `${paths.event} bounded payload`);
requireAll(eventTest, [
  "forum_search_projection_contract_roundtrips_with_root_causation",
  "ContractEventEnvelope::new_caused_by",
  "ContractEventPayload::ForumSearchProjection",
  "forum_search_projection_registry_and_scope_validation_are_fail_closed",
  "event_schema(\"forum.search_projection.invalidation_issued\")",
], paths.eventTest);

requireAll(eventLib, [
  "mod forum_search_projection;",
  "FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS",
  "ForumSearchProjectionEvent",
  "forum_search_projection_event_schema(event_type)",
  ".chain(FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS.iter())",
], paths.eventLib);
requireAll(eventPayload, [
  "ForumSearchProjectionEvent",
  "#[serde(rename = \"forum_search_projection\")]",
  "ForumSearchProjection(ForumSearchProjectionEvent)",
  "Self::ForumSearchProjection(event) => event.event_type()",
  "Self::ForumSearchProjection(event) => event.schema_version()",
  "Self::ForumSearchProjection(event) => event.validate()",
], paths.eventPayload);
requireAll(eventApi, [
  "ForumSearchProjectionEvent",
  "forum.search_projection.invalidation_issued",
  "Forum owner revision and Search-owned `ingest_sequence` remain independent",
], paths.eventApi);

requireAll(outbox, [
  "publish_contract_direct_in_tx_with_causation_and_envelope_id",
  "publish_contract_in_tx_with_causation",
  "ContractEventEnvelope::new_caused_by",
  "OutboxTransport::write_contract_envelope_in_tx",
], paths.outbox);
requireAll(publisher, [
  "DomainEvent::ReindexRequested",
  "ForumSearchProjectionEvent::InvalidationIssued",
  "allocate_projection_revision_in_tx",
  "publish_root_in_tx_with_envelope_id",
  "publish_contract_direct_in_tx_with_causation_and_envelope_id",
  "publish_in_tx_with_envelope_id",
  "publish_contract_in_tx_with_causation",
  "record_projection_revision_in_tx",
  "forum_projection_revision_ledger",
], paths.publisher);
rejectAll(publisher, [
  "typed_envelope_id",
  "ingest_sequence",
], `${paths.publisher} identity and ordering boundary`);

const direct = functionBlock(
  publisher,
  "async fn write_projection_invalidation_in_tx(",
  "async fn publish_projection_invalidation_in_tx(",
  "direct publisher",
);
const composed = functionBlock(
  publisher,
  "async fn publish_projection_invalidation_in_tx(",
  "fn projection_invalidation_event(",
  "composed publisher",
);
requireOrder(direct, [
  "allocate_projection_revision_in_tx",
  "publish_root_in_tx_with_envelope_id",
  "publish_contract_direct_in_tx_with_causation_and_envelope_id",
  "record_projection_revision_in_tx",
], "direct PostgreSQL publication order");
requireOrder(composed, [
  "allocate_projection_revision_in_tx",
  "publish_in_tx_with_envelope_id",
  "publish_contract_in_tx_with_causation",
  "record_projection_revision_in_tx",
], "composed PostgreSQL publication order");
requireAll(direct, [
  "txn.get_database_backend() != DatabaseBackend::Postgres",
  "root_event.validate()",
  "root_event_id",
  "record_projection_revision_in_tx(",
], "direct PostgreSQL boundary");
requireAll(composed, [
  "txn.get_database_backend() != DatabaseBackend::Postgres",
  ".publish_in_tx(txn, tenant_id, actor_id, root_event)",
  "root_event_id",
  "record_projection_revision_in_tx(",
], "composed PostgreSQL boundary");

checkDigestArtifact(digests, paths.digests, (message) => failures.push(message));
assertNotPrePublisher(digests, paths.digests, (message) => failures.push(message));

if (contract) {
  if (contract.task !== "FORUM-23B2G2B3B2"
      || contract.status !== "source_complete_consumer_pending") {
    failures.push(`${paths.contract}: task or status drift`);
  }
  if (contract.event_contract?.rust_family !== "ForumSearchProjectionEvent"
      || contract.event_contract?.transport_family !== "forum_search_projection"
      || contract.event_contract?.event_type !== "forum.search_projection.invalidation_issued"
      || contract.event_contract?.schema_version !== 1
      || contract.event_contract?.owner_revision_minimum !== 1) {
    failures.push(`${paths.contract}: event identity drift`);
  }
  if (JSON.stringify(contract.event_contract?.target_types ?? []) !== JSON.stringify([
    "forum",
    "forum_category",
    "forum_topic",
  ])) {
    failures.push(`${paths.contract}: target type drift`);
  }
  if (contract.postgresql_owner_transaction?.ledger_event_id !== "legacy root envelope id"
      || contract.postgresql_owner_transaction?.typed_envelope_id_is_ledger_identity !== false
      || contract.postgresql_owner_transaction?.legacy_root_required !== true
      || contract.postgresql_owner_transaction?.typed_contract_required !== true) {
    failures.push(`${paths.contract}: owner identity or transaction drift`);
  }
  if (contract.non_postgresql_behavior?.typed_contract_published !== false
      || contract.non_postgresql_behavior?.owner_revision_allocated !== false) {
    failures.push(`${paths.contract}: non-PostgreSQL behavior drift`);
  }
  if (contract.ordering?.owner_revision_compared_numerically_with_ingest_sequence !== false
      || contract.follow_up?.task !== "FORUM-23B2G2B3C") {
    failures.push(`${paths.contract}: ordering or follow-up drift`);
  }
  // `digest_generation.new_values` is the slice's *historical* record of what the
  // generator produced when it landed, not a live value; comparing it to the
  // committed artifact failed for every later event family. What must hold is that
  // the record is well-formed and that the slice states honestly whether the cargo
  // generator actually ran (`false` here means the values were derived by hand and
  // still need the admission workflow).
  checkDigestArtifact(
    { format_version: 1, ...contract.digest_generation?.new_values },
    `${paths.contract} digest_generation.new_values`,
    (message) => failures.push(message),
  );
  if (typeof contract.digest_generation?.cargo_generator_executed !== "boolean") {
    failures.push(`${paths.contract}: digest_generation must state whether the cargo generator ran`);
  }
}

if (wireContract?.task !== "FORUM-23B2G2B3A"
    || causationContract?.task !== "FORUM-23B2G2B3B1") {
  failures.push("publisher predecessor contract drift");
}
requireAll(note, [
  "# FORUM-23B2G2B3B2 versioned invalidation publisher",
  "source_complete_runtime_evidence_pending",
  "forum.search_projection.invalidation_issued",
  "The legacy root envelope ID remains the canonical identity",
  "FORUM-23B2G2B3C",
  "did not execute the Cargo example",
], paths.note);

if (failures.length > 0) {
  console.error("FORUM-23B2G2B3B2 versioned invalidation publisher verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2G2B3B2 versioned invalidation publisher is consistent.");
