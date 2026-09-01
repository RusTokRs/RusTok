import fs from "node:fs";

const forumCargo = fs.readFileSync("crates/rustok-forum/Cargo.toml", "utf8");
const forumLib = fs.readFileSync("crates/rustok-forum/src/lib.rs", "utf8");
const migrations = fs.readFileSync("crates/rustok-forum/src/migrations/mod.rs", "utf8");
const revisionMigration = fs.readFileSync(
  "crates/rustok-forum/src/migrations/m20260807_000027_add_forum_moderation_subject_revisions.rs",
  "utf8",
);
const adapter = fs.readFileSync("crates/rustok-forum/src/moderation_subject.rs", "utf8");
const replyOwner = fs.readFileSync("crates/rustok-forum/src/services/reply_owner.rs", "utf8");
const recoveryTransport = fs.readFileSync("apps/server/src/graphql/moderation_recovery.rs", "utf8");
const contract = JSON.parse(
  fs.readFileSync("crates/rustok-forum/contracts/forum-moderation-subject-adapter.json", "utf8"),
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

function forbidText(source, needle, message) {
  if (source.includes(needle)) throw new Error(message);
}

requireText(
  forumCargo,
  'rustok-moderation-api = { path = "../rustok-moderation-api" }',
  "Forum must depend only on the neutral Moderation API",
);
forbidText(
  forumCargo,
  "rustok-moderation =",
  "Forum must not depend on the Moderation owner crate",
);
requireText(
  forumLib,
  "register_moderation_subject_adapter_factory",
  "ForumModule must register Moderation subject adapter factories",
);
requireText(
  forumLib,
  "ForumModerationSubjectAdapterFactory::topic()",
  "ForumModule must register the topic adapter factory",
);
requireText(
  forumLib,
  "ForumModerationSubjectAdapterFactory::reply()",
  "ForumModule must register the reply adapter factory",
);
requireText(
  migrations,
  "m20260807_000027_add_forum_moderation_subject_revisions",
  "Forum must register its moderation subject revision migration",
);

for (const marker of [
  "forum_topic_moderation_subject_revisions",
  "forum_reply_moderation_subject_revisions",
  "forum_topic_moderation_subject_revision_owner_update",
  "forum_reply_moderation_subject_revision_owner_update",
  "forum_topic_moderation_subject_revision_translation_update",
  "forum_reply_moderation_subject_revision_body_update",
  "forum_topic_moderation_subject_revision_tenant_insert",
  "forum_topic_moderation_subject_revision_tenant_update",
  "forum_reply_moderation_subject_revision_tenant_insert",
  "forum_reply_moderation_subject_revision_tenant_update",
  "forum topic moderation subject revision tenant mismatch",
  "forum reply moderation subject revision tenant mismatch",
  "revision = revision + 1",
]) {
  requireText(
    revisionMigration,
    marker,
    `Forum moderation subject revision migration is missing ${marker}`,
  );
}

for (const marker of [
  "ModerationSubjectCommandPort",
  "ModerationSubjectKind::ForumTopic",
  "ModerationSubjectKind::ForumPost",
  "PortCallPolicy::write()",
  "PortActorKind::Service",
  "PortActorKind::System",
  "idempotency::admit",
  "command.decision_id.to_string()",
  "forum.moderation_decision_idempotency_mismatch",
  "forum.moderation_subject_revision_conflict",
  "forum_topic_moderation_subject_revisions",
  "forum_reply_moderation_subject_revisions",
  "FOR UPDATE",
  "IsolationLevel::Serializable",
  "TopicService::set_locked_in_tx",
  "publish_forum_topic_projection_direct_in_tx",
  "ModerationVisibilityState::Hidden",
  "apply_reply_hidden_effect_in_tx",
  "ModerationVisibilityState::Removed",
  "apply_reply_removed_effect_in_tx",
  "ModerationDecisionEffectAction::RejectPublication",
  "apply_reply_rejected_effect_in_tx",
  "apply_reply_non_public_status_effect_in_tx",
  "ReplyStatus::Rejected",
  "ReplyService::remove_in_tx",
  "ReplyService::set_status_in_tx",
  "TopicService::adjust_reply_count_in_tx",
  "CategoryService::adjust_counters_in_tx",
  "UserStatsService::adjust_reply_count_in_tx",
  "DomainEvent::ForumReplyStatusChanged",
  "publish_forum_category_projection_direct_in_tx",
  "forum.moderation_subject_revision_not_advanced",
]) {
  requireText(adapter, marker, `Forum Moderation adapter is missing ${marker}`);
}

for (const marker of [
  "ReplyRemovalOutcome",
  "pub(crate) async fn remove_in_tx",
  "claim_reply_delete_in_tx",
  "reply.status.validate_transition(&ReplyStatus::Deleted)",
  "forum_solution::Entity::delete_many()",
  "mark_reply_deleted_in_tx",
  "TopicService::adjust_reply_count_in_tx",
  "CategoryService::adjust_counters_in_tx",
  "UserStatsService::adjust_reply_count_in_tx",
  "UserStatsService::adjust_solution_count_in_tx",
]) {
  requireText(replyOwner, marker, `Forum reply removal owner path is missing ${marker}`);
}

// Unpublished remains intentionally unsupported and distinct from the exact
// RejectPublication -> ReplyStatus::Rejected owner contract. Removed is allowed
// only through ReplyService::remove_in_tx.
forbidText(
  adapter,
  "ModerationVisibilityState::Unpublished",
  "Forum must not approximate neutral Unpublished as the rejected publication lifecycle",
);
forbidText(
  adapter,
  "mark_reply_deleted_in_tx",
  "Moderation adapter must not bypass the Forum reply removal owner helper",
);
forbidText(
  adapter,
  "forum_solution::Entity",
  "Moderation adapter must not duplicate Forum accepted-solution removal logic",
);

for (const forbidden of [
  "rustok_moderation::",
  "moderation_cases",
  "moderation_reports",
  "moderation_appeals",
  "forum_topic_revision::Entity",
  "forum_reply_revision::Entity",
  "Column::DeletedAt",
  "reactionSnapshot",
  "applyReaction",
  "ReactionBar",
]) {
  forbidText(adapter, forbidden, `Forum Moderation adapter contains forbidden owner logic: ${forbidden}`);
}

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("Forum Moderation contract must remain source-ready pending execution");
}
if (contract.capability_owner !== "rustok-moderation") {
  throw new Error("Moderation ownership must remain with rustok-moderation");
}
if (contract.implementation_status !== "complete") {
  throw new Error("FORUM-19 bounded implementation must be complete");
}
if (contract.production_validation !== "deferred" || !contract.completion_boundary?.includes("deployment-dependent promotion")) {
  throw new Error("FORUM-19 production validation must be deferred without reopening implementation");
}
for (const marker of [
  "requeue_moderation_application",
  "reconcile_legacy_moderation_application",
  "create_moderation_rereview",
  "auth.is_human_user_principal()",
  "Permission::MODERATION_CASES_OVERRIDE",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
]) {
  requireText(recoveryTransport, marker, `Moderation recovery GraphQL transport is missing ${marker}`);
}
if (!contract.supported_effects.some((effect) => effect.includes("SetVisibility Hidden"))) {
  throw new Error("contract must record the exact hidden reply visibility effect");
}
if (!contract.supported_effects.some((effect) => effect.includes("SetVisibility Removed"))) {
  throw new Error("contract must record the exact removed reply visibility effect");
}
if (!contract.supported_effects.some((effect) => effect.includes("RejectPublication"))) {
  throw new Error("contract must record the exact reply reject-publication effect");
}
if (contract.reply_rejected_semantics?.target !== "ReplyStatus::Rejected") {
  throw new Error("RejectPublication must bind to Forum ReplyStatus::Rejected");
}
if (!contract.reply_rejected_semantics?.unpublished_distinction?.includes("remains unsupported")) {
  throw new Error("RejectPublication contract must keep neutral Unpublished distinct");
}
if (contract.reply_removed_semantics?.owner_helper !== "ReplyService::remove_in_tx") {
  throw new Error("Removed must bind to the complete Forum reply removal owner helper");
}
if (!contract.deferred_effects.some((effect) => effect.includes("SetVisibility Unpublished"))) {
  throw new Error("contract must keep Unpublished deferred");
}
if (contract.deferred_effects.some((effect) => effect.includes("SetVisibility Removed"))) {
  throw new Error("contract must not keep Removed deferred after exact owner-path reuse");
}
if (contract.deferred_effects.some((effect) => effect.includes("RejectPublication"))) {
  throw new Error("contract must not keep RejectPublication deferred after exact owner mapping");
}
if (!contract.forbidden.includes("mapping SetVisibility Unpublished to ReplyStatus::Rejected")) {
  throw new Error("contract must forbid collapsing Unpublished into RejectPublication state");
}
if (!contract.forbidden.includes("direct rustok-moderation owner dependency")) {
  throw new Error("contract must forbid a direct Moderation owner dependency");
}
if (!contract.forbidden.includes("reusing Reactions or content revision as the Moderation subject clock")) {
  throw new Error("contract must forbid reuse of Reactions/content revision as Moderation clock");
}
if (!contract.not_claimed.includes("all neutral Moderation effect coverage")) {
  throw new Error("completed bounded FORUM-19 slice must not claim every neutral Moderation effect");
}
for (const staleClaim of [
  "authorized admin transport or UI for operator recovery",
  "retained operator recovery execution evidence",
  "retained host composition execution evidence",
  "retained runtime decision application execution",
  "retained multi-host scheduler race and graceful shutdown evidence",
  "PostgreSQL concurrency evidence",
  "SQLite evidence",
]) {
  if (contract.not_claimed.includes(staleClaim)) {
    throw new Error(`FORUM-19 contract still carries completed evidence as not claimed: ${staleClaim}`);
  }
}

console.log("Forum Moderation subject adapter ownership guard passed");
