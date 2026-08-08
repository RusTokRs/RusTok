import fs from "node:fs";

const groupsContractPath =
  "crates/rustok-groups/contracts/groups-effective-membership-access.json";
const groupsFbaPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const groupsDocsPath =
  "crates/rustok-groups/docs/membership-enforcement-access-path-integration-contract.md";
const forumContractPath =
  "crates/rustok-forum/contracts/forum-audience-group-facts-host-runtime.json";
const adapterPath = "apps/server/src/services/forum_audience_group_facts.rs";
const ownerBackedPath =
  "apps/server/src/services/forum_audience_group_facts/owner_backed_tests.rs";
const compositionPath = "apps/server/src/services/module_event_dispatcher.rs";
const forumGuardPath = "scripts/verify/verify-forum-audience-group-facts-host-runtime.mjs";

const groupsContract = JSON.parse(fs.readFileSync(groupsContractPath, "utf8"));
const groupsFba = JSON.parse(fs.readFileSync(groupsFbaPath, "utf8"));
const docs = fs.readFileSync(groupsDocsPath, "utf8");
const forumContract = JSON.parse(fs.readFileSync(forumContractPath, "utf8"));
const adapter = fs.readFileSync(adapterPath, "utf8");
const ownerBacked = fs.readFileSync(ownerBackedPath, "utf8");
const composition = fs.readFileSync(compositionPath, "utf8");
const forumGuard = fs.readFileSync(forumGuardPath, "utf8");

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

const consumer = groupsContract?.external_provider_consumers?.forum_audience_group_facts;
if (!consumer) {
  throw new Error("Groups effective-membership contract is missing Forum audience provider consumer");
}
if (consumer.status !== "source_delivered_execution_pending") {
  throw new Error("Forum audience provider consumer must remain source-delivered/execution-pending");
}
if (consumer.owner_port !== "rustok_groups::GroupMembershipEnforcementReadPort") {
  throw new Error("Forum audience provider consumer must name the neutral Groups enforcement read port");
}
if (consumer.positive_fact !== "owner_computed_active_member_only") {
  throw new Error("Forum audience positive membership fact must remain owner-computed active-member only");
}
if (consumer.expiry !== "groups_owner_clock_without_cleanup") {
  throw new Error("Forum audience expiry semantics must remain Groups-owner-clock based");
}
if (consumer.storage_bypass !== false) {
  throw new Error("Forum audience provider consumer must not permit Groups storage bypass");
}
if (consumer.sqlite_source !== true || consumer.postgres_source !== true) {
  throw new Error("Forum audience provider consumer must retain both SQLite and PostgreSQL owner-backed source");
}

if (
  groupsContract?.converted_source_paths?.forum_audience_provider_acl !==
  "GroupMembershipEnforcementReadPort via ServerForumAudienceGroupFactsPort"
) {
  throw new Error("Groups converted access paths must retain the Forum audience ACL consumer");
}
if (
  !groupsContract?.remaining_paths?.includes(
    "additional_provider_specific_acl_adapters",
  )
) {
  throw new Error("Groups access contract must keep additional provider ACL adapters open");
}
if (
  groupsContract?.evidence?.provider_acl_static_guard !==
  "scripts/verify/verify-groups-membership-enforcement-access-path-integration.mjs"
) {
  throw new Error("Groups access contract is missing the cross-module provider ACL static guard");
}
if (groupsContract?.evidence?.provider_acl_runtime !== null) {
  throw new Error("unexecuted Groups provider ACL runtime evidence must remain null");
}

if (groupsFba?.membership_enforcement?.access_path_integration !== "implemented_source") {
  throw new Error("Groups FBA must retain source-complete core access-path integration");
}
if (groupsFba?.membership_enforcement?.provider_acl_integration !== "open") {
  throw new Error("Groups FBA broad provider ACL integration must remain open for additional profiles");
}
if (groupsFba?.evidence?.membership_enforcement_access_path_integration !== null) {
  throw new Error("unexecuted membership enforcement access-path runtime evidence must remain null");
}

for (const marker of [
  "GroupMembershipEnforcementReadPort",
  "SharedGroupMembershipEnforcementReadPort",
  ".read_membership_enforcement(",
  "if state.active_member",
  "validate_owner_state",
  "ServerForumAudienceGroupFactsPort",
  "PARTIAL_PROVIDER_CODE",
]) {
  requireText(adapter, marker, `Forum audience Groups adapter is missing ${marker}`);
}
for (const forbidden of [
  "group_memberships",
  "group_membership_enforcements",
  "rustok_groups::entities",
  "membership_enforcement::ActiveModel",
]) {
  if (adapter.includes(forbidden)) {
    throw new Error(`Forum audience Groups adapter contains direct owner-storage shortcut ${forbidden}`);
  }
}

for (const marker of [
  "forum_group_facts_follow_groups_owner_clock_sqlite",
  "forum_group_facts_follow_groups_owner_clock_postgres",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  "SuspendGroupMembershipRequest",
  "effective_until",
  "tokio::time::sleep",
  "facts_after_suspend.group_memberships.is_empty()",
  "facts_after_expiry.group_memberships",
]) {
  requireText(ownerBacked, marker, `Forum audience owner-backed evidence is missing ${marker}`);
}

for (const marker of [
  "ServerForumAudienceGroupFactsPort::shared",
  "ServerForumAudienceFactsPort::shared",
  "extensions.insert(audience_facts)",
]) {
  requireText(composition, marker, `Forum audience host composition is missing ${marker}`);
}

for (const marker of [
  "owner_backed_sqlite_effective_membership_source",
  "owner_backed_postgres_effective_membership_source",
  "groups_owner_port_reuse",
  "storage_access",
  "no direct Groups entity or table access from the server adapter or Forum",
]) {
  requireText(
    JSON.stringify(forumContract),
    marker,
    `Forum audience composition contract is missing ${marker}`,
  );
}
if (forumContract?.verification?.execution_status !== "not_run_by_implementation_agent") {
  throw new Error("Forum audience owner-backed runtime evidence must remain marked unexecuted");
}

for (const marker of [
  "ownerBackedTest",
  "forum_group_facts_follow_groups_owner_clock_sqlite",
  "forum_group_facts_follow_groups_owner_clock_postgres",
  "GroupMembershipEnforcementReadPort",
]) {
  requireText(forumGuard, marker, `Forum audience source guard is missing ${marker}`);
}

for (const marker of [
  "Forum audience provider ACL source delivered / maintainer execution pending",
  "Owner boundary",
  "GroupMembershipEnforcementReadPort::read_membership_enforcement",
  "state.active_member=true",
  "Host composition",
  "Owner-backed backend source",
  "forum_group_facts_follow_groups_owner_clock_sqlite",
  "forum_group_facts_follow_groups_owner_clock_postgres",
  "Degraded and partial-provider semantics",
  "additional_provider_specific_acl_adapters",
  "membership_enforcement_access_path_integration",
]) {
  requireText(docs, marker, `Groups enforcement access-path integration handoff is missing ${marker}`);
}

console.log("Groups membership-enforcement Forum audience access-path integration source guard passed");
