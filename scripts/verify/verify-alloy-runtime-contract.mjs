import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-alloy-runtime-contract] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) {
  for (const snippet of snippets) {
    if (!text.includes(snippet)) fail(`${label} missing ${snippet}`);
  }
}
function sameArray(actual, expected, label) {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const contractPath = 'crates/alloy/contracts/alloy-runtime-contract.json';
const evidencePath = 'crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json';
const contract = json(contractPath);
const evidence = json(evidencePath);

if (contract.schema_version !== 1) fail('contract schema_version drift');
if (contract.module !== 'alloy' || contract.crate !== 'alloy' || contract.role !== 'capability_runtime') fail('contract identity drift');
if (contract.status !== 'runtime_hardening') fail('contract status drift');
if (contract.static_gates?.script !== 'scripts/verify/verify-alloy-runtime-contract.mjs') fail('static gate script drift');
if (contract.static_gates?.evidence !== evidencePath) fail('static gate evidence drift');

sameArray(contract.script_list_contract?.ordering, ['name_asc', 'id_asc'], 'script ordering');
sameArray(contract.script_list_contract?.pagination_order, ['filter', 'sort', 'offset', 'limit'], 'script pagination order');
sameArray(contract.script_list_contract?.storage_parity, ['sea_orm', 'in_memory'], 'storage parity');
if (contract.script_list_contract?.status_filter !== 'known_script_status_or_validation_error') fail('status filter policy drift');
if (contract.script_list_contract?.page_min !== 1 || contract.script_list_contract?.per_page_min !== 1 || contract.script_list_contract?.per_page_max !== 100) fail('script pagination bounds drift');

sameArray(contract.execution_history_contract?.routes?.axum, ['/executions', '/scripts/{id}/executions'], 'axum routes');
sameArray(contract.execution_history_contract?.routes?.graphql, ['scriptExecutionHistory', 'recentScriptExecutions', 'scriptExecutions'], 'graphql routes');
sameArray(contract.execution_history_contract?.canonical_fields, ['id', 'script_id', 'script_name', 'phase', 'outcome', 'duration_ms', 'error', 'user_id', 'tenant_id', 'created_at'], 'execution canonical fields');
if (contract.execution_history_contract?.tenant_filter_before_offset !== true) fail('tenant filter ordering drift');

if (contract.sandbox_contract?.profiles?.default?.max_operations !== 50000 || contract.sandbox_contract?.profiles?.default?.timeout_ms !== 100) fail('default sandbox profile drift');
if (contract.sandbox_contract?.profiles?.strict?.max_operations !== 10000 || contract.sandbox_contract?.profiles?.strict?.timeout_ms !== 50 || contract.sandbox_contract?.profiles?.strict?.max_call_depth !== 8) fail('strict sandbox profile drift');
if (contract.sandbox_contract?.profiles?.relaxed?.max_operations !== 500000 || contract.sandbox_contract?.profiles?.relaxed?.timeout_ms !== 5000) fail('relaxed sandbox profile drift');
if (contract.sandbox_contract?.operator_surface !== 'rustok_sandbox::rhai::RhaiConfig::limits') fail('sandbox operator surface drift');
if (contract.sandbox_contract?.owner !== 'rustok-sandbox') fail('sandbox owner drift');
if (contract.sandbox_contract?.brokered_capability_adapter !== 'alloy::HttpCapabilityBridge via SandboxHost platform.http') fail('sandbox brokered capability adapter drift');
if (contract.sandbox_contract?.timeout_enforcement !== 'progress_callback_interrupts_execution_with_timeout') fail('sandbox timeout enforcement drift');
sameArray(contract.sandbox_contract?.rhai_native_limit_mapping, ['ErrorTooManyOperations_to_OperationLimit', 'ErrorDataTooLarge_to_ResourceLimit'], 'rhai native limit mapping');
if (contract.scheduler_hook_contract?.scheduler_phase !== 'Scheduled' || contract.scheduler_hook_contract?.scheduler_tenant_context !== 'script_tenant_id') fail('scheduler context drift');
sameArray(contract.script_crud_validation_contract?.rest_create, ['reject_duplicate_name', 'validate_cron_trigger', 'validate_workspace_and_compile_entrypoint_before_save', 'persist_optional_tenant_id'], 'REST create validation');
sameArray(contract.script_crud_validation_contract?.rest_update, ['invalidate_old_name_on_rename', 'validate_new_workspace_and_compile_entrypoint_before_save', 'validate_cron_trigger_before_save', 'invalidate_cache_on_workspace_change', 'require_expected_version'], 'REST update validation');
sameArray(contract.script_crud_validation_contract?.graphql_create_update, ['require_admin', 'validate_cron_trigger', 'validate_workspace_and_compile_entrypoint_before_save', 'tenant_from_context_on_create', 'require_expected_version'], 'GraphQL create/update validation');
sameArray(contract.execution_command_contract?.manual_run, ['require_expected_version', 'execute_loaded_snapshot'], 'manual execution revision contract');
sameArray(contract.execution_command_contract?.mcp_manual_run, ['require_expected_version', 'execute_loaded_snapshot'], 'MCP manual execution revision contract');
sameArray(contract.lifecycle_command_contract?.rest_activate_pause, ['require_expected_version'], 'REST lifecycle revision contract');
sameArray(contract.lifecycle_command_contract?.rest_delete, ['require_expected_version'], 'REST delete revision contract');
sameArray(contract.lifecycle_command_contract?.graphql_status_mutations, ['require_expected_version'], 'GraphQL lifecycle revision contract');
sameArray(contract.lifecycle_command_contract?.graphql_delete, ['require_expected_version'], 'GraphQL delete revision contract');
sameArray(contract.lifecycle_command_contract?.mcp_delete, ['require_expected_version'], 'MCP delete revision contract');
if (contract.source_revision_ledger_contract?.persistence !== 'durable_immutable_source_snapshots') fail('source revision ledger persistence contract drift');
if (contract.source_revision_ledger_contract?.lookup !== 'owner_scoped_by_script_id_and_revision') fail('source revision ledger lookup contract drift');
if (contract.source_revision_ledger_contract?.listing !== 'owner_tenant_scoped_revision_ascending') fail('source revision ledger listing contract drift');
sameArray(contract.source_revision_ledger_contract?.snapshot_fields, ['source_digest', 'workspace', 'author_id', 'parent_revision'], 'source revision ledger fields');
if (contract.review_contract?.persistence !== 'durable_immutable_revision_decisions') fail('review persistence contract drift');
if (contract.review_contract?.subject !== 'tenant_scoped_script_source_revision' || contract.review_contract?.precondition !== 'expected_current_revision') fail('review subject contract drift');
if (contract.review_contract?.idempotency !== 'script_revision_idempotency_key_and_request_digest') fail('review idempotency contract drift');
if (contract.review_contract?.transports !== 'graphql_and_host_http_require_scripts_manage_and_verified_actor') fail('review transport contract drift');
sameArray(contract.review_contract?.evidence_fields, ['source_digest', 'policy_revision', 'actor_id', 'reason'], 'review evidence fields');
if (contract.test_command_contract?.persistence !== 'durable_revision_pinned_test_run_ledger') fail('test command persistence contract drift');
if (contract.test_command_contract?.precondition !== 'expected_current_revision') fail('test command precondition contract drift');
if (contract.test_command_contract?.idempotency !== 'script_revision_idempotency_key_and_request_digest') fail('test command idempotency contract drift');
if (contract.test_command_contract?.recovery !== 'bounded_pending_lease_reclaim_without_replacing_source_revision') fail('test command recovery contract drift');
if (contract.test_command_contract?.execution !== 'sandbox_outside_transaction_with_capability_free_test_entrypoint') fail('test command execution contract drift');
if (contract.test_command_contract?.transports !== 'graphql_and_host_http_require_scripts_manage_and_verified_actor') fail('test command transport contract drift');
sameArray(contract.test_command_contract?.evidence_fields, ['source_digest', 'test_path', 'actor_id', 'terminal_boolean_result'], 'test command evidence fields');
if (contract.release_stage_contract?.persistence !== 'owner_owned_alloy_authored_staging') fail('release stage persistence contract drift');
if (contract.release_stage_contract?.precondition !== 'expected_current_revision_and_latest_approved_review') fail('release stage precondition contract drift');
if (contract.release_stage_contract?.idempotency !== 'publish_request_idempotency_key_bound_to_source_revision_and_review') fail('release stage idempotency contract drift');
if (contract.release_stage_contract?.ownership !== 'rustok_modules_is_sole_marketplace_writer') fail('release stage ownership contract drift');
if (contract.release_stage_contract?.artifact_payload_media_type !== 'application/vnd.rustok.rhai.workspace.v1') fail('release stage artifact payload media type drift');
if (contract.release_stage_contract?.artifact_digest_relation !== 'equals_reviewed_source_digest') fail('release stage artifact/source digest relation drift');
if (contract.release_stage_contract?.transports !== 'graphql_and_host_http_require_scripts_and_modules_manage_and_verified_actor') fail('release stage transport authorization drift');
if (contract.release_stage_contract?.transport_route !== '/api/alloy/scripts/{id}/releases/stage') fail('release stage host route drift');
sameArray(contract.release_stage_contract?.evidence_fields, ['artifact_digest', 'source_digest', 'source_revision', 'alloy_tenant_id', 'alloy_script_id', 'review_reference', 'review_digest', 'review_policy_revision', 'platform_admission'], 'release stage evidence fields');
if (contract.workspace_contract?.persistence !== 'bounded_revisioned_json_workspace') fail('workspace persistence contract drift');
if (contract.workspace_contract?.payload_media_type !== 'application/vnd.rustok.rhai.workspace.v1') fail('workspace payload media type drift');
if (contract.workspace_contract?.sandbox_source_resolution !== 'alloy_extension_static_in_memory_resolver_from_canonical_workspace_bytes') fail('workspace source resolution contract drift');
if (contract.workspace_contract?.guest_filesystem !== 'forbidden') fail('workspace guest filesystem contract drift');
if (contract.workspace_contract?.test_execution?.entrypoints !== 'declared_tests_rhai_only') fail('workspace test entrypoint contract drift');
if (contract.workspace_contract?.test_execution?.source_identity !== 'same_canonical_workspace_digest_and_revision') fail('workspace test source identity contract drift');
if (contract.workspace_contract?.test_execution?.imports !== 'exact_in_memory_src_rhai_only') fail('workspace test import contract drift');
if (contract.workspace_contract?.test_execution?.capabilities !== 'always_default_deny') fail('workspace test capability contract drift');
if (contract.workspace_contract?.test_execution?.result !== 'boolean_without_entity_mutations') fail('workspace test result contract drift');
if (contract.workspace_contract?.limits?.max_files !== 64 || contract.workspace_contract?.limits?.max_file_bytes !== 131072 || contract.workspace_contract?.limits?.max_workspace_bytes !== 1048576 || contract.workspace_contract?.limits?.max_path_bytes !== 160 || contract.workspace_contract?.limits?.max_import_depth !== 8) fail('workspace limits contract drift');
if (contract.script_crud_validation_contract?.error_mapping?.compilation !== 'validation' || contract.script_crud_validation_contract?.error_mapping?.invalid_cron !== 'validation' || contract.script_crud_validation_contract?.error_mapping?.duplicate_name !== 'conflict') fail('CRUD validation error mapping drift');
sameArray(contract.scheduler_hook_contract?.running_flag_reset, ['load_error', 'completed_success', 'completed_aborted', 'completed_failed'], 'scheduler running flag reset');
sameArray(contract.scheduler_hook_contract?.hook_phases, ['Before', 'After', 'OnCommit'], 'hook phases');
sameArray(contract.scheduler_hook_contract?.before_outcomes, ['Continue', 'Rejected', 'Error'], 'before hook outcomes');

if (evidence.generated_from !== contractPath || evidence.status !== contract.status) fail('evidence header drift');
sameArray(evidence.cases.map(c => c.name), ['script_list_pagination_status_contract', 'execution_history_transport_contract', 'documentation_sync_contract', 'sandbox_limits_timeout_contract', 'scheduler_hook_runtime_contract', 'script_crud_validation_contract', 'execution_command_revision_contract', 'lifecycle_command_revision_contract', 'source_revision_ledger_read_contract', 'workspace_payload_contract', 'review_revision_contract', 'test_command_revision_contract', 'release_stage_revision_contract'], 'evidence cases');

const dto = read('crates/alloy/src/api/dto.rs');
hasAll(dto, [
  'ScriptStatus::parse(status)',
  'Unsupported script status filter: {status}',
  'self.page.max(1)',
  'self.per_page.clamp(1, 100) as u64',
  'fn list_scripts_query_rejects_unknown_status_filter()',
  'fn list_scripts_query_clamps_limit_before_offset()',
  'pub struct ExecutionLogResponse',
  'pub script_id: ScriptId',
  'pub script_name: String',
  'pub phase: String',
  'pub outcome: String',
  'pub duration_ms: i64',
  'pub error: Option<String>',
  'pub user_id: Option<String>',
  'pub tenant_id: Option<Uuid>',
  'pub created_at: String'
], 'api dto');

const memory = read('crates/alloy/src/storage/memory.rs');
hasAll(memory, [
  'ScriptQuery::ByStatus(status) => guard',
  '.filter(|script| script.status == status)',
  'result.sort_by(|left, right|',
  'left.name',
  '.then_with(|| left.id.cmp(&right.id))',
  '.skip(offset as usize)',
  '.take(limit as usize)',
  'paginated_status_query_keeps_total_and_name_order_after_filtering',
  'ScriptError::RevisionConflict',
  'save_rejects_a_stale_script_revision',
  'async fn get_source_revision',
  'async fn list_source_revisions',
  'source_revision_history_preserves_immutable_source_snapshots',
  'async fn review',
  'async fn list_reviews',
  'ReviewError::IdempotencyConflict',
  'async fn claim_test_run',
  'async fn complete_test_run',
  'TestRunError::IdempotencyConflict'
], 'in-memory storage');

const sea = read('crates/alloy/src/storage/sea_orm.rs');
hasAll(sea, [
  'ScriptQuery::ByStatus(status) => select.filter(Column::Status.eq(status.as_str()))',
  '.order_by_asc(Column::Name)',
  '.offset(offset)',
  '.limit(limit)',
  'fn scoped_by_id(&self, id: ScriptId)',
  'fn ensure_script_scope(&self, script: &Script)',
  '.scoped_by_id(id)',
  '.filter(Column::Version.eq(expected_revision))',
  'mod draft_revision',
  'async fn insert_revision_snapshot',
  'async fn ensure_revision_snapshot',
  'fn model_to_source_revision',
  'async fn get_source_revision',
  'async fn list_source_revisions',
  'alloy_script_revisions',
  'save_rejects_a_stale_revision_without_overwriting_the_current_script',
  'save_persists_immutable_source_revision_lineage',
  'source_revision_queries_are_tenant_scoped',
  'tenant_scoped_single_record_paths_hide_and_preserve_other_tenant_scripts',
  'mod draft_review',
  'fn model_to_review_decision',
  'async fn review',
  'async fn list_reviews',
  'alloy_script_reviews',
  'mod draft_test_run',
  'async fn claim_test_run',
  'async fn complete_test_run',
  'alloy_script_test_runs'
], 'sea orm storage');

const revisionMigration = read('crates/alloy/src/migrations/m20260718_000003_create_script_revisions.rs');
hasAll(revisionMigration, [
  'alloy_script_revisions',
  'uidx_alloy_script_revisions_script_revision',
  'idx_alloy_script_revisions_tenant_script_revision'
], 'Alloy revision ledger migration');

const review = read('crates/alloy/src/model/review.rs');
hasAll(review, [
  'pub struct ReviewCommand',
  'pub struct ReviewDecision',
  'pub enum ReviewStatus',
  'expected_revision',
  'idempotency_key',
  'request_digest',
  'fn validate_transition'
], 'Alloy review contract');
const reviewMigration = read('crates/alloy/src/migrations/m20260718_000004_create_script_reviews.rs');
hasAll(reviewMigration, [
  'alloy_script_reviews',
  'uidx_alloy_script_reviews_idempotency',
  'idx_alloy_script_reviews_tenant_revision_created'
], 'Alloy review migration');
const testRun = read('crates/alloy/src/model/test_run.rs');
hasAll(testRun, [
  'pub struct TestCommand',
  'pub struct TestRun',
  'pub enum TestRunStatus',
  'pub enum TestRunClaim',
  'test_run_lease_expires_at',
  'IdempotencyConflict',
  'LeaseLost'
], 'Alloy test command contract');
const testMigration = read('crates/alloy/src/migrations/m20260718_000005_create_script_test_runs.rs');
hasAll(testMigration, [
  'alloy_script_test_runs',
  'uidx_alloy_script_test_runs_idempotency',
  'idx_alloy_script_test_runs_tenant_revision_created'
], 'Alloy test run migration');
const testRunner = read('crates/alloy/src/runner/test.rs');
hasAll(testRunner, [
  'pub struct RevisionedTestRunner',
  '.claim_test_run(command)',
  '.execute_test(&script, &lease.run.test_path, &context)',
  '.complete_test_run(lease.run.id, lease.lease_token, completion)',
  'sandbox work and records a terminal result afterward'
], 'Alloy revision-pinned test runner');
const release = read('crates/alloy/src/model/release.rs');
hasAll(release, [
  'pub struct AlloyReleaseStageCommand',
  'expected_revision',
  'publish_request_id',
  'artifact_digest',
  'idempotency_key',
  'pub fn review_evidence_digest',
  'pub fn review_reference'
], 'Alloy release stage contract');
const releaseRunner = read('crates/alloy/src/runner/release.rs');
hasAll(releaseRunner, [
  'pub trait AlloyReleaseGovernance',
  'pub struct RevisionedReleaseStager',
  'script.version != command.expected_revision',
  'command.artifact_digest != source.source_digest',
  'ArtifactSourceDigestMismatch',
  '.list_reviews(command.script_id, command.expected_revision)',
  'is_release_approved(review)',
  'alloy_tenant_id: source.tenant_id',
  'alloy_script_id: source.script_id',
  'ModuleAlloyAuthoredStageCommand',
  '.stage_alloy_authored('
], 'Alloy revision-pinned release stager');
const releaseGraphql = read('crates/alloy/src/graphql/mutation.rs');
hasAll(releaseGraphql, [
  'async fn stage_release',
  'require_release_admin(ctx).await?',
  'RevisionedReleaseStager::new',
  'AlloyReleaseStageCommand',
  'idempotency_key: input.idempotency_key'
], 'Alloy GraphQL release transport');
const governance = read('crates/rustok-modules/src/governance.rs');
hasAll(governance, [
  'AlloyAuthored',
  'pub struct ModuleAlloyAuthoredStageCommand',
  'pub async fn stage_alloy_authored',
  'registry_publish_alloy_staging',
  'PublishRequestMissingAlloyAuthoredStage',
  'PublishRequestMissingAlloyPlatformAdmission'
], 'owner Alloy publication stage');
const alloyArtifact = read('crates/alloy/src/artifact.rs');
hasAll(alloyArtifact, [
  'MODULE_ARTIFACT_RHAI_WORKSPACE_MEDIA_TYPE',
  'canonical_bytes()'
], 'Alloy workspace artifact package');
const installation = read('crates/rustok-modules/src/installation.rs');
hasAll(installation, [
  'pub payload_media_type: String',
  'admission.media_type AS payload_media_type',
  'media_type: self.payload_media_type.clone()',
  'MODULE_ARTIFACT_RHAI_WORKSPACE_MEDIA_TYPE'
], 'durable artifact payload media type');
const reviewGraphql = read('crates/alloy/src/graphql/mutation.rs');
hasAll(reviewGraphql, [
  'async fn review_script',
  'let auth = require_admin(ctx).await?',
  'actor_id: auth.user_id.to_string()',
  'ReviewCommand {'
], 'Alloy GraphQL review transport');
const reviewQuery = read('crates/alloy/src/graphql/query.rs');
hasAll(reviewQuery, [
  'async fn script_reviews',
  '.list_reviews(script_id, revision)'
], 'Alloy GraphQL review history');
const reviewHttp = read('crates/alloy/src/controllers/mod.rs');
hasAll(reviewHttp, [
  'fn review_actor',
  'Script review requires scripts.manage permission',
  'async fn review_script',
  'async fn list_reviews',
  'actor_id,',
  '/api/alloy/scripts/{id}/reviews'
], 'Alloy host HTTP review transport');
hasAll(reviewHttp, [
  'fn test_actor',
  'Script test requires scripts.manage permission',
  'async fn run_workspace_test',
  '/api/alloy/scripts/{id}/tests/run'
], 'Alloy host HTTP test transport');
hasAll(reviewHttp, [
  'fn release_actor',
  'scripts.manage or modules.manage permission',
  'async fn stage_release',
  '/api/alloy/scripts/{id}/releases/stage',
  'AlloyReleaseStageCommand'
], 'Alloy host HTTP release transport');
hasAll(reviewGraphql, [
  'async fn run_workspace_test',
  'RevisionedTestRunner::new',
  'actor_id: auth.user_id.to_string()',
  'TestCommand {'
], 'Alloy GraphQL test transport');

const engineConfig = read('crates/rustok-sandbox/src/rhai/config.rs');
hasAll(engineConfig, [
  'max_operations: 50_000',
  'timeout: Duration::from_millis(100)',
  'max_call_depth: 16',
  'max_string_size: 64 * 1024',
  'max_array_size: 10_000',
  'max_map_size: 16',
  'pub fn relaxed() -> Self',
  'max_operations: 500_000',
  'timeout: Duration::from_secs(5)',
  'pub fn strict() -> Self',
  'max_operations: 10_000',
  'timeout: Duration::from_millis(50)',
  'max_call_depth: 8',
  'pub fn limits(&self) -> RhaiLimits'
], 'engine config sandbox limits');

const engineRuntime = read('crates/rustok-sandbox/src/rhai/engine.rs');
hasAll(engineRuntime, [
  'engine.on_progress(move |_|',
  'TIMEOUT_MARKER',
  'ExecutionTimerGuard::start()',
  'RhaiError::Timeout',
  'EvalAltResult::ErrorTooManyOperations',
  'RhaiError::OperationLimit',
  'EvalAltResult::ErrorDataTooLarge',
  'RhaiError::ResourceLimit'
], 'engine runtime timeout and native limit mapping');

const alloyEngineAdapter = read('crates/alloy/src/engine/runtime.rs');
hasAll(alloyEngineAdapter, [
  'Alloy-specific adapter over the neutral Rhai execution kernel',
  'inner: RhaiEngine',
  '.map_err(ScriptError::from)'
], 'Alloy sandbox adapter');

const httpBridge = read('crates/alloy/src/bridge/http.rs');
hasAll(httpBridge, [
  'pub struct HttpCapabilityBridge',
  'impl RhaiHostExtension for HttpCapabilityBridge',
  'host.invoke_blocking(&call)',
  'const HTTP_CAPABILITY: &str = "platform.http"'
], 'Alloy brokered HTTP bridge');
if (httpBridge.includes('reqwest::')) fail('Alloy HTTP bridge must not own a direct HTTP client');
if (read('crates/alloy/Cargo.toml').includes('reqwest')) fail('Alloy must not depend on a direct HTTP client');

const handlers = read('crates/alloy/src/api/handlers.rs');
hasAll(handlers, [
  'Script with name',
  'code: "conflict".to_string()',
  'validate_trigger(&req.trigger)?',
  '.entrypoint_source()',
  'state.engine.compile(&req.name, source, &mut scope)?',
  'state.engine.invalidate(&script.name);',
  'script.workspace = workspace;',
  'state.engine.compile(&script.name, source, &mut scope)?',
  'req.expected_version',
  'validate_trigger(&trigger)?',
  'fn validate_trigger(trigger: &ScriptTrigger) -> ApiResult<()>',
  'Invalid cron expression: {error}'
], 'REST CRUD validation');

const gqlMutation = read('crates/alloy/src/graphql/mutation.rs');
hasAll(gqlMutation, [
  'fn validate_cron_trigger(trigger: &ScriptTriggerInput) -> Result<()>',
  'require_admin(ctx).await?',
  'validate_cron_trigger(&input.trigger)?',
  'input.workspace.0',
  '.compile(&input.name, source, &mut scope)',
  'validate_cron_trigger(trigger)?',
  '.compile(&script.name, source, &mut scope)',
  'input.expected_version',
  'data::<rustok_api::TenantContext>()'
], 'GraphQL CRUD validation');

const controllers = read('crates/alloy/src/controllers/mod.rs');
hasAll(controllers, [
  'validate_trigger(&req.trigger)?',
  '.compile(&req.name, source, &mut scope)',
  'req.expected_version',
  'validate_trigger(trigger)?',
  '.compile(&script.name, source, &mut scope)'
], 'host-composed REST CRUD validation');
hasAll(dto, [
  'pub struct ScriptRevisionRequest',
  'pub expected_version: u32'
], 'REST lifecycle revision request');
hasAll(controllers, [
  'Json(request): Json<ScriptRevisionRequest>',
  'script.version != request.expected_version',
  'ScriptError::RevisionConflict'
], 'REST lifecycle revision validation');
const apiHandlers = read('crates/alloy/src/api/handlers.rs');
hasAll(apiHandlers, [
  'Json(request): Json<ScriptRevisionRequest>',
  'state.registry.delete(id, request.expected_version)',
  'script.version != request.expected_version'
], 'direct REST delete revision validation');

const workspace = read('crates/alloy/src/model/workspace.rs');
hasAll(workspace, [
  'pub const MAX_WORKSPACE_FILES: usize = 64',
  'pub const MAX_WORKSPACE_FILE_BYTES: usize = 128 * 1024',
  'pub const MAX_WORKSPACE_BYTES: usize = 1024 * 1024',
  'pub const MAX_WORKSPACE_PATH_BYTES: usize = 160',
  'pub const MAX_WORKSPACE_IMPORT_DEPTH: usize = 8',
  'pub struct AlloyWorkspace',
  'pub struct WorkspaceFile',
  'StaticModuleResolver',
  'pub fn configure_rhai_engine',
  'pub fn configure_rhai_engine_for_entrypoint',
  'pub fn validate_rhai_workspace',
  'pub fn validate_rhai_test',
  'pub fn test_source',
  'workspace imports must use an exact src/*.rhai path',
  'workspace import cycle',
  'workspace import depth exceeds',
  'fn validate_path',
  'fn validate_file_kind',
  'pub fn canonical_bytes',
  'canonical_workspace_digest_is_independent_of_file_order'
], 'bounded Alloy workspace');
const sandboxRhai = read('crates/rustok-sandbox/src/rhai.rs');
hasAll(sandboxRhai, [
  'fn source_bytes(&self, _request: &SandboxRequest)',
  'multiple Rhai extensions supplied request source'
], 'Rhai owner source extension boundary');
const alloyDraft = read('crates/alloy/src/sandbox_request.rs');
hasAll(alloyDraft, [
  'ALLOY_DRAFT_RHAI_MEDIA_TYPE: &str = "application/vnd.rustok.rhai.workspace.v1"',
  'fn source_bytes(&self, request: &SandboxRequest)',
  'invalid Alloy workspace payload',
  '.configure_rhai_engine_for_entrypoint(engine, &request.payload.entrypoint)',
  '.executable_source(&request.payload.entrypoint)',
  'pub async fn execute_test',
  'pub fn build_test',
  'SandboxExecutionPhase::Test',
  'Alloy workspace tests must return a boolean',
  '.canonical_bytes()',
  '.digest()'
], 'Alloy workspace sandbox payload');

hasAll(handlers, [
  'req.expected_version',
  '.run_manual_snapshot(&script, params, entity, None)'
], 'REST manual execution revision validation');
hasAll(controllers, [
  'req.expected_version',
  '.run_manual_snapshot(&script, params, entity, None)'
], 'host-composed REST manual execution revision validation');
hasAll(gqlMutation, [
  'input.expected_version',
  '.run_manual_snapshot(&script, params, None, user_id)'
], 'GraphQL manual execution revision validation');
hasAll(gqlMutation, [
  'fn ensure_expected_revision(script: &Script, expected_version: u32)',
  'Script revision conflict: expected version',
  'async fn activate_script',
  'async fn pause_script',
  'async fn disable_script',
  'async fn archive_script',
  'async fn reset_script_errors',
  'expected_version: u32',
  'ensure_expected_revision(&script, expected_version)?'
], 'GraphQL lifecycle revision validation');
hasAll(gqlMutation, [
  'async fn delete_script',
  'expected_version: u32',
  '.delete(id, expected_version)'
], 'GraphQL delete revision validation');
const storageTraits = read('crates/alloy/src/storage/traits.rs');
hasAll(storageTraits, [
  'async fn delete(&self, id: ScriptId, expected_version: u32)'
], 'owner delete CAS contract');
const memoryStorage = read('crates/alloy/src/storage/memory.rs');
hasAll(memoryStorage, [
  'async fn delete(&self, id: ScriptId, expected_version: u32)',
  'script.version != expected_version',
  'ScriptError::RevisionConflict'
], 'memory delete CAS');
const seaOrmStorage = read('crates/alloy/src/storage/sea_orm.rs');
hasAll(seaOrmStorage, [
  'async fn delete(&self, id: ScriptId, expected_version: u32)',
  'Column::Version.eq',
  'ScriptError::RevisionConflict'
], 'SeaORM delete CAS');
const mcpAlloyTools = read('crates/rustok-mcp/src/alloy_tools.rs');
hasAll(mcpAlloyTools, [
  'pub struct DeleteScriptRequest',
  'pub expected_version: u32',
  'if script.version != request.expected_version',
  '.delete(id, request.expected_version)'
], 'MCP delete revision validation');
hasAll(mcpAlloyTools, [
  'pub workspace: AlloyWorkspace',
  'pub expected_version: u32',
  'validate_rhai_workspace()',
  'run_manual_snapshot(&script, params, entity, None)'
], 'MCP workspace and execution revision validation');
hasAll(mcpAlloyTools, [
  'pub struct UpdateScriptRequest',
  'if script.version != request.expected_version',
  'script.workspace = workspace'
], 'MCP update revision validation');
const orchestrator = read('crates/alloy/src/runner/orchestrator.rs');
hasAll(orchestrator, [
  'pub async fn run_manual_snapshot',
  'second registry lookup cannot replace the admitted source revision'
], 'Alloy manual snapshot execution');

const executor = read('crates/alloy/src/runner/executor.rs');
hasAll(executor, [
  'if elapsed > self.engine.config().timeout',
  'Script exceeded timeout',
  'self.record_execution(&result, &ctx_with_entity).await'
], 'executor timeout observability and audit persistence');

const scheduler = read('crates/alloy/src/scheduler/runner.rs');
hasAll(scheduler, [
  'job.running = true',
  'self.mark_finished(script_id).await',
  'ExecutionContext::new(ExecutionPhase::Scheduled)',
  '.with_tenant(script.tenant_id.to_string())',
  'self.update_schedule(&script).await',
  'job.running = false',
  'scheduler_tick_persists_execution_log_with_script_tenant'
], 'scheduler phase tenant and running flag contract');

const hookExecutor = read('crates/alloy/src/integration/hook_executor.rs');
hasAll(hookExecutor, [
  'pub enum BeforeHookResult',
  'Continue(HashMap<String, Dynamic>)',
  'Rejected(String)',
  'HookOutcome::Continue { changes }',
  'HookOutcome::Rejected { reason }',
  'HookOutcome::Error { error }',
  'pub async fn run_on_commit',
  'Vec<crate::runner::ExecutionResult>'
], 'hook executor typed outcome contract');

const axumRoutes = read('crates/alloy/src/api/routes.rs');
hasAll(axumRoutes, [
  'AXUM_EXECUTION_HISTORY_ROUTES: &[&str] = &["/executions", "/scripts/{id}/executions"]',
  'get(handlers::list_recent_executions::<S>)',
  'get(handlers::list_script_executions::<S>)'
], 'axum routes');

const controllerRoutes = read('crates/alloy/src/controllers/mod.rs');
hasAll(controllerRoutes, [
  'AXUM_EXECUTION_HISTORY_ROUTES as EXECUTION_HISTORY_ROUTES',
  'list_recent_executions',
  'list_script_executions'
], 'controller route bridge');

const graphql = read('crates/alloy/src/graphql/query.rs');
hasAll(graphql, [
  'async fn script_execution_history',
  'async fn recent_script_executions',
  'async fn script_executions',
  'execution_history_graphql_fields_match_public_schema_contract'
], 'graphql query');

const readme = read('crates/alloy/README.md');
hasAll(readme, [contractPath, evidencePath, 'npm run verify:alloy:runtime-contract'], 'crate README');
const docs = read('crates/alloy/docs/README.md');
hasAll(docs, [contractPath, evidencePath, 'npm run verify:alloy:runtime-contract'], 'local docs');
const plan = read('crates/alloy/docs/implementation-plan.md');
hasAll(plan, [contractPath, evidencePath, 'verify-alloy-runtime-contract.mjs', 'npm run verify:alloy:runtime-contract'], 'local plan');

console.log('[verify-alloy-runtime-contract] Alloy runtime contract metadata, static evidence, source guards and docs are consistent');
