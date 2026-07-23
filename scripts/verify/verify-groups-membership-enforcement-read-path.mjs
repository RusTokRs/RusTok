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
  entities: "crates/rustok-groups/src/membership_enforcement_entities.rs",
  migration:
    "crates/rustok-groups/src/migrations/m20260723_000008_create_group_membership_enforcement_state.rs",
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
  ]);
  if (read(files.ports).includes("GroupMembershipEnforcementCommandPort")) {
    failures.push(`${files.ports}: read-only slice must not publish enforcement commands`);
  }
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
    "groups.membership_enforcement_forbidden",
  ]);
  for (const forbidden of [
    "rustok_moderation::",
    "moderation_case::",
    "policy_snapshot:",
    "appeal_id",
  ]) {
    if (read(files.service).includes(forbidden) || read(files.entities).includes(forbidden)) {
      failures.push(`Groups enforcement read boundary contains forbidden owner copy/import ${JSON.stringify(forbidden)}`);
    }
  }
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
    "groups_bump_membership_revision_from_enforcement",
    "groups_20_membership_revision_bump",
    "groups_30_enforcement_membership_revision_insert",
  ]);
  requireMarkers(files.migrationRegistry, [
    "m20260723_000008_create_group_membership_enforcement_state",
  ]);
  requireMarkers(files.module, [
    "pub mod membership_enforcement;",
    "pub mod membership_enforcement_entities;",
    "GroupMembershipEnforcementService",
    "module.migrations().len(), 8",
  ]);
  requireMarkers(files.registry, [
    '"name": "GroupMembershipEnforcementReadPort"',
    '"effective_clock": "groups_owner_utc_clock"',
    '"legacy_banned_behavior": "deny_reentry"',
    '"command_port": "not_published_in_this_slice"',
    '"access_path_integration": "open"',
  ]);
  requireMarkers(files.plan, [
    "membership revision and read-only enforcement projection/resolver are source-complete",
    "GroupMembershipEnforcementReadPort",
    "status-only access-path conversion remains open",
    "GROUPS-07 | in_progress",
    "verify-groups-membership-enforcement-read-path.mjs",
  ]);
}

if (failures.length > 0) {
  console.error("Groups membership enforcement read-path verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Groups monotonic membership revision, bounded enforcement projection, owner-clock resolver, read port, and open integration gates passed source verification.",
);
