import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const files = {
  guard: "crates/rustok-groups/src/effective_membership_guard.rs",
  invitations: "crates/rustok-groups/src/effective_invitations.rs",
  applications: "crates/rustok-groups/src/effective_applications.rs",
  module: "crates/rustok-groups/src/lib.rs",
  contract:
    "crates/rustok-groups/contracts/groups-effective-membership-invitations-applications.json",
  plan: "crates/rustok-groups/docs/implementation-plan.md",
  readme: "crates/rustok-groups/README.md",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing effective invitation/application artifact: ${relative}`);
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
  requireMarkers(files.guard, [
    "resolve_group_membership_enforcement",
    "GroupManagerCapability",
    "has_existing_receipt",
    "groups.membership_suspended",
    "groups.membership_banned",
    "effective.active_member && role_allowed",
  ]);

  requireMarkers(files.invitations, [
    "pub struct GroupInvitationService",
    "pub struct GroupTargetedInvitationService",
    "require_effective_manager",
    "require_candidate_not_denied",
    "has_existing_receipt",
    "GroupInvitationCommandPort::accept_group_invitation",
    "GroupTargetedInvitationCommandPort::accept_targeted_group_invitation",
  ]);

  requireMarkers(files.applications, [
    "pub struct GroupApplicationService",
    "require_effective_manager",
    "require_candidate_not_denied",
    "require_user_not_denied",
    "has_existing_receipt",
    "GroupApplicationCasCommandPort",
    "GroupApplicationLifecycleCommandPort",
    "GroupApplicationPolicyManagementReadPort",
    "GroupApplicationBulkReviewCommandPort",
    "self.review_effective(item_context, item_request)",
    "groups-bulk-review:",
  ]);

  const module = read(files.module);
  requireMarkers(files.module, [
    "#[path = \"applications.rs\"]",
    "mod applications_legacy_module;",
    "#[path = \"invitations.rs\"]",
    "mod invitations_legacy;",
    "#[path = \"targeted_invitations.rs\"]",
    "mod targeted_invitations_legacy;",
    "pub mod applications {",
    "pub mod invitations {",
    "pub mod targeted_invitations {",
    "effective_applications::GroupApplicationService",
    "effective_invitations::GroupInvitationService",
    "effective_invitations::GroupTargetedInvitationService",
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
    '"compatibility_module_paths_preserved": true',
    '"existing_receipt_checked_before_effective_precheck": true',
    '"bulk_review": "bounded_partial_result_via_effective_single_review"',
    '"same_transaction_effective_recheck": "open"',
  ]);
  requireMarkers(files.plan, [
    "invitation and membership-application effective facades are source-complete",
    "Same-transaction effective recheck remains open",
    "verify-groups-effective-membership-invitations-applications.mjs",
  ]);
  requireMarkers(files.readme, [
    "effective invitation and membership-application facades",
    "receipt-first replay",
    "same-transaction effective",
  ]);
}

if (failures.length > 0) {
  console.error("Groups effective invitation/application verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Groups effective invitation and membership-application facades, sealed compatibility paths, receipt-first replay, bounded bulk review, and remaining transaction gate passed source verification.",
);
