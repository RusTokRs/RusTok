#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath ?? "");
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-user-trust-audience-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const adapter = read(contract.adapter_file);
const stateEntity = read(contract.state_entity);
const ownerService = read(contract.owner_service);
const serverComposition = read(contract.server_composition_file);
const membershipAdapter = read(contract.membership_adapter_file);
const runtimePublication = read(contract.runtime_publication_file);
const sqliteProof = read(contract.sqlite_proof);
const note = read(contract.owner_note);
const channelContract = JSON.parse(read(contract.historical_channel_contract) || "{}");
const groupContract = JSON.parse(read(contract.historical_group_contract) || "{}");
const serviceRegistry = read("crates/rustok-forum/src/services/mod.rs");
const crateRoot = read("crates/rustok-forum/src/lib.rs");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26B" ||
  contract.upstream_task !== "FORUM-26A"
) {
  failures.push("trust facts contract must identify FORUM-26B after FORUM-26A");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26B must not claim unexecuted verification evidence");
}

for (const key of [
  "forum_owned_authoritative_state_read",
  "absent_state_defaults_to_zero",
  "exact_tenant_user_actor_context",
  "read_only_port_policy",
  "bounded_request_normalization",
  "membership_delegation_disables_trust",
  "membership_positive_union_short_circuit",
  "confirmed_membership_miss_before_trust",
  "membership_response_identity_validation",
  "missing_membership_provider_fail_closed",
  "storage_error_retryable",
  "storage_corruption_fail_closed",
  "host_runtime_publication",
  "existing_transport_context_reused",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`trust facts contract must record ${key}=true`);
  }
}
for (const key of [
  "trust_derived_from_forum_user_stats",
  "migration_changed",
  "trust_state_write_changed",
  "posting_policy_evaluator_added",
  "automatic_promotion_demotion_added",
  "graphql_rest_transport_changed",
  "public_transport_dto_changed",
  "channel_groups_dependency_changed",
  "rate_limit_changed",
  "external_ai_scoring_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`trust facts contract must keep ${key}=false`);
  }
}
if (
  contract.composition?.trust_level_min !== 0 ||
  contract.composition?.trust_level_max !== 100
) {
  failures.push("trust facts bounds must remain 0..100");
}

for (const marker of [
  "pub struct ForumUserTrustAudienceFactsPort",
  "db: DatabaseConnection",
  "membership_facts: Option<SharedForumAudienceFactsPort>",
  "pub fn new(db: DatabaseConnection)",
  "pub fn with_membership_facts(",
  "pub fn shared(",
  "impl ForumAudienceFactsPort for ForumUserTrustAudienceFactsPort",
  "let request = normalize_request(request)?",
  "validate_context(&context, &request)?",
  "context.require_policy(PortCallPolicy::read())",
  "context.actor.kind != PortActorKind::User",
  "Uuid::parse_str(&context.actor.id).ok() != Some(request.user_id)",
  "include_trust_level: false",
  ".validate_for_request(&membership_request)",
  "!membership_facts.channel_memberships.is_empty()",
  "!membership_facts.group_memberships.is_empty()",
  "Some(self.read_trust_level(&request).await?)",
  "forum_user_trust_state::Entity::find_by_id((request.tenant_id, request.user_id))",
  ".map(|level| level.unwrap_or(0))",
  "state.revision <= 0",
  "trust_level > MAX_FORUM_USER_TRUST_LEVEL",
  "PortError::unavailable(",
  "PortError::invariant_violation(",
]) {
  requireText(adapter, marker, `trust facts adapter is missing ${marker}`);
}

const membershipIndex = adapter.indexOf("resolve_membership_facts(context.clone(), &request)");
const shortCircuitIndex = adapter.indexOf("return Ok(membership_facts)");
const trustReadIndex = adapter.indexOf("Some(self.read_trust_level(&request).await?)");
if (
  membershipIndex < 0 ||
  shortCircuitIndex < 0 ||
  trustReadIndex < 0 ||
  membershipIndex > shortCircuitIndex ||
  shortCircuitIndex > trustReadIndex
) {
  failures.push("membership resolution and positive-union short circuit must precede trust reads");
}

for (const forbidden of [
  "UserStatsService",
  "forum_user_stat::",
  "forum_user_stats::",
  "topic_count",
  "reply_count",
  "solution_count",
  "ActiveModelTrait",
  "ActiveValue::Set",
  ".insert(",
  ".update(",
  ".delete(",
  "TransactionTrait",
  "reqwest",
  "openai",
]) {
  rejectText(adapter, forbidden, `trust facts adapter must not use ${forbidden}`);
}

for (const marker of [
  'table_name = "forum_user_trust_states"',
  "pub trust_level: i16",
  "pub revision: i64",
]) {
  requireText(stateEntity, marker, `authoritative trust state entity is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumUserTrustService",
  "ForumUserTrustChangeKind::ManualOverride",
  "ForumUserTrustService::new",
]) {
  requireText(ownerService, marker, `FORUM-26A owner service is missing ${marker}`);
}
rejectText(
  ownerService,
  "ForumUserTrustAudienceFactsPort",
  "managed trust writes must not depend on the audience facts adapter",
);

for (const marker of [
  'pub mod forum_audience_facts {',
  'include!("forum_audience_facts.rs")',
  "pub(crate) struct ServerForumAudienceFactsPort",
  "membership::ServerForumAudienceFactsPort::shared(db.clone(), groups)",
  "ForumUserTrustAudienceFactsPort::shared(db, membership_facts)",
]) {
  requireText(serverComposition, marker, `server trust composition is missing ${marker}`);
}
for (const marker of [
  "pub(crate) struct ServerForumAudienceFactsPort",
  "channels: SharedChannelReadPort",
  "groups: Option<SharedForumAudienceFactsPort>",
  "ChannelReadSelector::Slug(channel_slug.clone())",
  "self.resolve_groups(context, &request).await?",
]) {
  requireText(membershipAdapter, marker, `historical membership adapter is missing ${marker}`);
}
for (const marker of [
  "ServerForumAudienceFactsPort::shared(",
  "extensions.insert(audience_facts)",
  "extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>()",
]) {
  requireText(runtimePublication, marker, `host runtime publication is missing ${marker}`);
}

for (const marker of [
  "mod user_trust_audience_facts;",
  "pub use user_trust_audience_facts::ForumUserTrustAudienceFactsPort;",
]) {
  requireText(serviceRegistry, marker, `Forum service registry is missing ${marker}`);
}
requireText(
  crateRoot,
  "ForumUserTrustAudienceFactsPort",
  "crate root must publish the trust facts adapter",
);

for (const marker of [
  "absent_authoritative_state_is_zero_and_activity_counters_are_not_trust",
  "managed_authoritative_state_is_published_as_exact_actor_trust",
  "membership_match_short_circuits_trust_storage_and_disables_delegated_trust",
  "confirmed_membership_miss_falls_through_to_authoritative_trust",
  "membership_request_without_provider_and_foreign_actor_fail_closed",
  "assert_eq!(facts.trust_level, Some(0))",
  "assert_eq!(facts.trust_level, Some(42))",
  "assert!(!delegated[0].include_trust_level)",
  "assert_eq!(facts.trust_level, Some(25))",
  "assert_eq!(unavailable.kind, PortErrorKind::Unavailable)",
  "assert_eq!(forbidden.kind, PortErrorKind::Forbidden)",
]) {
  requireText(sqliteProof, marker, `trust facts SQLite proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-26B user trust audience facts",
  "source-ready / unvalidated",
  "Missing trust state resolves to trust level `0`",
  "include_trust_level = false",
  "membership miss",
  "`forum_user_stats` remains an activity-counter projection",
  "no automatic trust promotion/demotion",
  "next bounded FORUM-26 slice",
  "canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not replaced",
  "were not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26B owner note is missing ${marker}`);
}

if (
  channelContract.downstream_trust_task !== "FORUM-26B" ||
  channelContract.composition?.downstream_authoritative_trust_wrapper !== true ||
  channelContract.not_delivered?.includes("Forum trust facts adapter")
) {
  failures.push("historical FORUM-20AT contract must acknowledge FORUM-26B trust composition");
}
if (
  groupContract.downstream_trust_task !== "FORUM-26B" ||
  groupContract.composition?.downstream_authoritative_trust_wrapper !== true ||
  groupContract.not_delivered?.includes("host trust facts adapter")
) {
  failures.push("historical FORUM-20Q contract must acknowledge FORUM-26B trust composition");
}

if (failures.length > 0) {
  console.error("Forum user trust audience facts verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum user trust audience facts contract is source-ready.");
