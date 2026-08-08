import fs from "node:fs";

const sourcePath = "crates/rustok-groups/src/effective_service.rs";
const contractPath = "crates/rustok-groups/contracts/groups-effective-membership-access.json";
const planPath = "crates/rustok-groups/docs/implementation-plan.md";

const source = fs.readFileSync(sourcePath, "utf8");
const contract = JSON.parse(fs.readFileSync(contractPath, "utf8"));
const plan = fs.readFileSync(planPath, "utf8");

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

const start = source.indexOf("async fn leave_group_owned(");
const end = source.indexOf("\n    async fn set_group_feature_owned(", start);
if (start < 0 || end < 0) {
  throw new Error("effective Groups leave owner function could not be isolated");
}
const leave = source.slice(start, end);

for (const marker of [
  "let transaction = self.db.begin().await?",
  "reserve_group_write_for_update(&transaction, tenant_id, request.group_id).await?",
  "resolve_group_membership_enforcement_for_update(",
  "effective.stored_status == Some(GroupMembershipStatus::Banned)",
  "GroupsError::MembershipBanned",
  "effective.role == Some(GroupRole::Owner)",
  'membership_model.status == GroupMembershipStatus::Left.as_str()',
  "let was_active = membership_model.status == GroupMembershipStatus::Active.as_str()",
  "active.status = Set(GroupMembershipStatus::Left.as_str().to_string())",
  ".update(&transaction)",
  "active.member_count = Set(next_member_count)",
  "active.version = Set(next_version)",
  "transaction.commit().await?",
]) {
  requireText(leave, marker, `transactional Groups leave is missing ${marker}`);
}

for (const forbidden of [
  "GroupCommandPort::leave_group(&self.legacy",
  ".one(&self.db)",
  ".update(&self.db)",
  "INSERT INTO group_membership_enforcements",
  "rustok_moderation::",
]) {
  if (leave.includes(forbidden)) {
    throw new Error(`transactional Groups leave contains stale or foreign-owner shortcut ${forbidden}`);
  }
}

requireText(
  source,
  "self.leave_group_owned(&context, request)",
  "public GroupCommandPort leave must use the effective owner implementation",
);

if (contract?.converted_source_paths?.leave_group !== "transaction_aware_effective_membership") {
  throw new Error("effective membership contract must register transaction-aware leave");
}
if (contract?.access_semantics?.leave_during_active_suspension !== "allowed_preserves_enforcement_projection") {
  throw new Error("effective membership contract must retain leave-during-suspension semantics");
}
if (contract?.access_semantics?.leave_for_legacy_banned_status !== "denied_preserves_ban") {
  throw new Error("effective membership contract must retain legacy-ban preservation on leave");
}
if (contract?.evidence?.leave_runtime !== null) {
  throw new Error("unexecuted leave runtime evidence must remain null");
}

for (const marker of [
  "Source-complete leave effective lifecycle",
  "Temporary suspension does not imprison the participant in the group",
  "Legacy banned membership is different",
  "leave-during-suspension, legacy-ban preservation and enforcement-vs-leave serialization",
  "verify-groups-leave-enforcement-authorization.mjs",
]) {
  requireText(plan, marker, `canonical Groups plan is missing leave enforcement marker ${marker}`);
}

console.log("Groups transaction-aware leave enforcement source guard passed");
