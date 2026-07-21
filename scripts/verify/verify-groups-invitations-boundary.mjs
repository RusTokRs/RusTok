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
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
    }
  }
};
const forbidMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) {
      failures.push(`${relative}: forbidden marker ${JSON.stringify(marker)}`);
    }
  }
};

const required = [
  "crates/rustok-groups/src/migrations/m20260721_000004_create_group_invitations.rs",
  "crates/rustok-groups/src/migrations/mod.rs",
  "crates/rustok-groups/src/invitation_entities.rs",
  "crates/rustok-groups/src/invitations.rs",
  "crates/rustok-groups/src/graphql_invitations.rs",
  "crates/rustok-groups/src/lib.rs",
  "crates/rustok-groups/src/ports.rs",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/admin/src/model.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/graphql_invitations_adapter.rs",
  "crates/rustok-groups/admin/src/transport/native_invitations_adapter.rs",
  "crates/rustok-groups/admin/src/ui/invitations.rs",
  "crates/rustok-groups/admin/src/ui/root.rs",
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/docs/README.md",
  "crates/rustok-groups/docs/implementation-plan.md",
];
for (const relative of required) requireFile(relative);

requireMarkers(
  "crates/rustok-groups/src/migrations/m20260721_000004_create_group_invitations.rs",
  [
    "GroupInvitations::Table",
    "GroupInvitationRedemptions::Table",
    "ColumnDef::new(GroupInvitations::TokenHash)",
    ".string_len(64)",
    "max_uses BETWEEN 1 AND 100",
    "use_count >= 0 AND use_count <= max_uses",
    "expires_at > created_at",
    "ux_group_invitations_token_hash",
    "ux_group_invitations_tenant_id",
    "ux_group_invitation_redemptions_tenant_invitation_user",
    "fk_group_invitations_tenant_group",
    "fk_group_invitation_redemptions_tenant_invitation",
    "fk_group_invitation_redemptions_tenant_group",
  ],
);
requireMarkers("crates/rustok-groups/src/migrations/mod.rs", [
  "mod m20260721_000004_create_group_invitations;",
  "Box::new(m20260721_000004_create_group_invitations::Migration)",
]);

requireMarkers("crates/rustok-groups/src/invitation_entities.rs", [
  "table_name = \"group_invitations\"",
  "pub token_hash: String",
  "pub max_uses: i32",
  "pub use_count: i32",
  "table_name = \"group_invitation_redemptions\"",
  "pub invitation_id: Uuid",
  "pub user_id: Uuid",
]);
forbidMarkers("crates/rustok-groups/src/invitation_entities.rs", [
  "pub token: String",
  "pub plaintext_token",
]);

requireMarkers("crates/rustok-groups/src/invitations.rs", [
  "GroupInvitationService",
  "GroupInvitationReadPort",
  "GroupInvitationCommandPort",
  "MIN_EXPIRY_SECONDS: u64 = 300",
  "MAX_EXPIRY_SECONDS: u64 = 30 * 24 * 60 * 60",
  "MAX_INVITATION_USES: u32 = 100",
  "a targeted invitation must have max_uses equal to 1",
  "generate_invitation_token",
  "invitation_token_hash",
  "Sha256::digest(token.as_bytes())",
  "stored_result.token = None",
  "replayed.token = None",
  "find_invitation_by_token_for_update",
  "lock_exclusive()",
  "group invitation token is invalid or unavailable",
  "redemption::ActiveModel",
  "GroupMembershipStatus::Active",
  "member_count.saturating_add(1)",
  "append_audit",
  "store_receipt",
  "group.invitation_created",
  "group.invitation_revoked",
  "group.invitation_accepted",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
]);
forbidMarkers("crates/rustok-groups/src/invitations.rs", [
  "token: Set(",
  "token_hash: Set(token.to_string())",
  "\"token\": token",
]);

requireMarkers("crates/rustok-groups/src/graphql_invitations.rs", [
  "pub struct GroupsQueryRoot",
  "pub struct GroupsMutationRoot",
  "GroupsInvitationsQuery",
  "GroupsInvitationsMutation",
  "group_invitations",
  "create_group_invitation",
  "revoke_group_invitation",
  "accept_group_invitation",
  "GroupInvitationReadPort",
  "GroupInvitationCommandPort",
  "with_idempotency_key",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  "query = \"graphql_invitations::GroupsQueryRoot\"",
  "mutation = \"graphql_invitations::GroupsMutationRoot\"",
  "subpath = \"invitations\"",
]);
requireMarkers("crates/rustok-groups/src/ports.rs", [
  "GroupInvitationReadPort",
  "GroupInvitationCommandPort",
]);

requireMarkers("crates/rustok-groups/admin/src/core.rs", [
  "MIN_GROUP_INVITATION_EXPIRY_SECONDS: u64 = 300",
  "MAX_GROUP_INVITATION_EXPIRY_SECONDS: u64 = 30 * 24 * 60 * 60",
  "MAX_GROUP_INVITATION_USES: u32 = 100",
  "prepare_group_invitation_query",
  "prepare_create_group_invitation",
  "prepare_revoke_group_invitation",
  "TargetedInviteMustBeSingleUse",
  "groups-admin-create-invitation",
  "groups-admin-revoke-invitation",
]);
forbidMarkers("crates/rustok-groups/admin/src/core.rs", [
  "use leptos",
  "leptos::",
]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "native_invitations_adapter",
  "graphql_invitations_adapter",
  "load_group_admin_invitations",
  "create_group_admin_invitation",
  "revoke_group_admin_invitation",
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers(
  "crates/rustok-groups/admin/src/transport/native_invitations_adapter.rs",
  [
    "groups/admin/invitations/list",
    "groups/admin/invitations/create",
    "groups/admin/invitations/revoke",
    "GroupInvitationReadPort",
    "GroupInvitationCommandPort",
    "with_idempotency_key",
  ],
);
requireMarkers(
  "crates/rustok-groups/admin/src/transport/graphql_invitations_adapter.rs",
  [
    "GroupsAdminInvitations",
    "GroupsAdminCreateInvitation",
    "GroupsAdminRevokeInvitation",
    "groupInvitations",
    "createGroupInvitation",
    "revokeGroupInvitation",
  ],
);
requireMarkers("crates/rustok-groups/admin/src/ui/invitations.rs", [
  "prepare_group_invitation_query",
  "prepare_create_group_invitation",
  "prepare_revoke_group_invitation",
  "load_group_admin_invitations",
  "create_group_admin_invitation",
  "revoke_group_admin_invitation",
  "groups.admin.invitations.tokenOnce",
  "groups.admin.invitations.shareable",
  "one_time_token",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/invitations.rs", [
  "graphql_invitations_adapter",
  "native_invitations_adapter",
  "unwrap_or_else(|| \"shareable\"",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/root.rs", [
  "GroupsInvitationsAdmin",
]);

const invitationLocaleKeys = [
  "groups.admin.invitations.title",
  "groups.admin.invitations.body",
  "groups.admin.invitations.groupId",
  "groups.admin.invitations.targetUserId",
  "groups.admin.invitations.targetHint",
  "groups.admin.invitations.expirySeconds",
  "groups.admin.invitations.maxUses",
  "groups.admin.invitations.invitationId",
  "groups.admin.invitations.includeInactive",
  "groups.admin.invitations.load",
  "groups.admin.invitations.create",
  "groups.admin.invitations.revoke",
  "groups.admin.invitations.empty",
  "groups.admin.invitations.busy",
  "groups.admin.invitations.error",
  "groups.admin.invitations.loaded",
  "groups.admin.invitations.created",
  "groups.admin.invitations.revoked",
  "groups.admin.invitations.tokenOnce",
  "groups.admin.invitations.version",
  "groups.admin.invitations.shareable",
  "groups.admin.invitations.invalidGroupId",
  "groups.admin.invitations.invalidInvitationId",
  "groups.admin.invitations.invalidTargetUserId",
  "groups.admin.invitations.invalidExpiry",
  "groups.admin.invitations.invalidMaxUses",
  "groups.admin.invitations.targetedSingleUse",
];
for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  let messages;
  try {
    messages = JSON.parse(read(relative));
  } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
    continue;
  }
  for (const key of invitationLocaleKeys) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing invitation key ${key}`);
    }
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  let registry;
  try {
    registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  } catch (error) {
    failures.push(`Groups FBA registry is invalid JSON: ${error.message}`);
  }
  if (registry) {
    const readPort = registry?.provider?.ports?.find(
      (port) => port?.name === "GroupInvitationReadPort",
    );
    const commandPort = registry?.provider?.ports?.find(
      (port) => port?.name === "GroupInvitationCommandPort",
    );
    if (!readPort?.operations?.includes("list_group_invitations")) {
      failures.push("Groups FBA registry is missing invitation read operation");
    }
    for (const operation of [
      "create_group_invitation",
      "revoke_group_invitation",
      "accept_group_invitation",
    ]) {
      if (!commandPort?.operations?.includes(operation)) {
        failures.push(`Groups FBA registry is missing invitation operation: ${operation}`);
      }
    }
    if (!commandPort?.transactional_receipt || !commandPort?.transactional_audit) {
      failures.push("Groups invitation commands must declare transactional receipt and audit");
    }
    if (registry?.invitations?.plaintext_token_storage !== "never") {
      failures.push("Groups invitation registry must forbid plaintext token storage");
    }
    if (registry?.invitations?.token_digest !== "sha256_hex") {
      failures.push("Groups invitation registry must declare SHA-256 token digests");
    }
    if (registry?.invitations?.create_replay_token !== "null") {
      failures.push("Groups invitation create replay must not reveal plaintext token");
    }
    if (
      registry?.invitations?.targeted_max_uses !== 1 ||
      registry?.invitations?.shareable_max_uses !== 100
    ) {
      failures.push("Groups invitation use bounds are not locked");
    }
    if (
      registry?.invitations?.minimum_expiry_seconds !== 300 ||
      registry?.invitations?.maximum_expiry_seconds !== 2592000
    ) {
      failures.push("Groups invitation expiry bounds are not locked");
    }
    if (registry?.invitations?.synchronous_notification_delivery !== false) {
      failures.push("Groups must not synchronously own invitation delivery");
    }
    const profile = registry?.transport_profiles?.find(
      (entry) => entry?.name === "embedded_invitations_native",
    );
    for (const surface of ["rust_port", "graphql", "leptos_server_function"]) {
      if (!profile?.surfaces?.includes(surface)) {
        failures.push(`Groups invitation profile is missing surface: ${surface}`);
      }
    }
    if (profile?.implicit_fallback !== false) {
      failures.push("Groups invitation transport profile must reject implicit fallback");
    }
    if (
      registry?.evidence?.invitation_static_boundary !==
      "scripts/verify/verify-groups-invitations-boundary.mjs"
    ) {
      failures.push("Groups invitation static evidence path is not registered");
    }
  }
}

requireMarkers("crates/rustok-groups/docs/README.md", [
  "GroupInvitationReadPort",
  "GroupInvitationCommandPort",
  "returns plaintext only from the first",
  "create replay returns the receipt-backed invitation with `token = null`",
  "inserts one unique redemption per user",
  "does not synchronously send email",
]);
requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "`GROUPS-05` | `in_progress`",
  "SHA-256-only persistence",
  "concurrent invitation acceptance at the final use",
  "create replay returning `token = null`",
]);

if (failures.length > 0) {
  console.error("Groups invitation boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups invitation storage, token secrecy, transport, UI, and ownership boundary checks passed.");
