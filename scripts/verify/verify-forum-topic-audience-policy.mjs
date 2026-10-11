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

// Markers may be strings or regular expressions: a bare `channel::` substring also occurs inside
// Forum's own `forum_topic_audience_channel::Entity`, which is not a dependency on the Channel module.
function matchesMarker(source, marker) {
  return marker instanceof RegExp ? marker.test(source) : source.includes(marker);
}

function requireText(source, marker, message) {
  if (!matchesMarker(source, marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (matchesMarker(source, marker)) failures.push(message);
}

/**
 * Every block in `source` that starts at `signature` and ends at its matching closing brace. The
 * previous signature-to-sibling-function slicing broke whenever a function moved into the seam file
 * or its neighbour was renamed, which is not a contract change.
 */
function blocksFor(source, signature, label) {
  const blocks = [];
  let from = 0;
  for (;;) {
    const start = source.indexOf(signature, from);
    if (start < 0) break;
    const open = source.indexOf("{", start);
    if (open < 0) break;
    let depth = 0;
    let end = -1;
    for (let index = open; index < source.length; index += 1) {
      if (source[index] === "{") depth += 1;
      else if (source[index] === "}") {
        depth -= 1;
        if (depth === 0) {
          end = index;
          break;
        }
      }
    }
    if (end < 0) break;
    blocks.push(source.slice(start, end + 1));
    from = end + 1;
  }
  if (blocks.length === 0) failures.push(`${label}: unable to isolate source block`);
  return blocks;
}

function requireInAny(blocks, markers, message) {
  if (!blocks.some((block) => markers.every((marker) => block.includes(marker)))) {
    failures.push(message);
  }
}

const contractPath =
  "crates/modules/rustok-forum/contracts/forum-topic-audience-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
// `services/mod.rs` composes the topic audience module from the shared helpers and the thin owner
// seam (`include!` of `topic_audience.rs` + `topic_audience_owner.rs`), so the owner contract is
// checked across both files instead of assuming one file per module.
const ownerParts = [
  read(contract.owner_file ?? ""),
  read(contract.owner_seam_file ?? contract.owner_file ?? ""),
].filter((value, index, all) => value.length > 0 && all.indexOf(value) === index);
const owner = ownerParts.join("\n");
const categoryOwner = read(contract.category_owner_file ?? "");
const audience = read(contract.audience_contract_file ?? "");
const migration = read(contract.migration_file ?? "");
const migrations = read(contract.migration_registry_file ?? "");
const entities = read(contract.entities_file ?? "");
const services = read(contract.services_file ?? "");
const crate = read(contract.crate_file ?? "");
const testSource = read(contract.test_file ?? "");

if (contract.schema_version !== 1) {
  failures.push("topic audience policy contract must use schema_version=1");
}
if (contract.task !== "FORUM-20H") {
  failures.push("topic audience policy contract must belong to FORUM-20H");
}
for (const [key, expected] of Object.entries({
  category_tree_nodes: 512,
  category_tree_depth: 16,
  roles_per_layer: 4,
  channels_per_layer: 32,
  groups_per_layer: 32,
  allow_users_per_layer: 100,
  deny_users_per_layer: 100,
  maximum_trust_level: 100,
})) {
  if (contract.bounds?.[key] !== expected) {
    failures.push(`topic audience bound ${key} must remain ${expected}`);
  }
}
if (
  contract.policy?.effective_order !==
  "root category layers then descendant category layers then optional topic layer"
) {
  failures.push("topic audience order must remain category ancestry followed by topic");
}
if (contract.policy?.effective_composition !== "conjunction of every non-empty layer") {
  failures.push("topic audience layers must remain conjunctive");
}
if (contract.policy?.storage !== "normalized typed tables without JSON") {
  failures.push("topic audience persistence must remain normalized and typed");
}
if (
  contract.policy?.category_snapshot_owner !==
  "category audience owner is the single bounded inheritance resolver"
) {
  failures.push("topic policy must reuse the category audience snapshot owner");
}
if (
  contract.composition?.category_storage !== true ||
  contract.composition?.category_inheritance !== true ||
  contract.composition?.topic_narrowing_storage !== true
) {
  failures.push("topic audience contract must record category and topic persistence");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted topic audience evidence");
}
for (const residual of [
  "category topic and reply read composition",
  "create reply and moderate audience write policy",
  "channel and group provider adapters",
  "trust-level owner provider",
  "visibility-scoped category and all-read mutations",
  "notification search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`topic audience contract must keep ${residual} open`);
  }
}
if (contract.not_delivered?.includes("topic narrowing persistence and commands")) {
  failures.push("delivered topic narrowing must not remain open");
}

for (const table of contract.tables ?? []) {
  requireText(migration, table, `topic audience migration is missing ${table}`);
}
for (const marker of [
  "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_topics_tenant_id",
  "FOREIGN KEY (tenant_id, topic_id)",
  "REFERENCES forum_topics (tenant_id, id)",
  "REFERENCES forum_topic_audience_policies (tenant_id, topic_id)",
  "minimum_trust_level BETWEEN 0 AND 100",
  "role IN ('super_admin', 'admin', 'manager', 'customer')",
  "effect IN ('allow', 'deny')",
  "forum_validate_topic_audience_channel_insert",
  "forum_validate_topic_audience_group_insert",
  "forum_validate_topic_audience_user_insert",
  "forum_reject_topic_audience_relation_update",
  "forum topic audience channels exceed bounded limit",
  "forum topic audience groups exceed bounded limit",
  "forum topic audience users exceed bounded limit",
  "BEFORE UPDATE ON forum_topic_audience_policies",
]) {
  requireText(migration, marker, `topic audience migration is missing ${marker}`);
}
for (const forbidden of ["JSONB", "jsonb", "serde_json", "metadata"]) {
  rejectText(migration, forbidden, `topic audience migration must not use ${forbidden}`);
}
requireText(
  migrations,
  "m20260725_000002_add_forum_topic_audience_policy",
  "Forum migration registry must include the topic audience migration",
);

for (const marker of [
  "pub mod forum_topic_audience_channel;",
  "pub mod forum_topic_audience_group;",
  "pub mod forum_topic_audience_policy;",
  "pub mod forum_topic_audience_role;",
  "pub mod forum_topic_audience_user;",
]) {
  requireText(entities, marker, `topic audience entity registry is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumTopicAudiencePolicy",
  "pub struct SetForumTopicAudiencePolicyInput",
  "pub struct ForumTopicAudiencePolicyService",
  "pub async fn get(",
  "pub async fn set(",
  "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?",
  "lock_category_tree_in_tx(&txn, tenant_id).await?",
  "lock_topic_audience_in_tx(&txn, tenant_id, topic_id).await?",
  "load_category_audience_policy(&txn, tenant_id, topic.category_id).await?",
  "forum_topic_audience_policy::Entity::delete_many()",
  "insert_roles(&txn, tenant_id, topic_id, &constraints).await?",
  "insert_channels(&txn, tenant_id, topic_id, &constraints).await?",
  "insert_groups(&txn, tenant_id, topic_id, &constraints).await?",
  "insert_users(&txn, tenant_id, topic_id, &constraints).await?",
  "inherited_category_layers",
  "Every layer remains independently required.",
  "configured_constraints",
  "load_topic_layer",
  "Forum topic audience storage contains an empty local layer",
]) {
  requireText(owner, marker, `topic audience owner is missing ${marker}`);
}
for (const marker of [
  "pub(crate) async fn load_category_audience_policy",
  "pub(crate) async fn lock_category_tree_in_tx",
  "MAX_FORUM_CATEGORY_TREE_NODES + 1",
  "MAX_FORUM_CATEGORY_TREE_DEPTH",
  "Forum category audience storage contains an empty local layer",
]) {
  requireText(categoryOwner, marker, `category snapshot owner is missing ${marker}`);
}
// `lock_topic_audience_in_tx` never existed: the seam takes the category tree lock, then the active
// topic write lock and the scope lock, and only then reads the category snapshot. Asserting both
// topic locks is strictly stronger than the single name the guard asked for.
const topicAudienceOrder = [
  "lock_category_tree_in_tx",
  "lock_active_topic_audience_write_in_tx",
  "lock_topic_audience_scopes_in_tx",
  "load_category_audience_policy",
  "forum_topic_audience_policy::Entity::delete_many()",
  "forum_topic_audience_policy::ActiveModel",
  "txn.commit().await?",
];
// The order must hold inside one `set` implementation, whichever file the seam composes it from.
if (
  !blocksFor(owner, "pub async fn set(", "topic audience owner set").some((block) => {
    const positions = topicAudienceOrder.map((marker) => block.indexOf(marker));
    return (
      positions.every((position) => position >= 0) &&
      positions.every((position, index) => index === 0 || position > positions[index - 1])
    );
  })
) {
  failures.push(
    "topic audience owner must lock the category tree, then the active topic and scope locks, resolve the category snapshot, and replace the local layer atomically",
  );
}

for (const forbidden of [
  "rustok_groups",
  "rustok_channel",
  /(?:^|[^A-Za-z0-9_])groups::/u,
  /(?:^|[^A-Za-z0-9_])channel::/u,
  "forum_user_stat",
  "serde_json::Value",
  "forum_category::Entity",
  "forum_category_audience_policy::Entity",
  "load_category_ancestor_ids",
]) {
  rejectText(owner, forbidden, `topic audience owner must not depend on ${forbidden}`);
}

for (const marker of [
  "pub struct ForumAudienceConstraints",
  "MAX_FORUM_AUDIENCE_CHANNELS",
  "MAX_FORUM_AUDIENCE_GROUPS",
  "MAX_FORUM_AUDIENCE_EXPLICIT_USERS",
]) {
  requireText(audience, marker, `shared audience contract is missing ${marker}`);
}
for (const marker of [
  "mod topic_audience {",
  'include!("topic_audience.rs");',
  'include!("topic_audience_owner.rs");',
  "ForumTopicAudiencePolicyService",
  "SetForumTopicAudiencePolicyInput",
]) {
  requireText(services, marker, `services composition is missing ${marker}`);
}
for (const marker of [
  "ForumTopicAudiencePolicy",
  "ForumTopicAudiencePolicyService",
  "SetForumTopicAudiencePolicyInput",
]) {
  requireText(crate, marker, `crate export is missing ${marker}`);
}

for (const marker of [
  "topic_audience_layer_narrows_inherited_category_layers_and_remains_bounded",
  "vec![root, child]",
  "empty constraints should clear only the topic layer",
  "database must reject a thirty-third topic channel relation",
  "database must reject mutable topic relation-row updates",
  "database must reject mutable topic policy-row updates",
  "database must reject a cross-tenant topic policy relation",
  "empty local layer",
]) {
  requireText(testSource, marker, `topic audience SQLite scenario is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum topic audience policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum topic audience policy contract is source-ready.");
