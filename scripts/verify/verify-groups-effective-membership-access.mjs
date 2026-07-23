import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const files = {
  facade: "crates/rustok-groups/src/effective_service.rs",
  legacy: "crates/rustok-groups/src/service.rs",
  module: "crates/rustok-groups/src/lib.rs",
  enforcement: "crates/rustok-groups/src/membership_enforcement.rs",
  graphql: "crates/rustok-groups/src/graphql.rs",
  adminNative: "crates/rustok-groups/admin/src/transport/native_server_adapter.rs",
  storefrontNative: "crates/rustok-groups/storefront/src/transport/native_server_adapter.rs",
  contract: "crates/rustok-groups/contracts/groups-effective-membership-access.json",
  plan: "crates/rustok-groups/docs/implementation-plan.md",
  readme: "crates/rustok-groups/README.md",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing effective membership access artifact: ${relative}`);
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
  requireMarkers(files.facade, [
    "pub struct GroupsService",
    "LegacyGroupsService",
    "resolve_group_membership_enforcement",
    "GroupSummaryReadPort::read_group(&self.legacy",
    "details.body = None",
    "details.features.clear()",
    "GroupAction::ViewMembers",
    "effective.denied_reentry",
    "group membership is suspended or banned",
    "effective.active_member",
    "GroupRole::can_manage_settings",
    "groups.access.membership_suspended",
    "groups.access.membership_banned",
    "active_member && membership_role == Some(GroupRole::Owner)",
  ]);

  const facade = read(files.facade);
  if (facade.includes("row.status == GroupMembershipStatus::Banned.as_str()")) {
    failures.push(`${files.facade}: final facade must not authorize through a status-only banned check`);
  }
  if (!facade.includes("visibility == GroupVisibility::Public || active_member")) {
    failures.push(`${files.facade}: public/private read split is not source-locked`);
  }

  const moduleSource = read(files.module);
  requireMarkers(files.module, [
    "mod effective_service;",
    "mod service;",
    "pub use effective_service::GroupsService;",
    "cannot bypass the effective membership facade",
  ]);
  for (const forbidden of ["pub mod effective_service;", "pub mod service;", "pub use service::GroupsService;"]) {
    if (moduleSource.includes(forbidden)) {
      failures.push(`${files.module}: public legacy/effective implementation surface remains: ${forbidden}`);
    }
  }

  requireMarkers(files.enforcement, [
    "pub(crate) async fn resolve_group_membership_enforcement",
    "&effective_from <= evaluated_at",
    "GroupMembershipEffectiveStatus::Suspended",
    "GroupMembershipEffectiveStatus::LegacyBanned",
  ]);

  for (const relative of [files.graphql, files.adminNative, files.storefrontNative]) {
    const source = read(relative);
    if (!source.includes("GroupsService")) {
      failures.push(`${relative}: module-owned surface does not materialize the crate-root GroupsService`);
    }
    if (source.includes("rustok_groups::service::GroupsService")) {
      failures.push(`${relative}: module-owned surface bypasses the effective crate-root facade`);
    }
  }

  requireMarkers(files.contract, [
    '"not_commercial_membership": true',
    '"type": "rustok_groups::GroupsService"',
    '"implementation_module_visibility": "crate_private"',
    '"legacy_delegate_visibility": "crate_private"',
    '"external_legacy_delegate_bypass": "sealed"',
    '"group_access_decision": "effective_membership_resolver"',
    '"join_and_rejoin": "effective_membership_resolver"',
    '"feature_settings_authorization": "effective_membership_resolver"',
    '"invitation_management_and_acceptance"',
    '"direct_suspend_revoke_commands"',
  ]);

  requireMarkers(files.plan, [
    "Group membership is social participation",
    "core public access facade is source-complete",
    "status-only access-path conversion remains open",
    "crate-private",
    "verify-groups-effective-membership-access.mjs",
  ]);
  requireMarkers(files.readme, [
    "not a paid subscription",
    "effective-membership `GroupsService`",
    "crate-private",
    "invitation/application/localization/governance",
  ]);
}

if (failures.length > 0) {
  console.error("Groups effective membership access verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Groups sealed crate-root effective membership facade, private redaction, join/re-entry denial, settings authorization, terminology, and remaining gates passed source verification.",
);
