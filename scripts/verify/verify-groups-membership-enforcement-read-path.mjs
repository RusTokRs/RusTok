import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const files = {
  domain: "crates/rustok-groups/src/domain.rs",
  dto: "crates/rustok-groups/src/dto.rs",
  ports: "crates/rustok-groups/src/ports.rs",
  service: "crates/rustok-groups/src/membership_enforcement.rs",
  command: "crates/rustok-groups/src/membership_enforcement_command.rs",
  entities: "crates/rustok-groups/src/membership_enforcement_entities.rs",
  migration:
    "crates/rustok-groups/src/migrations/m20260723_000008_create_group_membership_enforcement_state.rs",
  eventMigration:
    "crates/rustok-groups/src/migrations/m20260808_000009_extend_group_domain_events_for_membership_enforcement.rs",
  migrationRegistry: "crates/rustok-groups/src/migrations/mod.rs",
  module: "crates/rustok-groups/src/lib.rs",
  registry: "crates/rustok-groups/contracts/groups-fba-registry.json",
  plan: "crates/rustok-groups/docs/implementation-plan.md",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups enforcement artifact: ${relative}`);
  }
}

const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
    }
  }
};

if (failures.length === 0) {
  requireMarkers(files.domain, [
    "GroupMembershipEnforcementState",
    "GroupMembershipEnforcementSourceKind",
    "GroupMembershipEffectiveStatus",
    "LegacyBanned",
    "denies_reentry",
  ]);
  requireMarkers(files.dto, [
    "GroupMembershipEnforcementSummary",
    "GroupMembershipEffectiveState",
    "membership_revision",
    "effective_status",
    "active_member",
    "denied_reentry",
    "ReadGroupMembershipEnforcementRequest",
  ]);
  requireMarkers(files.ports, [
    "GroupMembershipEnforcementReadPort",
    "read_membership_enforcement",
    "SharedGroupMembershipEnforcementReadPort",
    "GroupMembershipEnforcementCommandPort",
  ]);
  requireMarkers(files.service, [
    "GroupMembershipEnforcementService",
    "resolve_group_membership_enforcement",
    "Utc::now()",
    "GroupMembershipEffectiveStatus::Missing",
    "GroupMembershipEffectiveStatus::Suspended",
    "GroupMembershipEffectiveStatus::LegacyBanned",
    "&effective_from <= evaluated_at",
    "evaluated_at < until",
    "moderation-driven enforcement decision identity is invalid",
    "group owner reference and owner membership role disagree",
    "membership.user_id == group_owner_user_id",
    "groups.membership_enforcement_forbidden",
    '"groups:access:read"',
    '"groups:moderate"',
  ]);
  if (read(files.service).includes('"groups:read"')) {
    failures.push(`${files.service}: broad groups:read must not reveal enforcement provenance`);
  }
  for (const forbidden of [
    "rustok_moderation::",
    "moderation_case::",
    "policy_snapshot:",
    "appeal_id",
  ]) {
    if (
      read(files.service).includes(forbidden) ||
      read(files.command).includes(forbidden) ||
      read(files.entities).includes(forbidden)
    ) {
      failures.push(`Groups enforcement boundary contains forbidden owner copy/import ${JSON.stringify(forbidden)}`);
    }
  }
  requireMarkers(files.command, [
    "GroupMembershipEnforcementCommandService",
    "apply_membership_suspension_in_tx",
    "revoke_membership_suspension_in_tx",
    "expected_membership_revision",
    "bump_group_version_without_member_count_change",
  ]);
  requireMarkers(files.entities, [
    'table_name = "group_memberships"',
    "pub revision: i64",
    'table_name = "group_membership_enforcements"',
    "moderation_decision_id",
    "moderation_decision_hash",
    "restore_status",
    "revoked_at",
  ]);
  requireMarkers(files.migration, [
    "GroupMemberships::Revision",
    'name("ux_group_memberships_tenant_id")',
    "GroupMembershipEnforcements::Table",
    "fk_group_membership_enforcements_tenant_membership",
    "effective_until IS NULL OR effective_until > effective_from",
    "groups_guard_membership_revision",
    "groups_guard_membership_enforcement",
    "group membership enforcement identity is immutable",
    "group membership enforcement revision must be monotonic",
    "groups_bump_membership_revision_from_enforcement",
    "groups_20_membership_revision_bump",
    "groups_24_membership_enforcement_identity_insert",
    "groups_27_membership_enforcement_revision_bump",
    "groups_30_enforcement_membership_revision_insert",
  ]);
  requireMarkers(files.eventMigration, [
    "groups.membership.suspended",
    "groups.membership.suspension_revoked",
    "cannot downgrade Groups membership enforcement events while append-only membership events exist",
  ]);
  requireMarkers(files.migrationRegistry, [
    "m20260723_000008_create_group_membership_enforcement_state",
    "m20260808_000009_extend_group_domain_events_for_membership_enforcement",
  ]);
  requireMarkers(files.module, [
    "pub mod membership_enforcement;",
    "mod membership_enforcement_command;",
    "GroupMembershipEnforcementService",
    "GroupMembershipEnforcementCommandService",
    "module.migrations().len(), 9",
  ]);
  requireMarkers(files.registry, [
    '"name": "GroupMembershipEnforcementReadPort"',
    '"name": "GroupMembershipEnforcementCommandPort"',
    '"effective_clock": "groups_owner_utc_clock"',
    '"legacy_banned_behavior": "deny_reentry"',
    '"direct_command_port": "implemented_source"',
    '"access_path_integration": "implemented_source"',
    '"moderation_adapter": "not_published_in_this_slice"',
  ]);
  requireMarkers(files.plan, [
    "Source-complete direct enforcement command",
    "GroupMembershipEnforcementReadPort",
    "GroupMembershipEnforcementCommandPort",
    "neutral moderation subject adapter",
    "GROUPS-07 | in_progress",
    "verify-groups-membership-enforcement-read-path.mjs",
    "verify-groups-membership-enforcement-command.mjs",
  ]);
}

if (failures.length > 0) {
  console.error("Groups membership enforcement read-path verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Groups monotonic membership revision, bounded enforcement projection, owner-clock resolver, direct command seam, and open moderation/runtime gates passed source verification.",
);
