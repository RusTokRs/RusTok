import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const files = {
  error: "crates/modules/rustok-groups/src/error.rs",
  guard: "crates/modules/rustok-groups/src/effective_membership_guard.rs",
  locking: "crates/modules/rustok-groups/src/membership_enforcement_transaction.rs",
  invitations: "crates/modules/rustok-groups/src/effective_invitations.rs",
  invitationOwner: "crates/modules/rustok-groups/src/invitations_transactional.rs",
  targetedOwner: "crates/modules/rustok-groups/src/targeted_invitations_transactional.rs",
  applications: "crates/modules/rustok-groups/src/effective_applications.rs",
  applicationOwner: "crates/modules/rustok-groups/src/applications_transactional.rs",
  applicationCasOwner: "crates/modules/rustok-groups/src/applications_transactional_cas.rs",
  applicationLifecycleOwner:
    "crates/modules/rustok-groups/src/applications_transactional_lifecycle.rs",
  module: "crates/modules/rustok-groups/src/lib.rs",
  applicationModule: "crates/modules/rustok-groups/src/applications.rs",
  invitationModule: "crates/modules/rustok-groups/src/invitations.rs",
  targetedModule: "crates/modules/rustok-groups/src/targeted_invitations.rs",
  contract:
    "crates/modules/rustok-groups/contracts/groups-effective-membership-invitations-applications.json",
  plan: "crates/modules/rustok-groups/docs/implementation-plan.md",
  runtime: "crates/modules/rustok-groups/docs/README.md",
  readme: "crates/modules/rustok-groups/README.md",
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

const requireOrder = (relative, markers) => {
  const source = read(relative);
  let previous = -1;
  for (const marker of markers) {
    const current = source.indexOf(marker, previous + 1);
    if (current < 0) {
      failures.push(`${relative}: missing ordered marker ${JSON.stringify(marker)}`);
      return;
    }
    if (current <= previous) {
      failures.push(`${relative}: invalid marker order at ${JSON.stringify(marker)}`);
      return;
    }
    previous = current;
  }
};

if (failures.length === 0) {
  requireMarkers(files.error, [
    "MembershipSuspended",
    "MembershipBanned",
    "ManagerRequired",
    "MembershipAlreadyActive",
    '"groups.membership_suspended"',
    '"groups.membership_banned"',
    '"groups.manager_required"',
    '"groups.membership_already_active"',
  ]);

  requireMarkers(files.locking, [
    "resolve_group_membership_enforcement_for_update",
    "Group -> GroupMembership -> GroupMembershipEnforcement",
    "UPDATE groups SET version = version",
    ".lock_exclusive()",
    "membership_enforcement::Entity::find_by_id",
    "do not use rows-affected here as an",
  ]);

  requireMarkers(files.guard, [
    "resolve_group_membership_enforcement_now_for_update",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
    "DatabaseTransaction",
    "GroupsError::MembershipSuspended",
    "GroupsError::MembershipBanned",
    "GroupsError::ManagerRequired",
    "GroupsError::MembershipAlreadyActive",
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
  requireOrder(files.invitationOwner, [
    "REVOKE_INVITATION_COMMAND",
    "require_effective_manager_owned",
    "group invitation is already revoked",
    "active.update(&transaction)",
  ]);
  requireOrder(files.invitationOwner, [
    "ACCEPT_INVITATION_COMMAND",
    "require_user_not_denied_owned",
    "ensure_invitation_active",
    "redemption::Entity::find()",
    "redemption::ActiveModel",
  ]);
  requireOrder(files.targetedOwner, [
    "ACCEPT_TARGETED_INVITATION_COMMAND",
    "require_user_not_denied_owned",
    "ensure_targeted_invitation_active",
    "redemption::Entity::find()",
    "redemption::ActiveModel",
  ]);

  for (const [relative, operation, validator, normalization] of [
    [files.invitationOwner, "revoke_group_invitation_effective_owned", "request.validate()?;", "idempotency_key(context)"],
    [files.targetedOwner, "accept_targeted_group_invitation_effective_owned", "request.validate()?;", "idempotency_key(context)"],
    [files.applicationLifecycleOwner, "cancel_application_effective_owned", "request.validate()?;", "idempotency_key(context)"],
    [files.applicationLifecycleOwner, "reopen_application_effective_owned", "request.validate()?;", "idempotency_key(context)"],
    [files.applicationOwner, "upsert_policy_effective_owned", "request.validate()?;", "normalize_policy("],
    [files.applicationOwner, "submit_application_effective_owned", "request.validate()?;", "normalize_application_submission("],
    [files.applicationOwner, "review_application_effective_owned", "request.validate()?;", "normalize_optional_note("],
    [files.applicationCasOwner, "upsert_policy_if_current_effective_owned", "request.policy.validate()?;", "normalize_policy("],
    [files.applicationCasOwner, "submit_application_if_current_effective_owned", "request.submission.validate()?;", "normalize_application_submission("],
  ]) {
    const source = read(relative);
    const start = source.indexOf(`async fn ${operation}(`);
    const next = source.indexOf("async fn ", start + 1);
    const body = source.slice(start, next < 0 ? undefined : next);
    const validation = body.indexOf(validator);
    const normalized = body.indexOf(normalization);
    const transaction = body.indexOf("self.db.begin()");
    if (start < 0 || validation < 0 || normalized < validation || transaction < validation) {
      failures.push(`${relative}: ${operation} must validate raw DTO before normalization and storage`);
    }
  }

  // Manager-only lifecycle errors must follow authority checks. Candidate visibility
  // gates intentionally use a separate order to preserve secret-group non-disclosure.
  for (const [relative, operation, lifecycle] of [
    [files.invitationOwner, "create_group_invitation_effective_owned", "require_active_group(&group_model)?;"],
    [files.applicationOwner, "upsert_policy_effective_owned", "require_active_group(&group_model)?;"],
    [files.applicationOwner, "review_application_effective_owned", "require_active_group(&group_model)?;"],
    [files.applicationCasOwner, "upsert_policy_if_current_effective_owned", "require_active_group(&group_model)?;"],
    [files.applicationLifecycleOwner, "reopen_application_effective_owned", "require_application_group(&group_model)?;"],
  ]) {
    const source = read(relative);
    const start = source.indexOf(`async fn ${operation}(`);
    const next = source.indexOf("async fn ", start + 1);
    const body = source.slice(start, next < 0 ? undefined : next);
    const authorization = body.indexOf("require_effective_manager_owned(");
    const state = body.indexOf(lifecycle);
    if (start < 0 || authorization < 0 || state < 0 || authorization >= state) {
      failures.push(`${relative}: ${operation} must authorize before disclosing group lifecycle`);
    }
    if (operation === "review_application_effective_owned") {
      const pending = body.indexOf("if membership_model.status != GroupMembershipStatus::Pending.as_str()");
      const write = body.indexOf("membership_active.update(&transaction)");
      if (pending < state || write < 0 || pending >= write) {
        failures.push(`${relative}: review must require a pending membership before writing`);
      }
    }
  }

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
  requireOrder(files.applicationOwner, [
    "SUBMIT_APPLICATION_COMMAND",
    "require_user_not_denied_owned",
    "load_policy_for_locale",
    "validate_submission",
    "membership::ActiveModel",
  ]);
  requireOrder(files.applicationOwner, [
    "REVIEW_APPLICATION_COMMAND",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
    "membership application is no longer pending",
    "membership_active.update(&transaction)",
  ]);
  requireOrder(files.applicationCasOwner, [
    "SUBMIT_APPLICATION_IF_CURRENT_COMMAND",
    "require_user_not_denied_owned",
    "load_policy_for_locale",
    "ensure_loaded_policy_precondition",
    "membership::ActiveModel",
  ]);
  requireOrder(files.applicationLifecycleOwner, [
    "CANCEL_APPLICATION_COMMAND",
    "require_user_not_denied_owned",
    "only a pending membership application can be cancelled",
    "membership_active.update(&transaction)",
  ]);
  requireOrder(files.applicationLifecycleOwner, [
    "REOPEN_APPLICATION_COMMAND",
    "require_effective_manager_owned",
    "require_user_not_denied_owned",
    "let previous_status",
    "membership_active.update(&transaction)",
  ]);

  requireMarkers(files.invitations, [
    "PortCallPolicy::write()",
    "create_group_invitation_effective_owned",
    "revoke_group_invitation_effective_owned",
    "accept_group_invitation_effective_owned",
    "accept_targeted_group_invitation_effective_owned",
  ]);
  requireMarkers(files.applications, [
    "validate_write_context",
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
    '#[path = "applications.rs"]', "mod application_owner;",
    '#[path = "invitations.rs"]', "mod invitation_owner;",
    '#[path = "targeted_invitations.rs"]', "mod targeted_invitation_owner;",
    "mod membership_enforcement_transaction;", "mod commands;",
    "pub mod applications {", "pub mod invitations {", "pub mod targeted_invitations {",
  ]);
  for (const [relative, children] of [
    [files.applicationModule, ["applications_transactional.rs", "applications_transactional_cas.rs", "applications_transactional_lifecycle.rs"]],
    [files.invitationModule, ["invitations_transactional.rs"]],
    [files.targetedModule, ["targeted_invitations_transactional.rs"]],
  ]) {
    for (const child of children) requireMarkers(relative, [`#[path = "${child}"]`]);
    if (read(relative).includes('include!(')) failures.push(`${relative}: source stitching bypasses module ownership`);
  }
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
    "effective authorization, mutation",
    "runtime evidence and remaining provider/owner paths are open",
  ]);
}

// Syntax parsers do not expand attribute macros: catch accidental deduplication leftovers.
for (const entry of fs.readdirSync(path.join(root, "crates/modules/rustok-groups/src"))) {
  if (!entry.endsWith(".rs")) continue;
  const relative = `crates/modules/rustok-groups/src/${entry}`;
  if (/#\[async_trait\]\s*#\[async_trait\]/.test(read(relative))) {
    failures.push(`${relative}: duplicate async_trait macro attribute`);
  }
}
for (const [relative, name] of [
  [files.invitationModule, "ensure_invitation_active"],
  [files.targetedModule, "ensure_targeted_invitation_active"],
]) {
  const source = read(relative);
  const start = source.indexOf(`fn ${name}(`);
  const end = source.indexOf("\n}", start);
  const body = source.slice(start, end);
  if (start < 0 || end < 0 || !body.includes("validate_stored_invitation_uses(model)")) {
    failures.push(`${relative}: ${name} must validate stored use bounds before acceptance`);
  }
}
requireMarkers(files.invitationModule, ["validate_stored_invitation_uses(&model)?;"]);
const serviceSource = read("crates/modules/rustok-groups/src/service.rs");
if (!serviceSource.includes("u64::try_from(model.member_count)")) {
  failures.push("summary projection must reject a negative stored member count");
}
requireMarkers(files.invitationModule, [
  "!(1..=MAX_INVITATION_USES as i32).contains(&model.max_uses)",
  "!(0..=model.max_uses).contains(&model.use_count)",
  "model.target_user_id.is_some() && model.max_uses != 1",
]);
requireMarkers(files.error, [
  "PortErrorKind::Unavailable | PortErrorKind::Timeout => Self::Delegated(value)",
  "GroupsError::Delegated(error) => error",
]);

if (failures.length > 0) {
  console.error("Groups transactional effective invitation/application verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Groups invitation/application writes use receipt-first transactional effective authorization and authorization-first disclosure with stable error codes, sealed public facades, and open runtime evidence.",
);
