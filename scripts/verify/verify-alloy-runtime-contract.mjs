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

if (evidence.generated_from !== contractPath || evidence.status !== contract.status) fail('evidence header drift');
sameArray(evidence.cases.map(c => c.name), ['script_list_pagination_status_contract', 'execution_history_transport_contract', 'documentation_sync_contract'], 'evidence cases');

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
