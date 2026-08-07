import fs from "node:fs";

const cargo = fs.readFileSync("crates/rustok-moderation/Cargo.toml", "utf8");
const moderationLib = fs.readFileSync("crates/rustok-moderation/src/lib.rs", "utf8");
const scheduler = fs.readFileSync(
  "crates/rustok-moderation/src/application_scheduler.rs",
  "utf8",
);
const dispatcher = fs.readFileSync(
  "crates/rustok-moderation/src/application_dispatch.rs",
  "utf8",
);
const appRuntime = fs.readFileSync("apps/server/src/services/app_runtime.rs", "utf8");
const forumCargo = fs.readFileSync("crates/rustok-forum/Cargo.toml", "utf8");

function requireText(source, needle, message) {
  if (!source.includes(needle)) throw new Error(message);
}

function forbidText(source, needle, message) {
  if (source.includes(needle)) throw new Error(message);
}

for (const marker of [
  "rustok-runtime.workspace = true",
]) {
  requireText(cargo, marker, `Moderation runtime dependency is missing ${marker}`);
}

for (const marker of [
  "mod application_scheduler;",
  "rustok_runtime::ModuleWorkRegistrations",
  "ModerationApplicationWorkRegistration",
  "register_runtime_extensions",
]) {
  requireText(moderationLib, marker, `Moderation module work registration is missing ${marker}`);
}

for (const marker of [
  'MODERATION_APPLICATION_WORKER: &str = "moderation_decision_application"',
  "ModuleWorkRegistration",
  "ModuleWorkSource",
  "ModuleWorkHandler",
  "ModuleWorkScheduler",
  "ModerationSubjectAdapterRegistry",
  "shared_get::<Arc<ModerationSubjectAdapterRegistry>>()",
  "next_due_candidate",
  "ModerationApplicationOperationStatus::Pending",
  "ModerationApplicationOperationStatus::Retryable",
  "ModerationApplicationOperationStatus::Applying",
  "LeaseExpiresAt.lte(now)",
  "dispatch_application_operation_once",
  "MODERATION_APPLICATION_LEASE_OWNER",
  "Ok(ModuleWorkOutcome::Completed)",
]) {
  requireText(scheduler, marker, `Moderation scheduler source is missing ${marker}`);
}

for (const marker of [
  "ModuleWorkScheduler::new()",
  ".register_all(&host, &scheduler)",
  "run_until_stopped",
  "StopHandle",
  "runs_background_workers()",
]) {
  requireText(appRuntime, marker, `Shared server module-work lifecycle is missing ${marker}`);
}

for (const marker of [
  "claim_application_operation(",
  "registry.get(&operation.subject.module, operation.subject.kind)",
  "apply_moderation_decision(context, command)",
  ".with_idempotency_key(decision_id.to_string())",
]) {
  requireText(dispatcher, marker, `One-attempt dispatcher must remain authoritative for ${marker}`);
}

for (const forbidden of [
  "tokio::spawn",
  "tokio::time::interval",
  "while let",
  "loop {",
  "apply_moderation_decision(",
  ".with_idempotency_key(",
  "mark_application_applied(",
  "mark_application_retryable(",
  "mark_application_rejected(",
  "mark_application_operator_review(",
]) {
  forbidText(
    scheduler,
    forbidden,
    `Moderation scheduler must not duplicate host loop/dispatcher ownership: ${forbidden}`,
  );
}

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
    scheduler,
    forbidden,
    `Moderation scheduler must not absorb Reactions behavior: ${forbidden}`,
  );
}

console.log("Moderation application module-work scheduler source guard passed");
