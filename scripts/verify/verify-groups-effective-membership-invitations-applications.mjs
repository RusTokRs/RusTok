import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const files = {
  guard: "crates/rustok-groups/src/effective_membership_guard.rs",
  locking: "crates/rustok-groups/src/membership_enforcement_transaction.rs",
  invitations: "crates/rustok-groups/src/effective_invitations.rs",
  invitationOwner: "crates/rustok-groups/src/invitations_transactional.rs",
  targetedOwner: "crates/rustok-groups/src/targeted_invitations_transactional.rs",
  applications: "crates/rustok-groups/src/effective_applications.rs",
  applicationOwner: "crates/rustok-groups/src/applications_transactional.rs",
  applicationCasOwner: "crates/rustok-groups/src/applications_transactional_cas.rs",
  applicationLifecycleOwner:
    "crates/rustok-groups/src/applications_transactional_lifecycle.rs",
  module: "crates/rustok-groups/src/lib.rs",
  contract:
    "crates/rustok-groups/contracts/groups-effective-membership-invitations-applications.json",
  plan: "crates/rustok-groups/docs/implementation-plan.md",
  runtime: "crates/rustok-groups/docs/README.md",
  readme: "crates/rustok-groups/README.md",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing transactional effective artifact: ${relative}`);
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
  requireMarkers(files.locking, [
    "resolve_group_membership_enforcement_for_update",
    "Group -> GroupMembership -> GroupMembershipEnforcement",
    "UPDATE groups SET version = version",
    ".lock_exclusive()",
    "membership_enforcement::Entity::find_by_id",
    "Do not use rows_affected as an existence test",
  ]);

  requireMarkers(files.guard, [
    "resolve_group_membership_enforcement_now_for_update",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
    "DatabaseTransaction",
    "effective.active_member && role_allowed",
  ]);
  if (read(files.guard).includes("has_existing_receipt")) {
    failures.push(`${files.guard}: obsolete facade receipt probe remains`);
  }

  for (const relative of [files.invitationOwner, files.targetedOwner]) {
    requireMarkers(relative, [
      "replay_receipt",
      "require_user_not_denied_owned",
      "transaction.commit().await?",
    ]);
  }
  requireMarkers(files.invitationOwner, [
    "create_group_invitation_effective_owned",
    "revoke_group_invitation_effective_owned",
    "accept_group_invitation_effective_owned",
    "require_effective_manager_owned",
  ]);
  requireMarkers(files.targetedOwner, [
    "accept_targeted_group_invitation_effective_owned",
  ]);

  requireMarkers(files.applicationOwner, [
    "upsert_policy_effective_owned",
    "submit_application_effective_owned",
    "review_application_effective_owned",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
    "replay_receipt",
  ]);
  requireMarkers(files.applicationCasOwner, [
    "upsert_policy_if_current_effective_owned",
    "submit_application_if_current_effective_owned",
    "ensure_policy_update_precondition",
    "ensure_loaded_policy_precondition",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
  ]);
  requireMarkers(files.applicationLifecycleOwner, [
    "cancel_application_effective_owned",
    "reopen_application_effective_owned",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
  ]);

  requireMarkers(files.invitations, [
    "create_group_invitation_effective_owned",
    "revoke_group_invitation_effective_owned",
    "accept_group_invitation_effective_owned",
    "accept_targeted_group_invitation_effective_owned",
  ]);
  requireMarkers(files.applications, [
    "upsert_policy_effective_owned",
    "submit_application_effective_owned",
    "review_application_effective_owned",
    "upsert_policy_if_current_effective_owned",
    "submit_application_if_current_effective_owned",
    "cancel_application_effective_owned",
    "reopen_application_effective_owned",
    "self.review_effective(item_context, item_request)",
    "map_effective_application_cas_error",
  ]);

  for (const [relative, forbidden] of [
    [files.invitations, "has_existing_receipt"],
    [files.applications, "has_existing_receipt"],
    [files.invitations, "GroupInvitationCommandPort::create_group_invitation(&self.legacy"],
    [files.applications, "GroupApplicationCasCommandPort::upsert_group_application_policy_if_current"],
  ]) {
    if (read(relative).includes(forbidden)) {
      failures.push(`${relative}: public write facade still uses pre-transaction path ${JSON.stringify(forbidden)}`);
    }
  }

  const module = read(files.module);
  requireMarkers(files.module, [
    "mod applications_legacy_module {",
    "include!(\"applications_transactional.rs\")",
    "include!(\"applications_transactional_cas.rs\")",
    "include!(\"applications_transactional_lifecycle.rs\")",
    "mod invitations_legacy {",
    "include!(\"invitations_transactional.rs\")",
    "mod targeted_invitations_legacy {",
    "include!(\"targeted_invitations_transactional.rs\")",
    "mod membership_enforcement_transaction;",
    "pub mod applications {",
    "pub mod invitations {",
    "pub mod targeted_invitations {",
  ]);
  for (const forbidden of [
    "pub mod applications;",
    "pub mod invitations;",
    "pub mod targeted_invitations;",
  ]) {
    if (module.includes(forbidden)) {
      failures.push(`${files.module}: legacy service module remains publicly bypassable: ${forbidden}`);
    }
  }

  requireMarkers(files.contract, [
    '"commercial_membership_or_subscription": false',
    '"authorization_and_mutation_same_transaction": true',
    '"receipt_replay_before_effective_authorization_in_owner_transaction": true',
    '"same_transaction_effective_authorization": "implemented_source"',
    '"concurrent_enforcement_change_race_evidence": null',
  ]);
  requireMarkers(files.plan, [
    "transaction-aware invitation/application writes",
    "Group -> GroupMembership -> GroupMembershipEnforcement",
    "Runtime proof remains open",
  ]);
  requireMarkers(files.runtime, [
    "Transaction-aware effective authorization",
    "SQLite acquires writer serialization",
    "Public write facades do not perform a separate receipt/effective precheck",
  ]);
  requireMarkers(files.readme, [
    "transaction-aware invitation/application writes",
    "effective authorization and mutation",
    "runtime evidence and the remaining owner paths are open",
  ]);
}

if (failures.length > 0) {
  console.error("Groups transactional effective invitation/application verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Groups invitation/application writes use receipt-first transactional effective authorization with sealed public facades and open runtime evidence.",
);
