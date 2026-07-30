#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";

const failures = [];
const read = (path) => readFileSync(path, "utf8");
const fail = (message) => failures.push(message);
const requireFile = (path) => {
  if (!existsSync(path)) fail(`${path}: missing required file`);
};
const requireText = (text, marker, label) => {
  if (!text.includes(marker)) fail(`${label}: missing ${marker}`);
};
const forbidText = (text, marker, label) => {
  if (text.includes(marker)) fail(`${label}: forbidden ${marker}`);
};

const files = {
  aiCargo: "crates/rustok-ai/Cargo.toml",
  aiPort: "crates/rustok-ai/src/ports.rs",
  aiLedger: "crates/rustok-ai/src/structured.rs",
  aiAccounting: "crates/rustok-ai/src/accounting.rs",
  aiRouter: "crates/rustok-ai/src/router.rs",
  aiStructuredRuntime: "crates/rustok-ai/src/structured_runtime.rs",
  aiStructuredLive: "crates/rustok-ai/src/structured_live_tests.rs",
  aiStructuredResult: "crates/rustok-ai/src/structured_result.rs",
  aiService: "crates/rustok-ai/src/service.rs",
  aiScheduler: "crates/rustok-ai/src/scheduler.rs",
  aiGraphqlQuery: "crates/rustok-ai/src/graphql/query.rs",
  aiGraphqlMutation: "crates/rustok-ai/src/graphql/mutation.rs",
  aiNativeAdmin:
    "crates/rustok-ai/admin/src/transport/native_server_adapter.rs",
  aiMigration:
    "crates/rustok-ai/src/migrations/m20260729_000001_structured_execution.rs",
  aiLocalePolicy: "crates/rustok-ai/src/service/helpers.rs",
  translationCargo: "crates/rustok-translation/Cargo.toml",
  translationPort: "crates/rustok-translation/src/machine.rs",
  translationService: "crates/rustok-translation/src/machine_service.rs",
  translationMachineEntity:
    "crates/rustok-translation/src/entities/machine_operation.rs",
  translationMemoryBindingEntity:
    "crates/rustok-translation/src/entities/machine_memory_binding.rs",
  translationCancellationEntity:
    "crates/rustok-translation/src/entities/machine_cancellation.rs",
  translationRecoveryEntity:
    "crates/rustok-translation/src/entities/machine_recovery.rs",
  translationMachineMigration:
    "crates/rustok-translation/src/migrations/m20260729_000009_create_translation_machine_operations.rs",
  translationGraphqlMutation:
    "crates/rustok-translation/src/graphql/mutation.rs",
  translationNativeAdmin:
    "crates/rustok-translation/admin/src/transport/native_server_adapter.rs",
  distributionCargo: "crates/rustok-distribution/Cargo.toml",
  distributionSource: "crates/rustok-distribution/src/lib.rs",
  serverCargo: "apps/server/Cargo.toml",
  serverComposition: "apps/server/src/services/module_event_dispatcher.rs",
  adapterCargo: "crates/rustok-ai-translation/Cargo.toml",
  adapterSource: "crates/rustok-ai-translation/src/lib.rs",
  adapterReadme: "crates/rustok-ai-translation/README.md",
  adapterPlan: "crates/rustok-ai-translation/docs/implementation-plan.md",
};

for (const file of Object.values(files)) requireFile(file);

const aiCargo = read(files.aiCargo);
const aiPort = read(files.aiPort);
const aiLedger = read(files.aiLedger);
const aiAccounting = read(files.aiAccounting);
const aiRouter = read(files.aiRouter);
const aiStructuredRuntime = read(files.aiStructuredRuntime);
const aiStructuredLive = read(files.aiStructuredLive);
const aiStructuredResult = read(files.aiStructuredResult);
const aiService = read(files.aiService);
const aiScheduler = read(files.aiScheduler);
const aiGraphqlQuery = read(files.aiGraphqlQuery);
const aiGraphqlMutation = read(files.aiGraphqlMutation);
const aiNativeAdmin = read(files.aiNativeAdmin);
const aiMigration = read(files.aiMigration);
const aiLocalePolicy = read(files.aiLocalePolicy);
const translationCargo = read(files.translationCargo);
const translationPort = read(files.translationPort);
const translationService = read(files.translationService);
const translationMachineEntity = read(files.translationMachineEntity);
const translationMemoryBindingEntity = read(
  files.translationMemoryBindingEntity,
);
const translationCancellationEntity = read(files.translationCancellationEntity);
const translationRecoveryEntity = read(files.translationRecoveryEntity);
const translationMachineMigration = read(files.translationMachineMigration);
const translationGraphqlMutation = read(files.translationGraphqlMutation);
const translationNativeAdmin = read(files.translationNativeAdmin);
const distributionCargo = read(files.distributionCargo);
const distributionSource = read(files.distributionSource);
const serverCargo = read(files.serverCargo);
const serverComposition = read(files.serverComposition);
const adapterCargo = read(files.adapterCargo);
const adapterSource = read(files.adapterSource);
const adapterReadme = read(files.adapterReadme);
const adapterPlan = read(files.adapterPlan);
const hasDependency = (manifest, dependency) =>
  new RegExp(`^\\s*${dependency}(?:\\.workspace\\s*=|\\s*=)`, "m").test(
    manifest,
  );

if (hasDependency(aiCargo, "rustok-translation")) {
  fail("rustok-ai must not depend on rustok-translation");
}
if (hasDependency(translationCargo, "rustok-ai")) {
  fail("rustok-translation must not depend on rustok-ai");
}
if (!hasDependency(adapterCargo, "rustok-ai")) {
  fail("rustok-ai-translation must depend on rustok-ai");
}
if (!hasDependency(adapterCargo, "rustok-translation")) {
  fail("rustok-ai-translation must depend on rustok-translation");
}

for (const marker of [
  "pub trait AiStructuredTaskPort",
  "PortCallPolicy::write()",
  "AiStructuredTaskExecution",
  "AiStructuredTaskExecutionKey",
  "AiStructuredTaskUsage",
  "AiStructuredTaskDescriptor",
  "AiStructuredTaskCatalog",
  "async fn status(",
  "async fn resolve(",
  "async fn cancel(",
  "async fn cancel_by_key(",
])
  requireText(aiPort, marker, "AI structured-task port");

for (const marker of [
  "IdempotencyKey",
  "RequestDigest",
  "InputDigest",
  "EvidenceDigest",
  "LeaseExpiresAt",
  "CancelIdempotencyKey",
  "ai_structured_attempts",
  "ai_structured_budgets",
  "ai_structured_provider_policies",
  "ai_structured_reservations",
  "ai_structured_results",
  "ai_structured_cancellation_intents",
  "uq_ai_structured_cancellation_execution_key",
])
  requireText(aiMigration, marker, "AI structured-task migration");
for (const marker of [
  "Executions::InputPayload",
  "Executions::OutputPayload",
  "Executions::RawResponse",
]) {
  forbidText(aiMigration, marker, "content-free structured-task migration");
}
for (const marker of ["ai.structured.idempotency_conflict", "request_cancel"])
  requireText(aiLedger, marker, "AI structured-task ledger");
for (const marker of [
  "put_budget",
  "put_provider_policy",
  "reserve",
  "finalize",
  "cancel_queued",
  "recover_expired",
  "recover_queued_cancellations",
  "settle_reservation",
  "finish_recovered_attempt",
  "begin_attempt",
  "finish_attempt",
  "complete_attempt",
  "price_snapshot_digest",
  "separate_process_recovers_and_reclaims_an_expired_execution",
  "structured_recovery_child_process",
  "RUSTOK_AI_TEST_STRUCTURED_DB_PATH",
  "std::env::current_exe()",
  "attempt.error_code.as_deref(), Some(RECOVERY_ERROR_CODE)",
])
  requireText(aiAccounting, marker, "AI structured-task accounting");
requireText(
  aiRouter,
  "ordered_provider_candidates",
  "AI structured-task routing",
);
for (const marker of [
  "impl AiStructuredTaskPort for DurableAiStructuredTaskPort",
  "validate_descriptor",
  "ordered_provider_candidates",
  "runtime_inference_engine",
  "complete_structured",
  "cancellation_requested",
  "apply_cancellation_intent",
  "put_cancellation_intent",
  "cancel_by_key",
  "DeadlineExceeded",
  "TerminalOutcome::Failed",
  "TerminalOutcome::Cancelled",
  "jsonschema::options()",
  "ai.structured.provider_output_schema_invalid",
  "structured_runtime_preserves_contract_and_accounting_across_failure_paths",
  'assert_eq!(conflict.code, "ai.structured.idempotency_conflict")',
  "assert_eq!(restarted_engine.calls(), 0)",
  "assert_eq!(committed_after_restart, committed_before_restart)",
  "assert_eq!(budget_after_cancellation.reserved_minor_units, 0)",
  'assert_eq!(quota_error.code, "ai.structured.quota_exhausted")',
  "assert_eq!(quota_engine.calls(), 0)",
  "AiStructuredTaskAvailability::Degraded",
  'Some("ai.structured.provider_unavailable")',
])
  requireText(aiStructuredRuntime, marker, "AI structured-task runtime");
for (const marker of [
  "RUSTOK_AI_LIVE_STRUCTURED_PROVIDER_CONFIG_JSON",
  "executes_declared_live_provider_through_durable_structured_runtime",
  '#[ignore = "requires deployment-owned',
  "DurableAiStructuredTaskPort::new",
  "RUSTOK_AI_LIVE_",
  "live structured restart replay",
  "committed_before_restart",
])
  requireText(aiStructuredLive, marker, "AI live structured-runtime probe");
for (const marker of [
  "Aes256Gcm",
  "AiStructuredResultKeyringConfig",
  "result_aad",
  "result_expired",
  "replay_count",
  "RESULT_CLEANUP_BATCH_SIZE",
  ".limit(RESULT_CLEANUP_BATCH_SIZE)",
])
  requireText(
    aiStructuredResult,
    marker,
    "AI encrypted structured-result handoff",
  );
for (const marker of [
  "put_structured_budget_policy",
  "put_structured_provider_policy",
  "list_structured_budget_policies",
  "list_structured_provider_policies",
  "AI_PROVIDERS_MANAGE",
  "AI_PROVIDERS_READ",
])
  requireText(aiService, marker, "AI structured-accounting operator service");
for (const marker of [
  "recover_queued_cancellations",
  "recover_expired",
  "delete_expired_results",
])
  requireText(aiScheduler, marker, "AI structured maintenance scheduling");
for (const marker of [
  "ai_structured_budget_policies",
  "ai_structured_provider_policies",
])
  requireText(aiGraphqlQuery, marker, "AI structured-accounting GraphQL reads");
for (const marker of [
  "put_ai_structured_budget_policy",
  "put_ai_structured_provider_policy",
])
  requireText(
    aiGraphqlMutation,
    marker,
    "AI structured-accounting GraphQL writes",
  );
for (const marker of [
  "ai_put_structured_budget_policy_native",
  "ai_put_structured_provider_policy_native",
])
  requireText(aiNativeAdmin, marker, "AI structured-accounting native writes");
forbidText(
  aiAccounting,
  "TerminalOutcome::Completed",
  "successful terminal transition without encrypted result handoff",
);

for (const marker of [
  "pub trait MachineTranslationPort",
  "pub trait MachineTranslationPortFactory",
  "SharedMachineTranslationPortFactory",
  "machine_translation_port_from_context",
  "MachineTranslationBatchRequest",
  "MachineTranslationExecutionEvidence",
  "review_required",
  "execution_status",
  "recover_batch",
  "cancel_execution",
])
  requireText(translationPort, marker, "Translation machine port");
for (const marker of [
  "pub struct TranslationMachineService",
  "pub async fn generate_proposal",
  "project_glossary",
  "MemoryLookupInput",
  "validate_provider_compatibility",
  "save_proposal",
  "ProposalOrigin::Ai",
  "validate_operation_replay",
  "read_pinned_memory_suggestions",
  "begin_machine_proposal_save",
  "pub struct TranslationMachineControlService",
  "pub async fn cancel_operation",
  "MachineOperationCancelled",
  "read_machine_operation_status",
  "provider_cancellation_status",
  "pub async fn recover_operation",
  "resume_machine_recovery",
  "MachineRecoveryResultUnavailable",
])
  requireText(
    translationService,
    marker,
    "Translation machine proposal command",
  );
for (const marker of [
  "translation_machine_operations",
  "machine_request_digest",
  "adapter_slug",
  "provider_policy_digest",
  "execution_request_digest",
  "prompt_policy_digest",
])
  requireText(
    translationMachineEntity,
    marker,
    "Translation machine operation entity",
  );
for (const marker of [
  "translation_machine_memory_bindings",
  "operation_id",
  "memory_entry_id",
  "batch_ordinal",
  "unit_ordinal",
  "score_basis_points",
])
  requireText(
    translationMemoryBindingEntity,
    marker,
    "Translation machine memory binding entity",
  );
for (const marker of [
  "translation_machine_cancellations",
  "operation_id",
  "reason",
  "idempotency_key",
  "request_hash",
])
  requireText(
    translationCancellationEntity,
    marker,
    "Translation machine cancellation entity",
  );
for (const marker of [
  "translation_machine_recoveries",
  "operation_id",
  "observed_updated_at",
  "request_hash",
])
  requireText(
    translationRecoveryEntity,
    marker,
    "Translation machine recovery entity",
  );
for (const marker of [
  "uq_translation_machine_operations_idempotency",
  "status IN ('registered', 'saving', 'completed', 'cancelled')",
  "execution_id IS NULL",
  "translation_machine_memory_bindings",
  "fk_translation_machine_memory_entry",
  "ForeignKeyAction::Restrict",
  "translation_machine_cancellations",
  "uq_translation_machine_cancellations_idempotency",
  "translation_machine_recoveries",
  "uq_translation_machine_recoveries_idempotency",
])
  requireText(
    translationMachineMigration,
    marker,
    "Translation machine operation migration",
  );
requireText(
  translationGraphqlMutation,
  "generate_machine_translation_proposal",
  "Translation machine GraphQL mutation",
);
requireText(
  translationGraphqlMutation,
  "cancel_machine_translation_operation",
  "Translation machine cancellation GraphQL mutation",
);
requireText(
  translationGraphqlMutation,
  "recover_machine_translation_operation",
  "Translation machine recovery GraphQL mutation",
);
requireText(
  translationNativeAdmin,
  "TranslationAdminOperation::GenerateMachineProposal",
  "Translation machine native command",
);
requireText(
  translationNativeAdmin,
  "TranslationAdminOperation::CancelMachineOperation",
  "Translation machine cancellation native command",
);
requireText(
  translationNativeAdmin,
  "TranslationAdminOperation::RecoverMachineOperation",
  "Translation machine recovery native command",
);
for (const marker of [
  "rustok_ai",
  "AiStructuredTaskPort",
  "AiManagementService",
]) {
  forbidText(translationService, marker, "Translation machine owner service");
}

for (const marker of [
  'MACHINE_TRANSLATION_TASK_SLUG: &str = "machine_translation"',
  "impl MachineTranslationPort for AiMachineTranslationAdapter",
  "AiStructuredTaskRequest",
  "machine_translation_task_descriptor",
  "machine_translation_input_schema_digest",
  "machine_translation_output_schema_digest",
  "machine_translation_port_from_context",
  "AiStructuredTaskExecutionKey",
  "cancel_by_key",
  "recover_batch",
  "output_unit_missing",
  "output_tokens_changed",
  "review_required: true",
])
  requireText(adapterSource, marker, "AI Translation adapter");
for (const marker of [
  'ai-translation = ["mod-ai", "mod-translation", "dep:rustok-ai-translation"]',
])
  requireText(distributionCargo, marker, "AI Translation distribution feature");
for (const marker of [
  "build_runtime_extensions",
  "SharedMachineTranslationPortFactory",
  "AiMachineTranslationPortFactory",
  "selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring",
  "machine_translation_port_from_context",
  ".is_none()",
])
  requireText(
    distributionSource,
    marker,
    "AI Translation distribution composition",
  );
const serverDefaultFeatures = serverCargo.match(
  /default\s*=\s*\[([\s\S]*?)\]/,
);
if (
  !serverDefaultFeatures ||
  !serverDefaultFeatures[1].includes('"ai-translation"')
) {
  fail(
    "production server default features must select the ai-translation bridge",
  );
}
requireText(
  serverComposition,
  "rustok_distribution::build_runtime_extensions(registry)",
  "capability-neutral server composition",
);
for (const marker of [
  "rustok_ai_translation",
  "AiMachineTranslationPortFactory",
  "SharedMachineTranslationPortFactory",
])
  forbidText(serverComposition, marker, "server capability imports");

for (const marker of [
  "sea_orm",
  "AiManagementService",
  "InferenceEngine",
  "apps::server",
  "rustok_product",
  "rustok_media",
  "rustok_blog",
  "rustok_commerce",
  "graphql",
])
  forbidText(adapterSource, marker, "stateless adapter boundary");

requireText(
  aiLocalePolicy,
  'assert!(!task_allows_free_locale("translation"));',
  "legacy translation locale alias removal",
);
forbidText(
  aiLocalePolicy,
  '| "translation"',
  "legacy translation free-locale allow-list",
);
requireText(adapterReadme, "never mutates", "adapter ownership docs");
requireText(
  adapterPlan,
  "Production-profile composition and fail-closed missing-keyring evidence are complete",
  "adapter gate docs",
);
requireText(
  adapterPlan,
  "Deterministic composed runtime evidence covers ordered provider fallback",
  "adapter runtime evidence docs",
);
requireText(
  adapterPlan,
  "Real separate-process recovery evidence uses a file-backed database",
  "adapter process-recovery evidence docs",
);

if (failures.length > 0) {
  console.error("AI Translation boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "AI Translation ownership, dependency, and activation boundary verification passed",
);
