import fs from "node:fs";

const transport = fs.readFileSync(
  "apps/server/src/graphql/moderation_recovery.rs",
  "utf8",
);
const graphqlMod = fs.readFileSync("apps/server/src/graphql/mod.rs", "utf8");
const schema = fs.readFileSync("apps/server/src/graphql/schema.rs", "utf8");
const serverCargo = fs.readFileSync("apps/server/Cargo.toml", "utf8");
const ports = fs.readFileSync("crates/rustok-moderation/src/ports.rs", "utf8");
const caseOpen = fs.readFileSync(
  "crates/rustok-moderation/src/commands/case_open.rs",
  "utf8",
);

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

function forbidText(source, needle, message) {
  if (source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "pub struct ModerationRecoveryMutation",
  "requeue_moderation_application",
  "reconcile_legacy_moderation_application",
  "create_moderation_rereview",
  'const MODULE_SLUG: &str = "moderation"',
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "auth.tenant_id != tenant.id",
  "auth.is_human_user_principal()",
  "Permission::MODERATION_CASES_OVERRIDE",
  "has_effective_permission",
  "idempotency_key.is_nil()",
  "RECOVERY_PORT_DEADLINE",
  ".with_idempotency_key(idempotency_key.to_string())",
  "context.with_claim(permission.to_string())",
  "ModerationRecoveryCommandPort::requeue_application",
  "ModerationRecoveryCommandPort::reconcile_legacy_application",
  "ModerationApplicationRecoveryPayload",
  "ModerationRereviewPayload",
]) {
  requireText(transport, marker, `Moderation recovery GraphQL transport is missing ${marker}`);
}

for (const forbidden of [
  "operator_requeue_application_replay_safe",
  "operator_reconcile_legacy_application_replay_safe",
  "dispatch_application_operation_once",
]) {
  forbidText(
    transport,
    forbidden,
    `GraphQL recovery transport must enter only through public owner ports: ${forbidden}`,
  );
}

for (const rereviewMarker of [
  "fresh_subject_revision",
  "source_decision_id",
  "ModerationReadPort::read_decision",
  "ModerationReadPort::read_case",
  "ModerationCaseStatus::Escalated",
  "fresh_subject_revision <= source_case.subject.revision",
  "let mut fresh_subject = source_case.subject.clone()",
  "fresh_subject.revision = fresh_subject_revision",
  "scope: source_case.scope.clone()",
  "queue_key: source_case.queue_key.clone()",
  "policy_id: source_case.policy_id",
  "policy_version: source_case.policy_version",
  "report_ids: Vec::new()",
  '"operator_rereview"',
  '"root_idempotency_key"',
  '"request_hash"',
  '"source_case_id"',
  '"source_decision_id"',
  '"source_subject_revision"',
  '"fresh_subject_revision"',
  "rereview_request_hash",
  "Sha256::digest",
  "require_owned_rereview_case",
  "ModerationCommandPort::open_case",
  "ModerationCommandPort::assign_case",
  "ModerationCommandPort::decide_case",
  "rereview_step_context",
  '"open"',
  '"assign"',
  '"decide"',
  'format!("{root_idempotency_key}:rereview:{step}")',
]) {
  requireText(
    transport,
    rereviewMarker,
    `Moderation rereview workflow is missing ${rereviewMarker}`,
  );
}

for (const forbiddenRereview of [
  "source_case.subject.id =",
  "source_case.subject.module =",
  "source_case.subject.kind =",
  "source_decision.subject_revision =",
]) {
  forbidText(
    transport,
    forbiddenRereview,
    `Rereview must not mutate historical identity: ${forbiddenRereview}`,
  );
}

requireText(
  caseOpen,
  "if created || !command.report_ids.is_empty()",
  "Deduplicated open with no reports must not emit a reports-attached audit fact",
);
requireText(
  caseOpen,
  '"reports_attached"',
  "Real report attachment must retain the canonical audit event",
);

requireText(
  transport,
  "Permission::FORUM_TOPICS_MODERATE",
  "Transport guard must retain explicit evidence that Forum moderation permission is insufficient",
);
requireText(
  transport,
  "Permission::FORUM_REPLIES_MODERATE",
  "Transport guard must retain explicit evidence that Forum reply moderation permission is insufficient",
);
requireText(
  graphqlMod,
  '#[cfg(feature = "mod-moderation")]\npub mod moderation_recovery;',
  "Server GraphQL module must feature-gate Moderation recovery transport",
);
requireText(
  schema,
  '#[cfg(feature = "mod-moderation")]\nuse super::moderation_recovery::ModerationRecoveryMutation;',
  "Server schema must import the Moderation recovery mutation only with mod-moderation",
);
requireText(
  schema,
  '#[cfg(feature = "mod-moderation")] ModerationRecoveryMutation,',
  "Server schema must merge the Moderation recovery mutation only with mod-moderation",
);
requireText(
  serverCargo,
  'mod-moderation = ["dep:rustok-moderation", "rustok-distribution/mod-moderation"]',
  "Server mod-moderation feature must retain the Moderation owner dependency",
);
requireText(
  ports,
  "pub trait ModerationRecoveryCommandPort",
  "Moderation owner must retain the dedicated recovery command port",
);
requireText(
  ports,
  "moderation_cases:override",
  "Moderation recovery port must retain its dedicated permission boundary",
);

console.log("Moderation recovery and rereview GraphQL transport source guard passed");