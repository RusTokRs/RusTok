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
  "crates/rustok-forum/contracts/forum-audience-channel-facts-host-runtime.json";
const contract = JSON.parse(read(contractPath) || "{}");
const adapter = read(contract.adapter_file ?? "");
const historicalGroups = read(contract.historical_group_adapter_file ?? "");
const services = read(contract.services_file ?? "");
const runtime = read(contract.runtime_composition_file ?? "");
const forumAudience = read(contract.forum_audience_owner_file ?? "");
const channelOwner = read(contract.channel_owner_port_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AT" ||
  contract.upstream_task !== "FORUM-20AS"
) {
  failures.push("channel facts contract must connect FORUM-20AS/20AT");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20AT must not claim unexecuted verification evidence");
}

for (const key of [
  "forum_build_publication",
  "channel_owner_port_reuse",
  "exact_requested_current_channel",
  "tenant_active_channel_validation",
  "no_channel_discovery",
  "channel_first_positive_union",
  "channel_only_without_groups",
  "optional_historical_groups_fallback",
  "trust_fail_closed",
  "runtime_extension_publication",
  "inline_contract_tests",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`FORUM-20AT contract must record ${key} as delivered`);
  }
}
for (const key of [
  "migration_changed",
  "public_dto_changed",
  "forum_to_channel_dependency_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`FORUM-20AT contract must keep ${key}=false`);
  }
}
for (const residual of [
  "Forum trust facts adapter",
  "reply and moderation audience policies",
  "remaining Forum read search index SEO and deep-link audience migration",
  "PostgreSQL concurrency and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`FORUM-20AT must keep ${residual} explicitly open`);
  }
}

for (const marker of [
  "pub(crate) struct ServerForumAudienceFactsPort",
  "channels: SharedChannelReadPort",
  "groups: Option<SharedForumAudienceFactsPort>",
  "impl ForumAudienceFactsPort for ServerForumAudienceFactsPort",
  "let request = normalize_request(request)?",
  "validate_context(&context, &request)?",
  "context.channel",
  ".binary_search(&channel_slug)",
  "ChannelReadSelector::Slug(channel_slug.clone())",
  "include_inactive: false",
  "validate_channel_projection(request, &channel_slug, &projection)",
  "if !channel_memberships.is_empty()",
  "self.resolve_groups(context, &request).await?",
  "if !group_memberships.is_empty()",
  "Err(partial_provider_unavailable())",
  "channel.tenant_id != request.tenant_id",
  "channel.slug.trim().to_lowercase() != requested_slug",
  "|| !channel.is_active",
]) {
  requireText(adapter, marker, `channel facts adapter is missing ${marker}`);
}

for (const forbidden of [
  "rustok_channel::entities",
  "rustok_groups::entities",
  "channel::Entity",
  "group_membership::Entity",
  "EntityTrait",
  "QueryFilter",
  "ColumnTrait",
  "SELECT ",
]) {
  rejectText(adapter, forbidden, `channel facts adapter must not use ${forbidden}`);
}

for (const marker of [
  "requested_active_current_channel_is_confirmed_without_group_calls",
  "unrequested_current_channel_is_not_discovered_through_owner_reads",
  "groups_are_consulted_only_after_channel_miss",
  "unresolved_trust_or_missing_optional_groups_remain_retryable",
  "assert_eq!(facts.channel_memberships, vec![\"members\".to_string()])",
  "assert_eq!(error.kind, PortErrorKind::Unavailable)",
  "assert!(error.retryable)",
]) {
  requireText(adapter, marker, `FORUM-20AT inline contract test is missing ${marker}`);
}

for (const marker of [
  "#[cfg(feature = \"mod-forum\")]\npub mod forum_audience_facts;",
  "#[cfg(all(feature = \"mod-forum\", feature = \"mod-groups\"))]\npub mod forum_audience_group_facts;",
]) {
  requireText(services, marker, `server services surface is missing ${marker}`);
}
for (const marker of [
  "ServerForumAudienceGroupFactsPort::shared(",
  "ServerForumAudienceFactsPort::shared(",
  "#[cfg(not(feature = \"mod-groups\"))]\n        let groups = None;",
  "extensions.insert(audience_facts)",
  "extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>()",
]) {
  requireText(runtime, marker, `server runtime composition is missing ${marker}`);
}
const publicationIndex = runtime.indexOf("extensions.insert(audience_facts)");
const materializationIndex = runtime.indexOf(
  "materialize_notification_source_registry(&mut extensions, &host)",
);
if (publicationIndex < 0 || materializationIndex < 0 || publicationIndex > materializationIndex) {
  failures.push("Forum audience facts must be published before notification source materialization");
}

for (const marker of [
  "pub trait ChannelReadPort: Send + Sync",
  "ChannelReadSelector::Slug(slug)",
  "if !request.include_inactive && !detail.channel.is_active",
  "ensure_tenant_scope(tenant_id, &detail)?",
]) {
  requireText(channelOwner, marker, `Channel owner read contract is missing ${marker}`);
}
for (const marker of [
  "pub(crate) struct ServerForumAudienceGroupFactsPort",
  "for group_id in &request.group_ids",
  ".read_membership_enforcement(",
  "validate_owner_state(&request, *group_id, &state)",
  "if state.active_member",
]) {
  requireText(historicalGroups, marker, `historical Groups adapter is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumAudienceFactsResolver",
  "pub struct ForumAudienceEvaluator",
  "constraints.channel_members_any",
  "constraints.group_members_any",
]) {
  requireText(forumAudience, marker, `Forum audience owner is missing ${marker}`);
}

for (const marker of [
  "FORUM-20A-AU provide",
  "### Delivered in `FORUM-20AT`",
  "### Delivered in `FORUM-20AU`",
  "implement Forum trust owner state under `FORUM-26`",
  "verify-forum-audience-channel-facts-host-runtime.mjs",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum audience Channel facts host runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Historical FORUM-20AT Channel facts contract remains valid through FORUM-20AU.");
