import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");
const governance = read("crates/rustok-groups/src/governance.rs");
const transaction = read("crates/rustok-groups/src/membership_enforcement_transaction.rs");
const graphql = read("crates/rustok-groups/src/graphql_governance.rs");
const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
const plan = read("crates/rustok-groups/docs/implementation-plan.md");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "lock_group(&transaction",
  "reserve_group_write_for_update",
  "replay_receipt::<GroupGovernanceResult>",
  "receipt.group_id != group_id",
  "receipt.actor_user_id != actor_user_id",
  "lock_governance_memberships",
  "lock_ownership_memberships",
  "user_ids.sort_unstable()",
  "membership_ids.sort_unstable()",
  "membership_enforcement::Entity::find_by_id",
  "resolve_group_membership_enforcement",
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupMembershipEffectiveStatus::LegacyBanned",
  "effective_manager_role",
  "effective_governance_role",
  "validate_platform_recovery_owner_state",
  "current group owner effective membership is not recoverable",
  "validate_owner_identity",
  "effective_local_manager",
  "effective_current_owner",
]) {
  requireText(governance, marker, `Groups governance source is missing ${marker}`);
}

for (const forbidden of [
  "async fn find_membership(",
  "fn active_role(",
  "replay_receipt::<GroupGovernanceResult>(\n            &transaction,\n            tenant_id,\n            &idempotency_key",
]) {
  if (governance.includes(forbidden)) {
    throw new Error(`Groups governance retains a pre-cutover shortcut: ${forbidden}`);
  }
}

for (const marker of [
  "reserve_group_write_for_update",
  "UPDATE groups SET version = version WHERE tenant_id = ? AND id = ?",
  "lock_exclusive()",
]) {
  requireText(transaction, marker, `Shared Groups writer reservation is missing ${marker}`);
}

for (const marker of [
  "GroupGovernanceCommandPort::change_group_role",
  "GroupGovernanceCommandPort::transfer_group_ownership",
  "PortActor::user",
  "with_claim(permission.to_string())",
]) {
  requireText(graphql, marker, `Groups governance GraphQL transport is missing ${marker}`);
}

const port = registry?.provider?.ports?.find((item) => item?.name === "GroupGovernanceCommandPort");
if (!port) throw new Error("Groups FBA registry is missing GroupGovernanceCommandPort");
if (port.authorization !== "effective_owner_admin_hierarchy_or_platform_manage") {
  throw new Error("Groups governance must publish effective owner/admin authorization");
}
if (port.receipt_replay_order !== "after_group_serialization_before_current_effective_authorization") {
  throw new Error("Groups governance receipt replay order is not locked");
}
if (port.receipt_identity !== "tenant_group_actor_command_request_hash") {
  throw new Error("Groups governance receipt identity is not actor/group bound");
}
if (port.lock_order !== "group_then_memberships_uuid_order_then_enforcements_membership_id_order") {
  throw new Error("Groups governance lock order is not deterministic");
}
if (port.new_owner_effective_state !== "active_required") {
  throw new Error("Groups ownership transfer must require an effective-active new owner");
}
if (registry?.governance?.effective_authorization !== "implemented_source") {
  throw new Error("Groups governance effective authorization is not source-complete in registry");
}
if (registry?.governance?.platform_owner_recovery !== "may_transfer_away_from_suspended_current_owner") {
  throw new Error("Groups governance platform recovery contract is not locked");
}
if (registry?.evidence?.governance_transport_parity !== null || registry?.evidence?.governance_concurrency !== null) {
  throw new Error("Unexecuted governance runtime evidence must remain null");
}

for (const marker of [
  "Source-complete governance effective authorization",
  "tenant + group + actor + command + request hash",
  "effective-active new owner",
  "suspended current owner",
  "Corrupt enforcement row",
  "verify-groups-governance-effective-authorization.mjs",
  "governance concurrency",
]) {
  requireText(plan, marker, `Canonical Groups plan is missing ${marker}`);
}

console.log("Groups governance effective authorization source guard passed");
