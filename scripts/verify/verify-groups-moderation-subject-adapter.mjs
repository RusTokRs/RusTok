import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");
const requireText = (source, needle, message) => {
  if (!source.includes(needle)) throw new Error(message);
};
const forbidText = (source, needle, message) => {
  if (source.includes(needle)) throw new Error(message);
};

const cargo = read("crates/rustok-groups/Cargo.toml");
const lib = read("crates/rustok-groups/src/lib.rs");
const adapter = read("crates/rustok-groups/src/moderation_subject.rs");
const ownerMutation = read("crates/rustok-groups/src/membership_enforcement_command.rs");
const ownerLock = read("crates/rustok-groups/src/membership_enforcement_transaction.rs");
const neutralModel = read("crates/rustok-moderation-api/src/model.rs");
const dispatcher = read("crates/rustok-moderation/src/application_dispatch.rs");
const plan = read("crates/rustok-groups/docs/implementation-plan.md");
const moderationPlan = read("crates/rustok-moderation/docs/implementation-plan.md");
const contract = JSON.parse(
  read("crates/rustok-groups/contracts/groups-effective-membership-access.json"),
);

for (const marker of [
  'rustok-moderation-api = { path = "../rustok-moderation-api" }',
  "rustok-outbox.workspace = true",
]) {
  requireText(cargo, marker, `Groups moderation adapter dependency missing: ${marker}`);
}
forbidText(
  cargo,
  "rustok-moderation =",
  "Groups must not depend on the Moderation persistence owner",
);

for (const marker of [
  "mod moderation_subject;",
  "pub use moderation_subject::GroupsModerationSubjectAdapterFactory;",
  "register_moderation_subject_adapter_factory",
  "moderation_subject::GroupsModerationSubjectAdapterFactory",
]) {
  requireText(lib, marker, `Groups module registration missing ${marker}`);
}

for (const marker of [
  'pub const GROUPS_MODERATION_MODULE: &str = "groups"',
  "ModerationSubjectKind::GroupMembership",
  "ModerationSubjectCommandPort for GroupsModerationSubjectAdapter",
  "context.require_policy(PortCallPolicy::write())?",
  'const MODERATION_DISPATCH_ACTOR: &str = "rustok-moderation"',
  "moderation_scope_from_claims(&context.claims)",
  "GroupsModerationReceiptRequest",
  "scope: &scope",
  "command: &command",
  "idempotency::admit",
  "idempotency::OwnerOperationScope::Tenant(tenant_id)",
  "idempotency::complete(&transaction, lease, &application)",
  "lock_membership_enforcement_target_by_id_for_update",
  "target.group.id != group_id",
  "target.membership.revision != command.subject.revision",
  "ModerationDecisionEffectAction::SuspendSubject",
  "GroupMembershipEnforcementSourceKind::ModerationDecision",
  "provenance.moderation_decision_id = Some(command.decision_id);",
  "provenance.moderation_decision_hash = Some(command.decision_hash.clone());",
  "apply_membership_suspension_in_tx",
  "result.membership_revision <= command.subject.revision",
  '"groups.moderation_effect_unsupported"',
  '"groups.moderation_scope_mismatch"',
]) {
  requireText(adapter, marker, `Groups moderation adapter is missing ${marker}`);
}

const admitIndex = adapter.indexOf("idempotency::admit");
const lockIndex = adapter.indexOf("let target = lock_membership_enforcement_target_by_id_for_update");
if (admitIndex < 0 || lockIndex < 0 || admitIndex >= lockIndex) {
  throw new Error("Groups moderation producer receipt admission must precede membership subject reads");
}
for (const forbidden of [
  "rustok_moderation::",
  "moderation_cases",
  "moderation_decisions",
  "moderation_reports",
]) {
  forbidText(adapter, forbidden, `Groups adapter crosses Moderation owner persistence: ${forbidden}`);
}

for (const marker of [
  "pub(crate) async fn apply_membership_suspension_in_tx",
  "validate_mutation_identity",
  "validate_provenance",
  "moderation-driven membership enforcement requires decision identity",
]) {
  requireText(ownerMutation, marker, `Groups owner mutation seam missing ${marker}`);
}
requireText(
  ownerLock,
  "lock_membership_enforcement_target_by_id_for_update",
  "Groups adapter must retain the receipt-first membership-ID owner lock primitive",
);

for (const marker of [
  'pub const MODERATION_SCOPE_CLAIM_PREFIX: &str = "moderation.scope.v1:"',
  "pub fn moderation_scope_claim",
  "pub fn moderation_scope_from_claims",
  "DuplicateClaim",
  "InvalidClaim",
]) {
  requireText(neutralModel, marker, `Neutral scope claim contract missing ${marker}`);
}
const commandStart = neutralModel.indexOf("pub struct ApplyModerationDecisionCommand");
const commandEnd = neutralModel.indexOf("pub struct ModerationDecisionApplication", commandStart);
if (commandStart < 0 || commandEnd < 0) throw new Error("neutral command boundary missing");
const commandBlock = neutralModel.slice(commandStart, commandEnd);
forbidText(
  commandBlock,
  "pub scope:",
  "Historical ApplyModerationDecisionCommand receipt shape must not be extended with scope",
);

for (const marker of [
  "moderation_scope_claim(&case.scope)",
  ".with_claim(scope_claim)",
  "application_port_context(tenant_id, decision_id, lease_token, scope_claim)",
]) {
  requireText(dispatcher, marker, `Moderation dispatcher scope propagation missing ${marker}`);
}

if (contract.remaining_paths?.includes("moderation_subject_adapter")) {
  throw new Error("Groups contract still lists the source-complete moderation adapter as remaining");
}
if (!contract.converted_source_paths?.moderation_subject_adapter?.includes(
  "crates/rustok-groups/src/moderation_subject.rs",
)) {
  throw new Error("Groups contract does not retain the moderation adapter source path");
}
if (
  contract.evidence?.moderation_subject_adapter_static_boundary !==
  "scripts/verify/verify-groups-moderation-subject-adapter.mjs"
) {
  throw new Error("Groups contract is missing the adapter static boundary");
}
if (contract.evidence?.moderation_subject_adapter_runtime !== null) {
  throw new Error("Groups contract must keep runtime adapter evidence explicitly open");
}
for (const marker of [
  "### Source-complete moderation adapter",
  "Runtime/replay/race",
  "GROUPS-07 | in_progress",
  "verify-groups-moderation-subject-adapter.mjs",
]) {
  requireText(plan, marker, `Groups canonical plan is missing ${marker}`);
}
requireText(
  moderationPlan,
  "For Groups compatibility now present in source:",
  "Moderation plan must retain the Groups reference-adapter handoff",
);
requireText(
  moderationPlan,
  "historical domain receipt request digests",
  "Moderation plan must document scope propagation compatibility",
);

console.log("Groups moderation membership adapter source guard passed");
