import fs from "node:fs";

const dispatch = fs.readFileSync(
  "crates/rustok-moderation/src/application_dispatch.rs",
  "utf8",
);
const application = fs.readFileSync(
  "crates/rustok-moderation/src/application.rs",
  "utf8",
);
const moderationLib = fs.readFileSync(
  "crates/rustok-moderation/src/lib.rs",
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
  "dispatch_application_operation_once",
  "claim_application_operation(",
  "DEFAULT_APPLICATION_LEASE_SECONDS",
  "reconstruct_application_command",
  "get_decision(tenant_id, operation.decision_id)",
  "get_case(tenant_id, operation.case_id)",
  "decision.decision_hash != operation.decision_hash",
  "case.subject != operation.subject",
  "decision.effect.ok_or_else",
  "validate_for_decision_kind",
  "registry.get(&operation.subject.module, operation.subject.kind)",
  "apply_moderation_decision(context, command)",
  "finish_adapter_success",
  "mark_application_applied",
  "ModerationError::ApplicationEvidenceMismatch",
  "EVIDENCE_INVALID_CODE",
  "mark_application_retryable",
  "mark_application_rejected",
  "mark_application_operator_review",
  "error.retryable",
  "PortErrorKind::Conflict | PortErrorKind::InvariantViolation",
  "application_retry_delay_seconds",
]) {
  requireText(dispatch, marker, `one-attempt dispatcher is missing ${marker}`);
}

for (const marker of [
  'PortActor::service(APPLICATION_DISPATCH_ACTOR)',
  '.with_idempotency_key(decision_id.to_string())',
  '.with_causation_id(decision_id.to_string())',
  'APPLICATION_ADAPTER_DEADLINE_SECONDS: u64 = 30',
  'APPLICATION_RETRY_BASE_SECONDS: i64 = 5',
  'APPLICATION_RETRY_MAX_SECONDS: i64 = 300',
  'ADAPTER_MISSING_CODE: &str = "moderation.application_adapter_missing"',
  'EVIDENCE_INVALID_CODE: &str = "moderation.application_evidence_invalid"',
]) {
  requireText(dispatch, marker, `dispatch context/retry contract is missing ${marker}`);
}

requireText(
  application,
  "LeaseToken.eq(lease_token)",
  "application completion must remain fenced by the UUID lease token",
);
requireText(
  application,
  "LeaseExpiresAt.gt(now)",
  "application completion must remain fenced by an unexpired lease",
);
requireText(
  moderationLib,
  "pub mod application_dispatch;",
  "dispatcher module must be exported by rustok-moderation",
);

for (const forbidden of [
  "for loop",
  "while let",
  "tokio::spawn",
  "tokio::time::interval",
  "sleep(",
]) {
  forbidText(
    dispatch,
    forbidden,
    `one-attempt dispatcher must not absorb the background scheduler: ${forbidden}`,
  );
}

forbidText(
  forumCargo,
  "rustok-moderation =",
  "Forum must remain free of the Moderation owner dependency",
);
for (const forbidden of [
  "rustok_reactions",
  "ReactionBar",
  "applyReaction",
  "reactionSnapshot",
]) {
  forbidText(
    dispatch,
    forbidden,
    `Moderation dispatcher must not absorb Reactions behavior: ${forbidden}`,
  );
}

console.log("Moderation one-attempt application dispatch source guard passed");
