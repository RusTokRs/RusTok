import fs from "node:fs";

const forumCargo = fs.readFileSync("crates/rustok-forum/Cargo.toml", "utf8");
const forumLib = fs.readFileSync("crates/rustok-forum/src/lib.rs", "utf8");
const migrations = fs.readFileSync("crates/rustok-forum/src/migrations/mod.rs", "utf8");
const revisionMigration = fs.readFileSync(
  "crates/rustok-forum/src/migrations/m20260807_000027_add_forum_moderation_subject_revisions.rs",
  "utf8",
);
const adapter = fs.readFileSync("crates/rustok-forum/src/moderation_subject.rs", "utf8");
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
  "forum.moderation_subject_revision_not_advanced",
]) {
  requireText(adapter, marker, `Forum Moderation adapter is missing ${marker}`);
}

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
if (!contract.forbidden.includes("direct rustok-moderation owner dependency")) {
  throw new Error("contract must forbid a direct Moderation owner dependency");
}
if (!contract.forbidden.includes("reusing Reactions or content revision as the Moderation subject clock")) {
  throw new Error("contract must forbid reuse of Reactions/content revision as Moderation clock");
}
if (!contract.not_claimed.includes("complete FORUM-19 effect coverage")) {
  throw new Error("bounded slice must not claim complete FORUM-19 coverage");
}

console.log("Forum Moderation subject adapter ownership guard passed");
