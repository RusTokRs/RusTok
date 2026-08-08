import fs from "node:fs";

const test = fs.readFileSync(
  "crates/rustok-distribution/tests/forum_moderation_lost_response_postgres.rs",
  "utf8",
);
const docs = fs.readFileSync(
  "crates/rustok-moderation/docs/forum-lost-response-postgres-contract.md",
  "utf8",
);
const distributionCargo = fs.readFileSync(
  "crates/rustok-distribution/Cargo.toml",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

for (const marker of [
  '#![cfg(all(feature = "mod-forum", feature = "mod-moderation"))]',
  'OutboxModule.migrations()',
  'TaxonomyModule.migrations()',
  'ForumModule.migrations()',
  'rustok_moderation::ModerationModule.migrations()',
  'ReplyService::new',
  'ForumModerationSubjectAdapterFactory::reply()',
  'factory.build(&HostRuntimeContext::new(database.db.clone()))',
  'ModerationDecisionKind::Hide',
  'ModerationVisibilityState::Hidden',
  'application_context(seed.tenant_id, decision.id, "lost-response-first")',
  'ModerationApplicationOperationStatus::Pending',
  'ModerationCaseStatus::Decided',
  'dispatch_application_operation_once',
  'ModerationApplicationOperationStatus::Applied',
  'ModerationCaseStatus::Closed',
  'forum_reply_moderation_subject_revisions',
  "event_type = 'forum.reply.status_changed'",
  'owner_operation_receipts',
  "owner_slug = 'forum'",
  'APPLY_OPERATION',
  'assert_eq!(reply_revision(&database.db, &seed).await?, revision_after_first)',
]) {
  requireText(test, marker, `Forum/Moderation lost-response PostgreSQL contract is missing ${marker}`);
}

for (const marker of [
  'mod-moderation = ["dep:rustok-moderation"]',
  'mod-forum = ["dep:rustok-forum"',
]) {
  requireText(
    distributionCargo,
    marker,
    `Distribution feature graph no longer supports the cross-owner evidence boundary: ${marker}`,
  );
}

for (const marker of [
  'response-loss window',
  'before** the adapter reaches its subject-revision fence',
  'exactly one completed Forum owner-operation receipt',
  'producer mutation was not executed twice',
  '--features mod-forum,mod-moderation',
]) {
  requireText(docs, marker, `Forum/Moderation lost-response handoff is missing ${marker}`);
}

console.log("Forum/Moderation lost-response PostgreSQL source guard passed");
