import fs from "node:fs";

const command = fs.readFileSync(
  "crates/rustok-groups/src/membership_enforcement_command.rs",
  "utf8",
);
const resolver = fs.readFileSync(
  "crates/rustok-groups/src/membership_enforcement.rs",
  "utf8",
);
const ports = fs.readFileSync("crates/rustok-groups/src/ports.rs", "utf8");
const registry = fs.readFileSync(
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "utf8",
);
const migration = fs.readFileSync(
  "crates/rustok-groups/src/migrations/m20260808_000009_extend_group_domain_events_for_membership_enforcement.rs",
  "utf8",
);
const plan = fs.readFileSync(
  "crates/rustok-groups/docs/implementation-plan.md",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-groups/docs/membership-enforcement-command-contract.md",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "GroupMembershipEnforcementCommandService",
  "suspend_membership",
  "revoke_membership_suspension",
  "groups.membership.suspend.v1",
  "groups.membership.suspension_revoke.v1",
  "expected_membership_revision",
  "MembershipEnforcementRevisionConflict",
  "MembershipEnforcementOwnerProtected",
  "MembershipEnforcementSourceConflict",
  "lock_group",
  "lock_command_memberships",
  "sort_unstable",
  "replay_receipt",
  "receipt.actor_user_id != actor_user_id",
  "apply_membership_suspension_in_tx",
  "revoke_membership_suspension_in_tx",
  "bump_group_version_without_member_count_change",
  '"member_count_semantics": "stored_lifecycle_active"',
  "group.membership_suspended",
  "group.membership_suspension_revoked",
  "groups.membership.suspended",
  "groups.membership.suspension_revoked",
  "ModerationDecision",
]) {
  requireText(command, marker, `Groups enforcement command is missing ${marker}`);
}

for (const marker of [
  "group owner reference and owner membership role disagree",
  "membership.user_id == group_owner_user_id",
]) {
  requireText(resolver, marker, `Groups owner identity resolver is missing ${marker}`);
}

for (const marker of [
  "GroupMembershipEnforcementCommandPort",
  "SharedGroupMembershipEnforcementCommandPort",
]) {
  requireText(ports, marker, `Groups enforcement port surface is missing ${marker}`);
}

for (const marker of [
  '"name": "GroupMembershipEnforcementCommandPort"',
  '"direct_command_port": "implemented_source"',
  '"member_count_semantics": "stored_lifecycle_active_unchanged_by_temporary_enforcement"',
  '"membership_enforcement_command_static_boundary"',
  '"membership_enforcement_command_transport_parity": null',
]) {
  requireText(registry, marker, `Groups FBA enforcement contract is missing ${marker}`);
}

for (const marker of [
  "chk_group_domain_events_kind",
  "groups.membership.suspended",
  "groups.membership.suspension_revoked",
  "group_domain_events_next",
  "cannot downgrade Groups membership enforcement events while append-only membership events exist",
  "groups_targeted_invitation_created_event",
  "group_domain_events_immutable_update",
  "group_domain_events_immutable_delete",
]) {
  requireText(migration, marker, `Groups enforcement event migration is missing ${marker}`);
}

for (const marker of [
  "stored lifecycle active count",
  "Source-complete direct enforcement command",
  "shared Groups enforcement command",
  "neutral moderation subject adapter",
]) {
  requireText(plan, marker, `Canonical Groups plan is missing ${marker}`);
}

for (const marker of [
  "Member-count semantics",
  "stored lifecycle active count",
  "receipt replay",
  "local moderation cannot erase moderation-decision provenance",
  "m20260808_000009_extend_group_domain_events_for_membership_enforcement",
  "cargo test -p rustok-groups",
]) {
  requireText(docs, marker, `Groups enforcement command handoff is missing ${marker}`);
}

console.log("Groups membership enforcement command source guard passed");
