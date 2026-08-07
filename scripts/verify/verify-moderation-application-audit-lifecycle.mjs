import fs from "node:fs";

const application = fs.readFileSync("crates/rustok-moderation/src/application.rs", "utf8");
const dispatcher = fs.readFileSync(
  "crates/rustok-moderation/src/application_dispatch.rs",
  "utf8",
);
const scheduler = fs.readFileSync(
  "crates/rustok-moderation/src/application_scheduler.rs",
  "utf8",
);
const moderationLib = fs.readFileSync("crates/rustok-moderation/src/lib.rs", "utf8");
const forumCargo = fs.readFileSync("crates/rustok-forum/Cargo.toml", "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

function forbidText(source, needle, message) {
  if (source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "TransactionTrait",
  "self.database().begin().await?",
  "transition_case_status_in_transaction",
  "ModerationCaseStatus::Decided",
  "ModerationCaseStatus::ApplyingDecision",
  '"case_application_started"',
  '"application_attempt_claimed"',
  '"application_retry_scheduled"',
  '"application_applied"',
  '"application_rejected"',
  '"application_operator_review"',
  '"case_closed"',
  '"case_escalated"',
  "ModerationCaseStatus::Closed",
  "ModerationCaseStatus::Escalated",
  "moderation_case::Column::ClosedAt",
  "moderation_case::Column::ActiveDeduplicationKey",
  "append_event(",
  "transaction.commit().await?",
]) {
  requireText(application, marker, `Moderation application audit lifecycle is missing ${marker}`);
}

for (const marker of [
  "claim_application_operation(",
  "mark_application_retryable(",
  "mark_application_rejected(",
  "mark_application_operator_review(",
  "mark_application_applied(",
]) {
  requireText(dispatcher, marker, `One-attempt dispatcher must keep using owner primitive ${marker}`);
}

for (const forbidden of [
  "moderation_case::",
  "append_event(",
  '"case_closed"',
  '"case_escalated"',
]) {
  forbidText(
    dispatcher,
    forbidden,
    `Dispatcher must not duplicate application audit lifecycle ownership: ${forbidden}`,
  );
}

for (const forbidden of [
  "mark_application_retryable(",
  "mark_application_rejected(",
  "mark_application_operator_review(",
  "mark_application_applied(",
  "moderation_case::",
  "append_event(",
]) {
  forbidText(
    scheduler,
    forbidden,
    `Shared scheduler must remain outside Moderation lifecycle finalization: ${forbidden}`,
  );
}

requireText(
  moderationLib,
  "assert_eq!(module.migrations().len(), 4);",
  "Application audit lifecycle must not add a schema migration",
);

forbidText(
  forumCargo,
  "rustok-moderation =",
  "Forum must remain free of the Moderation owner dependency",
);
forbidText(
  forumCargo,
  "rustok-reactions =",
  "Forum must remain free of the Reactions owner dependency",
);
forbidText(
  forumCargo,
  "rustok-reactions-storefront",
  "Forum owner must remain free of the Reactions presentation dependency",
);

for (const forbidden of [
  "rustok_reactions",
  "ReactionBar",
  "reactionSnapshot",
  "applyReaction",
]) {
  forbidText(
    application,
    forbidden,
    `Moderation application lifecycle must not absorb Reactions behavior: ${forbidden}`,
  );
}

console.log("Moderation application audit lifecycle source guard passed");
