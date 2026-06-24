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

sameArray(contract.execution_history_contract?.routes?.loco, ['/executions', '/scripts/{id}/executions'], 'loco routes');
sameArray(contract.execution_history_contract?.routes?.axum, ['/executions', '/scripts/{id}/executions'], 'axum routes');
sameArray(contract.execution_history_contract?.routes?.graphql, ['scriptExecutionHistory', 'recentScriptExecutions', 'scriptExecutions'], 'graphql routes');
sameArray(contract.execution_history_contract?.canonical_fields, ['id', 'script_id', 'script_name', 'phase', 'outcome', 'duration_ms', 'error', 'user_id', 'tenant_id', 'created_at'], 'execution canonical fields');
if (contract.execution_history_contract?.tenant_filter_before_offset !== true) fail('tenant filter ordering drift');

if (contract.sandbox_contract?.profiles?.default?.max_operations !== 50000 || contract.sandbox_contract?.profiles?.default?.timeout_ms !== 100) fail('default sandbox profile drift');
if (contract.sandbox_contract?.profiles?.strict?.max_operations !== 10000 || contract.sandbox_contract?.profiles?.strict?.timeout_ms !== 50 || contract.sandbox_contract?.profiles?.strict?.max_call_depth !== 8) fail('strict sandbox profile drift');
if (contract.sandbox_contract?.profiles?.relaxed?.max_operations !== 500000 || contract.sandbox_contract?.profiles?.relaxed?.timeout_ms !== 5000) fail('relaxed sandbox profile drift');
if (contract.sandbox_contract?.operator_surface !== 'EngineConfig::limits') fail('sandbox operator surface drift');
sameArray(contract.sandbox_contract?.rhai_native_limit_mapping, ['ErrorTooManyOperations_to_OperationLimit', 'ErrorDataTooLarge_to_ResourceLimit'], 'rhai native limit mapping');
if (contract.scheduler_hook_contract?.scheduler_phase !== 'Scheduled' || contract.scheduler_hook_contract?.scheduler_tenant_context !== 'script_tenant_id') fail('scheduler context drift');
sameArray(contract.scheduler_hook_contract?.running_flag_reset, ['load_error', 'completed_success', 'completed_aborted', 'completed_failed'], 'scheduler running flag reset');
sameArray(contract.scheduler_hook_contract?.hook_phases, ['Before', 'After', 'OnCommit'], 'hook phases');
sameArray(contract.scheduler_hook_contract?.before_outcomes, ['Continue', 'Rejected', 'Error'], 'before hook outcomes');

if (evidence.generated_from !== contractPath || evidence.status !== contract.status) fail('evidence header drift');
sameArray(evidence.cases.map(c => c.name), ['script_list_pagination_status_contract', 'execution_history_transport_contract', 'documentation_sync_contract', 'sandbox_limits_timeout_contract', 'scheduler_hook_runtime_contract'], 'evidence cases');

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
  'paginated_status_query_keeps_total_and_name_order_after_filtering'
], 'in-memory storage');

const sea = read('crates/alloy/src/storage/sea_orm.rs');
hasAll(sea, [
  'ScriptQuery::ByStatus(status) => select.filter(Column::Status.eq(status.as_str()))',
  '.order_by_asc(Column::Name)',
  '.offset(offset)',
  '.limit(limit)'
], 'sea orm storage');

const engineConfig = read('crates/alloy/src/engine/config.rs');
hasAll(engineConfig, [
  'max_operations: 50_000',
  'timeout: Duration::from_millis(100)',
  'max_call_depth: 16',
  'max_string_size: 64 * 1024',
  'max_array_size: 10_000',
  'max_map_depth: 16',
  'pub fn relaxed() -> Self',
  'max_operations: 500_000',
  'timeout: Duration::from_secs(5)',
  'pub fn strict() -> Self',
  'max_operations: 10_000',
  'timeout: Duration::from_millis(50)',
  'max_call_depth: 8',
  'pub fn limits(&self) -> EngineLimits'
], 'engine config sandbox limits');

const engineRuntime = read('crates/alloy/src/engine/runtime.rs');
hasAll(engineRuntime, [
  'let timeout = self.config.timeout',
  'let max_ops = self.config.max_operations',
  'if elapsed > timeout',
  'ScriptError::Timeout',
  'EvalAltResult::ErrorTooManyOperations',
  'ScriptError::OperationLimit { limit: op_limit }',
  'EvalAltResult::ErrorDataTooLarge',
  'ScriptError::ResourceLimit { resource: kind }'
], 'engine runtime timeout and native limit mapping');

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

const locoRoutes = read('crates/alloy/src/controllers/mod.rs');
hasAll(locoRoutes, [
  'LOCO_EXECUTION_HISTORY_ROUTES: &[&str] = &["/executions", "/scripts/{id}/executions"]',
  'list_recent_executions',
  'list_script_executions'
], 'loco routes');

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
