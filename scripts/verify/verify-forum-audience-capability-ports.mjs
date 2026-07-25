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

function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-audience-capability-ports.json";
const contract = JSON.parse(read(contractPath) || "{}");
const audience = read(contract.contract_file ?? "");
const crate = read(contract.crate_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum audience capability contract must use schema_version=1");
}
if (contract.task !== "FORUM-20F") {
  failures.push("forum audience capability contract must belong to FORUM-20F");
}
for (const [key, expected] of Object.entries({
  roles: 4,
  channels: 32,
  groups: 32,
  explicit_allow_users: 100,
  explicit_deny_users: 100,
  maximum_trust_level: 100,
  channel_slug_characters: 128,
})) {
  if (contract.limits?.[key] !== expected) {
    failures.push(`forum audience capability limit ${key} must remain ${expected}`);
  }
}
if (contract.policy?.positive_selectors !== "union") {
  failures.push("forum audience positive selectors must remain a documented union");
}
if (contract.policy?.owner_facts !== "exact requested tenant actor and candidate subset only") {
  failures.push("forum audience owner facts must remain exact by tenant, actor, and candidates");
}
if (
  contract.port?.echoes_tenant_and_actor !== true ||
  contract.port?.validates_context_identity !== true
) {
  failures.push("forum audience port must echo and validate exact tenant/actor identity");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted audience evidence");
}
for (const residual of [
  "audience policy persistence and inheritance",
  "category topic and reply read composition",
  "create reply and moderate audience write policy",
  "topic narrowing commands",
  "channel and group provider adapters",
  "trust-level owner provider",
  "visibility-scoped category and all-read mutations",
  "notification search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum audience capability contract must keep ${residual} open`);
  }
}

for (const marker of [
  "pub const MAX_FORUM_AUDIENCE_ROLES: usize = 4",
  "pub const MAX_FORUM_AUDIENCE_CHANNELS: usize = 32",
  "pub const MAX_FORUM_AUDIENCE_GROUPS: usize = 32",
  "pub const MAX_FORUM_AUDIENCE_EXPLICIT_USERS: usize = 100",
  "pub const MAX_FORUM_AUDIENCE_TRUST_LEVEL: u8 = 100",
  "const MAX_FORUM_AUDIENCE_CHANNEL_SLUG_LEN: usize = 128",
  "pub struct ForumAudienceConstraints",
  "pub roles_any: Vec<UserRole>",
  "pub minimum_trust_level: Option<u8>",
  "pub channel_members_any: Vec<String>",
  "pub group_members_any: Vec<Uuid>",
  "pub allow_user_ids: Vec<Uuid>",
  "pub deny_user_ids: Vec<Uuid>",
  "pub fn normalize(mut self) -> ForumResult<Self>",
  "pub struct ForumAudienceFactsRequest",
  "pub struct ForumAudienceFacts",
  "pub trait ForumAudienceFactsPort: Send + Sync",
  "pub type SharedForumAudienceFactsPort = Arc<dyn ForumAudienceFactsPort>",
  "pub struct ForumAudienceFactsResolver",
  "pub struct ForumAudienceEvaluator",
  "pub enum ForumAudienceDecisionReason",
]) {
  requireText(audience, marker, `forum audience contract is missing ${marker}`);
}

const firstRoleBound = audience.indexOf("self.roles_any.len()");
const firstRoleDedup = audience.indexOf("self.roles_any.dedup()");
const firstChannelBound = audience.indexOf("self.channel_members_any.len()");
const firstChannelNormalize = audience.indexOf(
  "self.channel_members_any = normalize_channel_slugs",
);
if (
  firstRoleBound < 0 ||
  firstRoleDedup < 0 ||
  firstRoleBound > firstRoleDedup ||
  firstChannelBound < 0 ||
  firstChannelNormalize < 0 ||
  firstChannelBound > firstChannelNormalize
) {
  failures.push("raw audience candidates must be bounded before deduplication or normalization");
}

for (const marker of [
  "Providers must resolve only the requested tenant, actor and candidate",
  "pub tenant_id: Uuid",
  "pub user_id: Uuid",
  "validate_identity(self.tenant_id, \"fact request tenant\")?",
  "validate_identity(self.user_id, \"fact request user\")?",
  "Forum audience facts returned a different tenant or actor",
  "Forum audience facts returned an unrequested trust level",
  "Forum audience facts returned an unrequested channel membership",
  "Forum audience facts returned an unrequested group membership",
  ".validate_for_request(&request)",
]) {
  requireText(audience, marker, `exact owner-facts contract is missing ${marker}`);
}

for (const marker of [
  "fn validate_port_context(",
  "context_tenant_id != tenant_id",
  "context.actor.kind != PortActorKind::User",
  "context_user_id != user_id",
  "port context tenant does not match",
  "port context actor does not match",
  ".require_policy(PortCallPolicy::read())",
  "FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE",
  "ForumError::capability_unavailable(",
  "ForumError::capability_failure(",
  "constraints.roles_any.contains(&security.role)",
  "security.is_public_read()",
]) {
  requireText(audience, marker, `fail-closed audience resolver is missing ${marker}`);
}

for (const marker of [
  "must not be nil",
  "must not contain nil identifiers",
  "ids.iter().any(Uuid::is_nil)",
]) {
  requireText(audience, marker, `nil audience identity guard is missing ${marker}`);
}

const evaluator = between(
  audience,
  "impl ForumAudienceEvaluator {",
  "fn validate_raw_len(",
  "forum audience evaluator",
);
const denyIndex = evaluator.indexOf("ForumAudienceDecisionReason::ExplicitDeny");
const allowIndex = evaluator.indexOf("ForumAudienceDecisionReason::ExplicitAllow");
const roleIndex = evaluator.indexOf("ForumAudienceDecisionReason::Role");
const trustIndex = evaluator.indexOf("ForumAudienceDecisionReason::TrustLevel");
const channelIndex = evaluator.indexOf("ForumAudienceDecisionReason::ChannelMembership");
const groupIndex = evaluator.indexOf("ForumAudienceDecisionReason::GroupMembership");
if (
  denyIndex < 0 ||
  allowIndex < 0 ||
  roleIndex < 0 ||
  trustIndex < 0 ||
  channelIndex < 0 ||
  groupIndex < 0 ||
  denyIndex > allowIndex ||
  allowIndex > roleIndex ||
  roleIndex > trustIndex ||
  trustIndex > channelIndex ||
  channelIndex > groupIndex
) {
  failures.push("audience evaluator precedence must remain deny, allow, local role, trust, channel, group");
}

for (const forbidden of [
  "rustok_groups",
  "rustok_channel",
  "forum_group",
  "forum_channel_member",
  "forum_user_stats::Entity",
  "serde_json::Value",
]) {
  rejectText(audience, forbidden, `forum audience contract must not depend on ${forbidden}`);
}

for (const marker of [
  "pub mod audience;",
  "ForumAudienceConstraints",
  "ForumAudienceFactsPort",
  "ForumAudienceFactsResolver",
  "ForumAudienceEvaluator",
]) {
  requireText(crate, marker, `crate export is missing ${marker}`);
}

for (const marker of [
  "audience_constraints_are_bounded_and_canonical",
  "explicit_deny_wins_and_positive_selectors_are_a_union",
  "exact_owner_facts_reject_wrong_identity_and_unrequested_memberships",
  "resolver_is_fail_closed_and_requires_matching_read_context",
  "FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE",
  "port.deadline_required",
  "actor does not match",
  "different tenant or actor",
]) {
  requireText(testSource, marker, `audience capability test is missing ${marker}`);
}

for (const marker of [
  "Delivered in `FORUM-20F`",
  "ForumAudienceFactsPort",
  "forum-audience-capability-ports.json",
  "audience_capability_contract",
  "verify-forum-audience-capability-ports.mjs",
]) {
  requireText(plan, marker, `canonical FORUM-20 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum audience capability verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum audience capability port contract is source-ready.");
