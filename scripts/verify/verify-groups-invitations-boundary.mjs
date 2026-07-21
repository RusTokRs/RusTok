import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const requireFile = (relative) => {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups invitation artifact: ${relative}`);
    return false;
  }
  return true;
};
const requireMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
  }
};
const forbidMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relative}: forbidden marker ${JSON.stringify(marker)}`);
  }
};

requireMarkers("crates/rustok-groups/src/migrations/m20260721_000004_create_group_invitations.rs", [
  "GroupInvitations::Table",
  "GroupInvitationRedemptions::Table",
  "TokenHash",
  ".string_len(64)",
  "max_uses BETWEEN 1 AND 100",
  "target_user_id IS NULL OR max_uses = 1",
  "ux_group_invitations_token_hash",
  "ux_group_invitation_redemptions_tenant_invitation_user",
]);
requireMarkers("crates/rustok-groups/src/invitations.rs", [
  "GroupInvitationReadPort",
  "GroupInvitationCommandPort",
  "Sha256::digest(token.as_bytes())",
  "stored_result.token = None",
  "replayed.token = None",
  "lock_exclusive()",
  "redemption::ActiveModel",
  "append_audit",
  "store_receipt",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
]);
forbidMarkers("crates/rustok-groups/src/invitations.rs", [
  "token: Set(",
  "token_hash: Set(token.to_string())",
  '"token": token',
]);

requireMarkers("crates/rustok-groups/src/graphql_invitations.rs", [
  "group_invitations",
  "create_group_invitation",
  "revoke_group_invitation",
  "accept_group_invitation",
  "accept_targeted_group_invitation",
  "GroupInvitationReadPort",
  "GroupInvitationCommandPort",
  "with_idempotency_key",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_applications::GroupsQueryRoot"',
  'mutation = "graphql_applications::GroupsMutationRoot"',
  'subpath = "invitations"',
]);

requireMarkers("crates/rustok-groups/admin/src/core.rs", [
  "prepare_group_invitation_query",
  "prepare_create_group_invitation",
  "prepare_revoke_group_invitation",
  "TargetedInviteMustBeSingleUse",
]);
forbidMarkers("crates/rustok-groups/admin/src/core.rs", ["use leptos", "leptos::"]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "load_group_admin_invitations",
  "create_group_admin_invitation",
  "revoke_group_admin_invitation",
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/invitations.rs", [
  "prepare_group_invitation_query",
  "prepare_create_group_invitation",
  "prepare_revoke_group_invitation",
  "load_group_admin_invitations",
  "create_group_admin_invitation",
  "revoke_group_admin_invitation",
  "groups.admin.invitations.tokenOnce",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/invitations.rs", [
  "graphql_invitations_adapter",
  "native_invitations_adapter",
]);

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  const messages = JSON.parse(read(relative));
  for (const key of [
    "groups.admin.invitations.title",
    "groups.admin.invitations.targetUserId",
    "groups.admin.invitations.tokenOnce",
    "groups.admin.invitations.targetedSingleUse",
  ]) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing invitation key ${key}`);
    }
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  const readPort = registry?.provider?.ports?.find((port) => port?.name === "GroupInvitationReadPort");
  const commandPort = registry?.provider?.ports?.find((port) => port?.name === "GroupInvitationCommandPort");
  if (!readPort?.operations?.includes("list_group_invitations")) {
    failures.push("Groups registry is missing invitation read operation");
  }
  for (const operation of [
    "create_group_invitation",
    "revoke_group_invitation",
    "accept_group_invitation",
  ]) {
    if (!commandPort?.operations?.includes(operation)) {
      failures.push(`Groups registry is missing invitation operation: ${operation}`);
    }
  }
  if (registry?.invitations?.plaintext_token_storage !== "never") {
    failures.push("Groups invitation registry must forbid plaintext token storage");
  }
  if (registry?.invitations?.token_digest !== "sha256_hex") {
    failures.push("Groups invitation registry must declare SHA-256 token digests");
  }
  if (registry?.invitations?.create_replay_token !== "null") {
    failures.push("Groups invitation replay must not reveal plaintext token");
  }
  if (registry?.evidence?.invitation_transport_parity !== null || registry?.evidence?.invitation_concurrency !== null) {
    failures.push("unexecuted invitation runtime evidence must remain null");
  }
}

if (failures.length > 0) {
  console.error("Groups invitation boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups invitation token, digest, redemption, FBA, FFA, and no-fallback boundary checks passed.");
