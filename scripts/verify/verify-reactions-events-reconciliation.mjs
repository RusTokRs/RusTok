import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-reactions/contracts/reactions-events-reconciliation.json";

function fail(message) {
  throw new Error(`Reactions events/reconciliation verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

function normalized(value) {
  return value.replace(/\s+/gu, " ");
}

function section(source, start, end) {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) fail(`missing section start ${start}`);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) fail(`missing section end ${end}`);
  return source.slice(startIndex, endIndex);
}

const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 1) fail("unsupported contract schema version");
if (contract.contract !== "reactions_events_reconciliation_v1") {
  fail("unexpected contract identity");
}
if (contract.module !== "reactions") fail("owner module must be reactions");
if (contract.status !== "source_ready_maintainer_execution_pending") {
  fail("source contract must not claim retained execution");
}
for (const requiredFile of contract.required_files) read(requiredFile);

const eventsCargo = read("crates/rustok-events/Cargo.toml");
const eventContract = read("crates/rustok-events/src/contract.rs");
const eventLib = read("crates/rustok-events/src/lib.rs");
const eventFamily = read("crates/rustok-events/src/reactions.rs");
const eventTests = read("crates/rustok-events/tests/reactions_contracts.rs");
const eventApi = read("crates/rustok-events/CRATE_API.md");
const ownerCargo = read("crates/rustok-reactions/Cargo.toml");
const ownerLib = read("crates/rustok-reactions/src/lib.rs");
const service = read("crates/rustok-reactions/src/service.rs");
const reconciliation = read("crates/rustok-reactions/src/reconciliation.rs");
const reactionsPlan = normalized(
  read("crates/rustok-reactions/docs/implementation-plan.md"),
);
const forumPlan = normalized(
  read("crates/rustok-forum/docs/implementation-plan.md"),
);

if (!ownerCargo.includes("rustok-events.workspace = true")) {
  fail("Reactions owner must depend on the central typed event contract crate");
}
if (eventsCargo.includes("rustok-reactions")) {
  fail("rustok-events must not depend on the Reactions owner crate");
}

for (const eventType of contract.event_family.types) {
  if (!eventFamily.includes(`"${eventType}"`)) {
    fail(`typed event family is missing ${eventType}`);
  }
}
for (const fragment of [
  "pub enum ReactionsEvent",
  "ActorStateChanged",
  "SubjectReconciled",
  "impl EventContract for ReactionsEvent",
  "impl ValidateEvent for ReactionsEvent",
  "MAX_REACTIONS_EVENT_KEYS",
  "added keys must be selected and disjoint from removed keys",
  "a reconciliation event must contain a bounded changed-key sample",
]) {
  if (!eventFamily.includes(fragment)) fail(`event family is missing ${fragment}`);
}
for (const fragment of [
  "Reactions(ReactionsEvent)",
  "Self::Reactions(event) => event.event_type()",
  "Self::Reactions(event) => event.schema_version()",
  "Self::Reactions(event) => event.validate()",
]) {
  if (!eventContract.includes(fragment)) fail(`closed payload family is missing ${fragment}`);
}
for (const fragment of [
  "mod reactions;",
  "REACTIONS_EVENT_SCHEMAS",
  "reactions_event_schema(event_type)",
  ".chain(REACTIONS_EVENT_SCHEMAS.iter())",
]) {
  if (!eventLib.includes(fragment)) fail(`event registry is missing ${fragment}`);
}
for (const fragment of [
  "reactions_family_registers_both_schema_v1_contracts",
  "actor_state_change_is_typed_validated_and_enveloped",
  "actor_state_change_rejects_noop_and_overlapping_deltas",
  "reconciled_event_requires_truthful_bounded_sample",
]) {
  if (!eventTests.includes(fragment)) fail(`event tests are missing ${fragment}`);
}
for (const fragment of [
  "ReactionsEvent",
  "reactions.actor_state.changed",
  "reactions.subject.reconciled",
  "owner-operation UUID",
]) {
  if (!eventApi.includes(fragment)) fail(`event CRATE_API is missing ${fragment}`);
}

for (const fragment of [
  "lease.operation_id",
  "publish_actor_state_changed(",
  "TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(",
  "ReactionsEvent::ActorStateChanged",
  "if changed {",
  "idempotency::complete(&transaction, lease, &receipt)",
  "transaction.commit()",
  "reactions.event_identity_conflict",
  "reactions.event_unavailable",
]) {
  if (!service.includes(fragment)) fail(`transactional command path is missing ${fragment}`);
}
const executeApply = section(
  service,
  "async fn execute_apply(",
  "#[async_trait]\nimpl ReactionReadPort",
);
const applyBody = section(
  service,
  "async fn apply_inside_transaction(",
  "async fn publish_actor_state_changed(",
);
const applyCallIndex = executeApply.indexOf("apply_inside_transaction(");
const completionIndex = executeApply.indexOf(
  "idempotency::complete(&transaction, lease, &receipt)",
);
const commitIndex = executeApply.indexOf("transaction.commit()");
if (!(applyCallIndex >= 0 && applyCallIndex < completionIndex && completionIndex < commitIndex)) {
  fail("command mutation/event, receipt completion and commit ordering is not explicit");
}
const mutationIndex = applyBody.indexOf("persist_actor_state(");
const aggregateIndex = applyBody.indexOf("apply_aggregate_delta(");
const eventIndex = applyBody.indexOf("publish_actor_state_changed(");
if (!(mutationIndex >= 0 && mutationIndex < aggregateIndex && aggregateIndex < eventIndex)) {
  fail("actor state, aggregate and event ordering is not explicit");
}
if (service.includes("ContractEventEnvelope::new(")) {
  fail("command path must not generate a second random event identity");
}

for (const fragment of [
  "MAX_REACTION_RECONCILIATION_ACTOR_STATES: u32 = 1_000",
  "MAX_REACTION_RECONCILIATION_ISSUES: usize = 64",
  "MAX_REACTION_RECONCILIATION_AGGREGATE_ROWS: u64 = 128",
  'const RECONCILE_REACTIONS_CLAIM: &str = "reactions:reconcile"',
  "pub async fn inspect_reconciliation",
  "PortCallPolicy::read()",
  "pub async fn repair_reconciliation",
  "PortCallPolicy::write()",
  "reactions.reconciliation_command_idempotency_mismatch",
  "idempotency::admit(",
  "self.execute_repair(lease, actor_id, &command)",
  "serialize_subject(transaction",
  "ReactionReconciliationStatus::Blocked",
  "reaction aggregate repair is blocked by catalog or actor-state corruption",
  "aggregate::Entity::delete_many()",
  "ReactionsEvent::SubjectReconciled",
  "idempotency::complete(&transaction, lease, &receipt)",
  "transaction.commit()",
  "row.revision <= 0",
  "actor_state_duplicate_keys",
  "actor_selection_limit_exceeded",
  "actor_selection_outside_catalog",
  "aggregate_count_mismatch",
  "issues.len() < MAX_REACTION_RECONCILIATION_ISSUES",
]) {
  if (!reconciliation.includes(fragment)) {
    fail(`bounded reconciliation is missing ${fragment}`);
  }
}
const repairEntry = section(
  reconciliation,
  "pub async fn repair_reconciliation(",
  "async fn execute_repair(",
);
if (!repairEntry.includes("self.execute_repair(lease, actor_id, &command).await")) {
  fail("failure receipt boundary must call the transaction-scoped repair helper");
}
if (!repairEntry.includes("idempotency::fail(self.database(), lease, error)")) {
  fail("repair failure receipt must be persisted after the helper returns");
}
if (!ownerLib.includes("mod reconciliation;")) {
  fail("owner crate does not compile the reconciliation module");
}
for (const symbol of [
  "ReactionReconciliationRequest",
  "ReactionReconciliationReport",
  "ReactionReconciliationReceipt",
  "RepairReactionSubjectCommand",
]) {
  if (!ownerLib.includes(symbol)) fail(`owner crate does not export ${symbol}`);
}

for (const forbidden of [
  "rustok_forum",
  "rustok_blog",
  "rustok_profiles",
  "async_graphql",
  "axum::",
  "leptos::",
]) {
  if (service.includes(forbidden) || reconciliation.includes(forbidden)) {
    fail(`owner event/reconciliation path imports forbidden dependency ${forbidden}`);
  }
}
for (const forbidden of [
  "visibility",
  "display_name",
  "profile_handle",
  "locale",
  "channel",
  "claims",
  "roles",
]) {
  if (eventFamily.includes(`${forbidden}:`)) {
    fail(`semantic event payload exposes forbidden field ${forbidden}`);
  }
}

for (const fragment of [
  "typed semantic events and completed shared Outbox receipts now share one transaction",
  "Bounded inspect/repair reconciliation is source-ready",
  "Idempotent no-op commands and completed receipt replays do not publish another event",
  "reactions:reconcile",
  "event_contract_digests",
  "verify-reactions-events-reconciliation.mjs",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}
for (const fragment of [
  "| `FORUM-18` | `in_progress` |",
  "semantic reaction events",
  "bounded aggregate reconciliation",
  "Existing Forum votes remain Forum semantics",
  "verify-reactions-events-reconciliation.mjs",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}

console.log("Reactions transactional events and bounded reconciliation verified.");
