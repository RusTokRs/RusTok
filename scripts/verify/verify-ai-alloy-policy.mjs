import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-ai-alloy-policy] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const snippet of snippets) if (!text.includes(snippet)) fail(`${label} missing ${snippet}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json';
const evidencePath = 'crates/rustok-ai-alloy/contracts/evidence/ai-alloy-policy-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'ai-alloy' || registry.crate !== 'rustok-ai-alloy' || registry.role !== 'domain_support_adapter' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
if (registry.consumer_profile !== 'alloy_script_descriptor') fail('consumer profile drift');
if (registry.execution_policy?.composition_owner !== 'rustok-ai' || registry.execution_policy?.domain_owner !== 'rustok-ai-alloy') fail('execution ownership drift');
if (registry.execution_policy?.runtime_payload_json !== 'absent_blank_or_json_object') fail('runtime payload shape drift');
if (registry.execution_policy?.remote_transport !== 'not_started') fail('remote transport status drift');
sameSet(registry.execution_policy?.allowed_operations ?? [], ['list_scripts', 'get_script', 'validate_script', 'run_script'], 'execution policy allowed operations');
if (registry.support_adapter?.runtime_operation !== 'run_script' || registry.support_adapter?.transport_owner !== 'rustok-ai') fail('support adapter runtime operation/transport owner drift');
sameSet(registry.code_agents?.roles ?? [], ['alloy_code_planner', 'alloy_code_implementer', 'alloy_code_reviewer', 'alloy_code_verifier'], 'code agent roles');
if (registry.code_agents?.owner !== 'rustok-ai-alloy' || registry.code_agents?.catalog_api !== 'alloy_code_agents' || registry.code_agents?.workflow_api !== 'alloy_swarm_workflows') fail('code agent ownership/API drift');
if (registry.code_agents?.workflow !== 'alloy_change_review' || registry.code_agents?.apply_requires_approval !== true) fail('code agent workflow policy drift');
sameSet(registry.code_agents?.stage_execution_bindings ?? [], ['alloy_code_planner:list_scripts', 'alloy_code_implementer:validate_script', 'alloy_code_reviewer:validate_script', 'alloy_code_verifier:run_script'], 'code agent stage execution bindings');
if (registry.code_agents?.stage_execution_api !== 'alloy_stage_execution') fail('code agent stage execution API drift');

const source = read(registry.support_adapter.source);
hasAll(source, [
  'ALLOY_CODE_TASK_SLUG: &str = "alloy_code"',
  'ALLOY_CODE_TOOL_NAME: &str = "direct.alloy.run_script"',
  'register_alloy_ai_vertical_handlers',
  'validate_runtime_payload',
  'AlloyScriptExecutionPolicy',
  'ALLOY_SCRIPT_EXECUTION_POLICY',
  'alloy_script_execution_policy',
  'runtime_payload_json_shape: "absent_blank_or_json_object"',
  'composition_owner: "rustok-ai"',
  'domain_owner: "rustok-ai-alloy"',
  'runtime_operation: "run_script"',
  'transport_owner: "rustok-ai"',
  'ALLOY_SCRIPT_ALLOWED_OPERATIONS',
  'allowed_operations: ALLOY_SCRIPT_ALLOWED_OPERATIONS',
  'remote_transport: "not_started"',
  'AlloyCodeAgentDescriptor',
  'AlloySwarmWorkflowDescriptor',
  'ALLOY_CODE_AGENTS',
  'ALLOY_SWARM_WORKFLOWS',
  'alloy_code_agents',
  'alloy_swarm_workflows',
  'AlloyStageExecutionDescriptor',
  'ALLOY_STAGE_EXECUTIONS',
  'alloy_stage_execution',
  'slug: "alloy_change_review"',
  '!parsed.is_object()'
], 'support adapter source');

const runtimeSource = read('crates/rustok-ai/src/service.rs');
hasAll(runtimeSource, [
  'pub async fn execute_agent_workflow_stage',
  'catalog.validate_stage_execution(',
  'Self::run_task_job_with_authority(',
  'TaskJobExecutionAuthority::RegisteredAgentAssignment',
  'pub async fn resolve_agent_workflow_stage_approval',
  'let parent_is_active = ai_agent_workflow_runs::Entity::find_by_id',
  'Self::sync_agent_workflow_run_status(',
  'pub async fn claim_agent_workflow_stage',
  'let executable_workflow = ai_agent_workflow_runs::Entity::find_by_id',
  'pub async fn requeue_expired_agent_stage_leases',
  'async fn sync_agent_workflow_run_status',
  'Self::promote_agent_workflow_stages(db, tenant_id, stage.workflow_run_id)',
  'async fn sync_workflow_stage_after_run',
  'Self::promote_agent_workflow_stages',
  'let affected_workflow_runs',
  'workflow stage lease expired before its AI run could be recorded',
  'workflow stage lease expired before its AI approval state could be recorded',
  'ensure_agent_provider_capabilities(&provider, descriptor)?;',
  'workflow {binding_name} bindings must match the owner-declared stages exactly',
  'fn agent_workflow_execution_context(',
  'workflow run is missing its persisted agent execution context',
  '"agent_execution_context": {',
  '&agent_operator,',
  'async fn agent_execution_context_for_run(',
  'agent run parent workflow is terminal',
  'workflow stage parent run is terminal',
  'let execution_operator = agent_execution_context_for_run',
  'access_context_for_operator(&execution_operator)',
  'async fn sync_workflow_stage_after_run(',
  'Self::sync_workflow_stage_after_run(db, operator.tenant_id, &record).await?',
  'Column::RunId.eq(run.id)',
  'Expr::value("cancelled")',
  'workflow AI run was cancelled',
  'fn aggregate_agent_workflow_status',
  'fn cancellation_is_preserved_unless_a_stage_failed',
  'Column::LeaseExpiresAt.gte(Utc::now())',
  'ai_agent_workflow_stages::Column::StartedAt'
], 'AI workflow runtime source');

const agentSource = read('crates/rustok-ai/src/agent.rs');
hasAll(agentSource, [
  'fn owner_stage_binding_resolves_to_a_registered_direct_handler',
  'DirectExecutionRegistry::with_defaults()',
  '"alloy_code_verifier"',
  '"product_copywriter"'
], 'agent composed direct-binding regression');

const agentInputs = read('crates/rustok-ai/src/graphql/types.rs');
for (const inputName of ['CreateAiAgentPrincipalInputGql', 'UpdateAiAgentPrincipalInputGql']) {
  const start = agentInputs.indexOf(`pub struct ${inputName}`);
  const end = agentInputs.indexOf('\n}', start);
  if (start < 0 || end < 0) fail(`agent principal input ${inputName} is missing`);
  const body = agentInputs.slice(start, end);
  if (body.includes('role_slugs') || body.includes('permission_slugs')) fail(`agent principal input ${inputName} must not accept raw RBAC vocabulary`);
}

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
for (const evidenceCase of evidence.cases) {
  const registryCase = registry.contract_tests.cases.find(c => c.operation === evidenceCase.operation);
  sameSet(evidenceCase.assertions, registryCase.assertions, `assertions for ${evidenceCase.operation}`);
}

const plan = read('crates/rustok-ai-alloy/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `boundary_ready`', 'ai-alloy-policy-registry.json', 'ai-alloy-policy-static-matrix.json', 'alloy_script_execution_policy', 'allowed_operations', 'runtime_operation', 'alloy_change_review'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `rustok-ai-alloy` |', 'crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json', 'scripts/verify/verify-ai-alloy-policy.mjs', 'allowed operations'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`ai-alloy`', 'ai-alloy-policy-registry.json', 'alloy_script_execution_policy'], 'unified plan');

console.log('[verify-ai-alloy-policy] ai-alloy execution policy metadata and static evidence are consistent');
