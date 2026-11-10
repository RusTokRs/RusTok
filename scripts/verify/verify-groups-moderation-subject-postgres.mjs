import fs from "node:fs";

const testPath = "crates/modules/rustok-groups/tests/moderation_subject_postgres.rs";
const docPath = "crates/modules/rustok-groups/docs/moderation-subject-postgres-contract.md";
const workflowPath = ".github/workflows/groups-moderation-subject-postgres.yml";

const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docPath, "utf8");
const workflow = fs.readFileSync(workflowPath, "utf8");

const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) {
    throw new Error(`${label} is missing required marker: ${marker}`);
  }
};

for (const marker of [
  'RUSTOK_GROUPS_TEST_POSTGRES_URL',
  'OutboxModule.migrations()',
  'GroupsModule.migrations()',
  'GroupsModerationSubjectAdapterFactory',
  '.build(&HostRuntimeContext::new(db))',
  'PortActor::service(MODERATION_ACTOR)',
  'moderation_scope_claim(&scope)',
  '.with_idempotency_key(decision_id.to_string())',
  'ModerationSubjectKind::GroupMembership',
  'ModerationDecisionKind::SuspendSubject',
  'ModerationDecisionEffectAction::SuspendSubject',
  'source_kind, moderation_decision_id, moderation_decision_hash, actor_kind, actor_id',
  'assert_eq!(source_kind, "moderation_decision")',
  'assert_eq!(actor_kind, "service")',
  'assert_eq!(actor_id, MODERATION_ACTOR)',
  'assert_eq!(group_snapshot(db, fixture).await, (2, 2))',
  'assert_eq!(membership_revision(db, fixture).await, 2)',
  'assert_eq!(replay, first)',
  'same decision id with changed Groups producer request must conflict',
  'tokio::join!(left, right)',
  'groups.moderation_subject_revision_conflict',
  'non-retryable stale moderation decision must replay its failed receipt',
  'CREATE SCHEMA',
  'DROP SCHEMA',
]) {
  requireMarker(test, marker, "Groups moderation PostgreSQL evidence test");
}

// This evidence needs neutral root contracts, not Moderation owner services/entities.
// Bound the allowed imports rather than banning the crate that exports the contracts.
const neutralContracts = new Set([
  "ApplyModerationDecisionCommand", "ModerationDecisionEffect", "ModerationDecisionEffectAction",
  "ModerationDecisionKind", "ModerationReasonCode", "ModerationScopeKind", "ModerationScopeRef",
  "ModerationSubjectAdapterFactory", "ModerationSubjectCommandPort", "ModerationSubjectKind",
  "ModerationSubjectRef", "moderation_scope_claim",
]);
const remaining = test.replace(/\buse\s+rustok_moderation::\{([^}]+)\}\s*;/g, (_, imports) => {
  for (const name of imports.split(",").map((name) => name.trim()).filter(Boolean)) {
    if (!neutralContracts.has(name)) {
      throw new Error(`Groups evidence imports a non-neutral Moderation contract: ${name}`);
    }
  }
  return "";
});
if (remaining.includes("rustok_moderation::")) {
  throw new Error("Groups evidence must use only the explicitly allowed neutral Moderation imports");
}

for (const forbidden of [
  'moderation_cases',
  'moderation_decisions',
  'moderation_application_operations',
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups evidence must not cross Moderation owner persistence: ${forbidden}`);
  }
}

for (const marker of [
  'GROUPS-07',
  'lost-response replay',
  'receipt-before-subject-read',
  'revision fence',
  'member_count',
  'moderation_decision',
  'RUSTOK_GROUPS_TEST_POSTGRES_URL',
  'groups-moderation-subject-postgres.yml',
]) {
  requireMarker(docs, marker, "Groups moderation PostgreSQL evidence doc");
}

for (const marker of [
  'name: Groups Moderation Subject PostgreSQL Evidence',
  'postgres:16',
  'RUSTOK_GROUPS_TEST_POSTGRES_URL:',
  'verify-groups-moderation-subject-postgres.mjs',
  'cargo test --locked -p rustok-groups --test moderation_subject_postgres -- --ignored --nocapture',
  'cargo check --locked -p rustok-groups --tests',
]) {
  requireMarker(workflow, marker, "Groups moderation PostgreSQL evidence workflow");
}

console.log("Groups moderation subject PostgreSQL evidence contract verified.");
