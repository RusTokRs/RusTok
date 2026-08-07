import fs from "node:fs";

const entity = fs.readFileSync(
  "crates/rustok-moderation/src/entities/moderation_application_operation.rs",
  "utf8",
);
const migration = fs.readFileSync(
  "crates/rustok-moderation/src/migrations/m20260807_000004_create_moderation_application_operations.rs",
  "utf8",
);
const migrations = fs.readFileSync(
  "crates/rustok-moderation/src/migrations/mod.rs",
  "utf8",
);
const domain = fs.readFileSync("crates/rustok-moderation/src/domain.rs", "utf8");
const application = fs.readFileSync(
  "crates/rustok-moderation/src/application.rs",
  "utf8",
);
const decide = fs.readFileSync(
  "crates/rustok-moderation/src/commands/case_decide.rs",
  "utf8",
);
const forumCargo = fs.readFileSync("crates/rustok-forum/Cargo.toml", "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

function forbidText(source, needle, message) {
  if (source.includes(needle)) throw new Error(message);
}

for (const marker of [
  'table_name = "moderation_application_operations"',
  "pub decision_id: Uuid",
  "pub tenant_id: Uuid",
  "pub decision_hash: String",
  "pub subject_module: String",
  "pub subject_kind: String",
  "pub subject_id: Uuid",
  "pub subject_revision: i64",
  "pub attempt_count: i32",
  "pub next_attempt_at: DateTimeWithTimeZone",
  "pub lease_token: Option<Uuid>",
  "pub lease_expires_at: Option<DateTimeWithTimeZone>",
  "pub applied_revision: Option<i64>",
]) {
  requireText(entity, marker, `application operation entity is missing ${marker}`);
}

for (const status of [
  "pending",
  "applying",
  "retryable",
  "applied",
  "rejected",
  "operator_review",
]) {
  requireText(domain, `\"${status}\"`, `application status is missing ${status}`);
  requireText(migration, `'${status}'`, `migration status check is missing ${status}`);
}

for (const marker of [
  "m20260807_000004_create_moderation_application_operations",
  'vec!["m20260723_000003_create_moderation_decision_effects"]',
]) {
  requireText(migrations, marker, `migration registry is missing ${marker}`);
}

for (const marker of [
  'name("fk_moderation_application_operations_tenant_decision")',
  "ModerationApplicationOperations::TenantId",
  "ModerationApplicationOperations::DecisionId",
  "ModerationDecisions::TenantId",
  "ModerationDecisions::Id",
  ".string_len(100)",
  'JOIN moderation_decision_effects e',
  'JOIN moderation_cases c',
  "WHERE NOT EXISTS",
  "'pending'",
]) {
  requireText(migration, marker, `application migration is missing ${marker}`);
}

for (const marker of [
  "enqueue_application_operation_in_transaction",
  "list_due_application_operations",
  "claim_application_operation",
  "mark_application_retryable",
  "mark_application_rejected",
  "mark_application_operator_review",
  "mark_application_applied",
  "Uuid::new_v4()",
  "LeaseToken.eq(lease_token)",
  "LeaseExpiresAt.gt(now)",
  "LeaseExpiresAt.lte(now)",
  "AttemptCount).add(1)",
  "MAX_DUE_APPLICATION_OPERATIONS",
  "MAX_APPLICATION_LEASE_SECONDS",
  "MAX_APPLICATION_RETRY_SECONDS",
  "validate_application_evidence",
  "application.subject.revision != operation.subject_revision",
  "application.applied_revision < operation.subject_revision",
]) {
  requireText(application, marker, `application journal is missing ${marker}`);
}

requireText(
  decide,
  "enqueue_application_operation_in_transaction(",
  "case decision must durably enqueue application intent",
);
requireText(
  decide,
  '"application_status": "pending"',
  "case_decided event must record pending application intent",
);
const effectInsert = decide.indexOf("moderation_decision_effect::ActiveModel");
const enqueue = decide.indexOf("enqueue_application_operation_in_transaction(");
const event = decide.indexOf("append_event(");
if (!(effectInsert >= 0 && enqueue > effectInsert && event > enqueue)) {
  throw new Error(
    "decision effect, application enqueue, and case event must remain in one owner transaction in that order",
  );
}

forbidText(
  application,
  "ModerationSubjectAdapterRegistry",
  "application operation foundation must not dispatch adapters yet",
);
forbidText(
  application,
  "apply_moderation_decision(",
  "application operation foundation must not invoke domain adapters yet",
);
forbidText(
  forumCargo,
  "rustok-moderation =",
  "Forum must remain free of the Moderation owner dependency",
);
for (const forbidden of ["ReactionBar", "applyReaction", "reactionSnapshot"]) {
  forbidText(
    application,
    forbidden,
    `Moderation application operation must not absorb Reactions behavior: ${forbidden}`,
  );
}

console.log("Moderation application operation source guard passed");
