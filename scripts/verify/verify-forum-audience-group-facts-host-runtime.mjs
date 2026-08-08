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
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
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
  "crates/rustok-forum/contracts/forum-audience-group-facts-host-runtime.json";
const contract = JSON.parse(read(contractPath) || "{}");
const adapter = read(contract.adapter_file ?? "");
const ownerBackedTest = read(contract.owner_backed_test_file ?? "");
const services = read(contract.services_file ?? "");
const runtime = read(contract.runtime_composition_file ?? "");
const forumAudienceOwner = read(contract.forum_audience_owner_file ?? "");
const groupsOwnerPort = read(contract.groups_owner_port_file ?? "");
const groupsOwnerService = read(contract.groups_owner_service_file ?? "");
const notificationSource = read(contract.notification_source_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum audience group facts host contract must use schema_version=1");
}
if (
  contract.task !== "FORUM-20Q" ||
  contract.upstream_task !== "FORUM-20P" ||
  contract.downstream_trust_task !== "FORUM-26B"
) {
  failures.push("forum audience group facts host contract must connect FORUM-20P/Q and downstream FORUM-26B");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("group facts publication must not claim unexecuted evidence");
}

for (const delivered of [
  "feature_guarded_server_adapter",
  "groups_owner_port_reuse",
  "bounded_requested_group_calls",
  "exact_user_actor_validation",
  "owner_response_identity_validation",
  "active_memberships_only",
  "positive_union_short_circuit",
  "unsupported_dimension_retryability",
  "downstream_authoritative_trust_wrapper",
  "runtime_extension_publication",
  "publication_before_notification_source_materialization",
  "notification_source_factory_consumption",
  "inline_contract_tests",
  "owner_backed_sqlite_effective_membership_source",
  "owner_backed_postgres_effective_membership_source",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum audience group facts contract must record ${delivered} as delivered`);
  }
}
if (contract.not_delivered?.includes("host trust facts adapter")) {
  failures.push("FORUM-20Q metadata must not keep the delivered trust adapter open after FORUM-26B");
}
for (const residual of [
  "profile privacy and blocking policy",
  "final notification creation and delivery authorization",
  "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration",
  "executed PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum audience group facts contract must keep ${residual} explicitly open`);
  }
}

const deliveredSlices = [
  "FORUM-20H",
  "FORUM-20I",
  "FORUM-20J",
  "FORUM-20K",
  "FORUM-20L",
  "FORUM-20M",
  "FORUM-20N",
  "FORUM-20O",
  "FORUM-20P",
  "FORUM-20Q",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20Q") {
  failures.push("forum audience group facts contract must require the canonical ledger through FORUM-20Q");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum audience group facts contract must require FORUM-20H through FORUM-20Q delivered sections");
}
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan synchronization must identify FORUM-20G as the historical plan boundary");
  }
  const downstreamSynchronizationRecorded =
    plan.includes("### Delivered in `FORUM-20AM`") &&
    plan.includes("### Delivered in `FORUM-20AU`");
  if (downstreamSynchronizationRecorded) {
    requireText(
      plan,
      "FORUM-20A-AU provide",
      "downstream canonical plan must advance the FORUM-20 ledger through AU",
    );
    requireText(
      plan,
      "### Delivered in `FORUM-20H` through `FORUM-20Q`",
      "downstream canonical plan must retain the consolidated FORUM-20H through FORUM-20Q history",
    );
  } else {
    requireText(
      plan,
      "FORUM-20A-G provide",
      "pending canonical plan synchronization must remain grounded in the historical FORUM-20A-G ledger row",
    );
    for (const slice of deliveredSlices) {
      rejectText(
        plan,
        `### Delivered in \`${slice}\``,
        `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
      );
    }
  }
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-Q provide", "synchronized canonical plan must advance the FORUM-20 ledger through Q");
  for (const slice of deliveredSlices) {
    requireText(
      plan,
      `### Delivered in \`${slice}\``,
      `synchronized canonical plan is missing the delivered ${slice} section`,
    );
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub(crate) struct ServerForumAudienceGroupFactsPort",
  "groups: SharedGroupMembershipEnforcementReadPort",
  "impl ForumAudienceFactsPort for ServerForumAudienceGroupFactsPort",
  "context.require_policy(PortCallPolicy::read())",
  "request.group_ids.len() > MAX_FORUM_AUDIENCE_GROUPS",
  "context.actor.kind != PortActorKind::User",
  "Uuid::parse_str(&context.actor.id).ok() != Some(request.user_id)",
  "for group_id in &request.group_ids",
  ".read_membership_enforcement(",
  "validate_owner_state(&request, *group_id, &state)",
  "if state.active_member",
  "group_memberships.push(*group_id)",
  "request.include_trust_level || !request.channel_slugs.is_empty()",
  "return Err(partial_provider_unavailable())",
  "PortError::unavailable(",
  "state.tenant_id != request.tenant_id",
  "state.group_id != group_id",
  "state.user_id != request.user_id",
  "GroupMembershipEnforcementService::new(db)",
  "mod owner_backed_tests;",
]) {
  requireText(adapter, marker, `forum group facts adapter is missing ${marker}`);
}

for (const forbidden of [
  "rustok_groups::entities",
  "membership_state::Entity",
  "group_membership::Entity",
  "membership_enforcement_state::Entity",
  "forum_group_memberships",
  "EntityTrait",
  "QueryFilter",
  "ColumnTrait",
  "SELECT ",
]) {
  rejectText(adapter, forbidden, `forum group facts adapter must reuse the Groups owner instead of ${forbidden}`);
}

for (const marker of [
  "#[cfg(feature = \"mod-forum\")]\npub mod forum_audience_facts {",
  "membership::ServerForumAudienceFactsPort::shared(db.clone(), groups)",
  "ForumUserTrustAudienceFactsPort::shared(db, membership_facts)",
  "#[cfg(all(feature = \"mod-forum\", feature = \"mod-groups\"))]",
  "pub mod forum_audience_group_facts;",
]) {
  requireText(services, marker, `server services surface is missing ${marker}`);
}
for (const marker of [
  "ServerForumAudienceGroupFactsPort::shared(",
  "ServerForumAudienceFactsPort::shared(",
  "extensions.insert(audience_facts)",
  "extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>()",
]) {
  requireText(runtime, marker, `server runtime group facts composition is missing ${marker}`);
}
const publicationIndex = runtime.indexOf("extensions.insert(audience_facts)");
const materializationIndex = runtime.indexOf(
  "materialize_notification_source_registry(&mut extensions, &host)",
);
if (publicationIndex < 0 || materializationIndex < 0 || publicationIndex > materializationIndex) {
  failures.push("group audience facts capability must be published before notification source materialization");
}

for (const marker of [
  "pub trait GroupMembershipEnforcementReadPort: Send + Sync",
  "async fn read_membership_enforcement(",
  "pub type SharedGroupMembershipEnforcementReadPort",
]) {
  requireText(groupsOwnerPort, marker, `Groups owner port is missing ${marker}`);
}
for (const marker of [
  "pub struct GroupMembershipEnforcementService",
  "pub fn new(db: DatabaseConnection) -> Self",
  "impl GroupMembershipEnforcementReadPort for GroupMembershipEnforcementService",
  "read_effective_state_owned(",
]) {
  requireText(groupsOwnerService, marker, `Groups owner service is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumAudienceFactsResolver",
  ".resolve_forum_audience_facts(context, request.clone())",
  "pub struct ForumAudienceEvaluator",
  "constraints.group_members_any",
]) {
  requireText(forumAudienceOwner, marker, `Forum audience owner is missing ${marker}`);
}
requireText(
  notificationSource,
  "host.shared_get::<SharedForumAudienceFactsPort>()",
  "Forum notification source factory must consume the published audience facts capability",
);

for (const marker of [
  "group_facts_resolve_only_requested_active_memberships",
  "active_group_match_short_circuits_unsupported_positive_dimensions",
  "unsupported_dimensions_are_retryable_when_groups_do_not_decide",
  "foreign_user_context_is_rejected_before_owner_calls",
  "assert_eq!(facts.group_memberships, vec![active])",
  "assert_eq!(error.kind, PortErrorKind::Unavailable)",
  "assert!(error.retryable)",
]) {
  requireText(adapter, marker, `forum group facts inline contract test is missing ${marker}`);
}

for (const marker of [
  "rustok_groups::migrations::migrations()",
  "ServerForumAudienceGroupFactsPort::from_db(db.clone())",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "GroupMembershipEffectiveStatus::Suspended",
  "forum_group_facts_follow_groups_owner_clock_sqlite",
  "forum_group_facts_follow_groups_owner_clock_postgres",
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "options=-csearch_path%3D",
  'assert_eq!(stored_status_during_suspension, "active")',
  "facts_after_suspend.group_memberships.is_empty()",
  "facts_after_expiry.group_memberships, vec![group_id]",
  "tokio::time::sleep",
  "revision_after_expiry, suspended_revision",
  "group_member_count",
]) {
  requireText(ownerBackedTest, marker, `owner-backed Forum Groups audience evidence is missing ${marker}`);
}
for (const forbidden of [
  "UPDATE group_membership_enforcements",
  "DELETE FROM group_membership_enforcements",
  "GroupMembershipEffectiveState {",
  "rustok_moderation::",
]) {
  rejectText(ownerBackedTest, forbidden, `owner-backed Forum Groups audience evidence contains shortcut ${forbidden}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20P" ||
  upstream.upstream_task !== "FORUM-20O" ||
  upstream.composition?.recipient_specific_topic_visibility !== true ||
  upstream.composition?.notification_sparse_page_prerequisite !== true
) {
  failures.push("FORUM-20Q group facts adapter must remain grounded in the FORUM-20P notification consumer");
}

for (const marker of [
  "## `FORUM-20` — ACL and visibility inheritance",
  "notifications, search, SEO and deep links must call the same",
]) {
  requireText(plan, marker, `canonical Forum plan is missing the visibility boundary ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum audience group facts host runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Historical FORUM-20Q Groups facts contract remains valid with owner-backed SQLite/PostgreSQL evidence source.");
