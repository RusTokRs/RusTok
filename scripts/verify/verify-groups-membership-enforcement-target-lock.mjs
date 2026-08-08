import fs from "node:fs";

const sourcePath = "crates/rustok-groups/src/membership_enforcement_transaction.rs";
const docsPath = "crates/rustok-groups/docs/membership-enforcement-target-lock-contract.md";
const source = fs.readFileSync(sourcePath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

for (const marker of [
  "pub(crate) struct LockedMembershipEnforcementTarget",
  "lock_membership_enforcement_target_by_id_for_update",
  "membership_state::Entity::find_by_id(membership_id)",
  "reserve_group_write_for_update(transaction, tenant_id, locator.group_id).await?",
  "group::Entity::find()",
  ".filter(membership_state::Column::GroupId.eq(locator.group_id))",
  "query.lock_exclusive()",
  "membership_enforcement::Entity::find_by_id(locked_membership.id)",
  "locked_membership.group_id != locator.group_id",
  "locked_membership.user_id != locator.user_id",
  "membership subject aggregate identity changed while owner locks were acquired",
]) {
  requireText(source, marker, `Groups membership target-lock source is missing ${marker}`);
}

const locator = source.indexOf("let Some(locator) = membership_state::Entity::find_by_id(membership_id)");
const groupReservation = source.indexOf(
  "reserve_group_write_for_update(transaction, tenant_id, locator.group_id).await?",
  locator,
);
const lockedMembership = source.indexOf("let locked_membership = match", groupReservation);
const enforcement = source.indexOf("let enforcement = match", lockedMembership);
if (!(locator >= 0 && locator < groupReservation && groupReservation < lockedMembership && lockedMembership < enforcement)) {
  throw new Error("Groups membership target-lock source must preserve locator -> Group -> Membership -> Enforcement ordering");
}

for (const forbidden of [
  "rustok_moderation",
  "rustok_outbox",
  "apply_membership_suspension_in_tx",
  "group_command_receipts",
]) {
  if (source.includes(forbidden)) {
    throw new Error(`Groups membership target-lock primitive must remain owner-lock-only: ${forbidden}`);
  }
}

for (const marker of [
  "locator read",
  "Group -> GroupMembership -> GroupMembershipEnforcement",
  "receipt admission **before** calling this primitive",
  "crate-private",
  "Runtime contention evidence remains pending",
]) {
  requireText(docs, marker, `Groups membership target-lock handoff is missing ${marker}`);
}

console.log("Groups membership-subject owner-lock source guard passed");
