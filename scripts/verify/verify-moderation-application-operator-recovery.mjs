import fs from "node:fs";

const recovery = fs.readFileSync(
  "crates/rustok-moderation/src/application_recovery.rs",
  "utf8",
);
const domain = fs.readFileSync("crates/rustok-moderation/src/domain.rs", "utf8");
const moderationLib = fs.readFileSync("crates/rustok-moderation/src/lib.rs", "utf8");
const dispatcher = fs.readFileSync(
  "crates/rustok-moderation/src/application_dispatch.rs",
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
  "RequeueModerationApplicationCommand",
  "ReconcileLegacyModerationApplicationCommand",
  "ModerationApplicationRecoveryRecord",
]) {
  requireText(domain, marker, `Moderation recovery domain contract is missing ${marker}`);
}

for (const marker of [
  "operator_requeue_application_replay_safe",
  "operator_reconcile_legacy_application_replay_safe",
  'const OP_REQUEUE_APPLICATION: &str = "operator_requeue_application"',
  'const OP_RECONCILE_LEGACY_APPLICATION: &str = "operator_reconcile_legacy_application"',
  "PortActorKind::User",
  "required_idempotency_key(&context)",
  "request_hash(",
  "replay_existing(",
  "ModerationReceiptAdmission::Replay",
  "ModerationReceiptAdmission::New",
  "expected_case_revision",
  "ModerationApplicationOperationStatus::Rejected",
  "ModerationApplicationOperationStatus::OperatorReview",
  "ModerationApplicationOperationStatus::Retryable",
  "ModerationCaseStatus::ApplyingDecision",
  '"application_operator_requeued"',
  '"case_application_requeued"',
  '"application_legacy_terminal_reconciled"',
  '"case_legacy_terminal_reconciled"',
  "ModerationApplicationOperationStatus::Applied => Some(ModerationCaseStatus::Closed)",
  "Some(ModerationCaseStatus::Escalated)",
  "application recovery identity does not match immutable decision and case facts",
  "terminal moderation application must not retain a live lease tuple",
  "applied decisions must never be requeued",
]) {
  requireText(recovery, marker, `Moderation application recovery is missing ${marker}`);
}

for (const forbidden of [
  "ModerationSubjectAdapterRegistry",
  "apply_moderation_decision(",
  "dispatch_application_operation_once(",
  "rustok_reactions",
  "ReactionBar",
]) {
  forbidText(
    recovery,
    forbidden,
    `Operator recovery must not dispatch domains or absorb unrelated ownership: ${forbidden}`,
  );
}

requireText(
  dispatcher,
  ".with_idempotency_key(decision_id.to_string())",
  "Operator recovery must not change immutable decision UUID domain idempotency",
);
requireText(
  moderationLib,
  "assert_eq!(module.migrations().len(), 4);",
  "Operator recovery must not add a schema migration",
);
requireText(
  moderationLib,
  "pub mod application_recovery;",
  "Moderation owner must compile the application recovery module",
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
  "Forum must remain free of the Reactions presentation dependency",
);

console.log("Moderation application operator recovery source guard passed");
