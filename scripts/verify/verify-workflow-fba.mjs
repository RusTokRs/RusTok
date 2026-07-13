import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-workflow-fba] ${message}`); process.exit(1); };
const sameSet = (a, b) => Array.isArray(a) && Array.isArray(b) && a.length === b.length && b.every((x) => a.includes(x));

const registryPath = 'crates/rustok-workflow/contracts/workflow-fba-registry.json';
const evidencePath = 'crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json';
const runtimeSmokePath = 'crates/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(runtimeSmokePath);
const runtimeOrderSmoke = json(registry.evidence.runtime_order_smoke);
const manifest = read('crates/rustok-workflow/rustok-module.toml');
const plan = read('crates/rustok-workflow/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const lib = read('crates/rustok-workflow/src/lib.rs');
const ports = read('crates/rustok-workflow/src/ports.rs');
const dto = read('crates/rustok-workflow/src/dto/mod.rs');
const nativeAdapter = read('crates/rustok-workflow/admin/src/transport/native_server_adapter.rs');
const graphqlAdapter = read('crates/rustok-workflow/admin/src/transport/graphql_adapter.rs');
const transportFacade = read('crates/rustok-workflow/admin/src/transport/mod.rs');
const packageJson = json('package.json');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'workflow' || registry.role !== 'provider' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
if (registry.contract_version !== 'workflow.read_projection.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'WorkflowReadPort') fail('WorkflowReadPort missing');
for (const op of ['list_workflows', 'get_workflow']) if (!port.operations.includes(op)) fail(`port lacks ${op}`);
if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== false) fail('read projection semantics drift');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/workflow-fba-registry.json"') || !manifest.includes('contract_version = "workflow.read_projection.v1"')) fail('manifest FBA metadata drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait WorkflowReadPort', 'impl WorkflowReadPort for WorkflowService', 'context.require_policy(PortCallPolicy::read())?', 'workflow_tenant_id(&context)?', 'workflow.tenant_id_invalid', 'PortErrorKind::NotFound']) if (!ports.includes(marker)) fail(`ports source missing ${marker}`);
if (ports.includes('require_write_semantics()?')) fail('workflow read port must not require write idempotency');
if (!dto.includes('Serialize, Deserialize')) fail('workflow DTOs must remain serializable');
if (!plan.includes('- FBA status: `boundary_ready`') || !plan.includes(registryPath) || !plan.includes('WorkflowReadPort') || !plan.includes('workflow-contract-test-static-matrix.json') || !plan.includes('workflow-read-projection-runtime-smoke.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `workflow` |') || !central.includes(registryPath) || !central.includes('`phase_b_ready` | `boundary_ready`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'workflow' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-workflow-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
for (const c of registry.contract_tests.cases) {
  const e = evidence.cases.find((entry) => entry.operation === c.operation);
  if (!e || e.execution_status !== 'static_locked_runtime_pending' || !sameSet(e.assertions, c.assertions)) fail(`evidence case drift for ${c.operation}`);
}
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
if (packageJson.scripts?.['verify:workflow:fba'] !== 'node scripts/verify/verify-workflow-fba.mjs && npm run verify:owner:fba-runtime-order') fail('package script verify:workflow:fba drift');
if (runtimeSmoke.schema_version !== 1 || runtimeSmoke.module !== 'workflow' || runtimeSmoke.status !== 'compile_free_runtime_smoke_locked') fail('runtime smoke identity drift');
if (runtimeSmoke.generated_from !== registryPath || runtimeSmoke.runner !== 'scripts/verify/verify-workflow-fba.mjs' || runtimeSmoke.contract_version !== registry.contract_version) fail('runtime smoke source/runner/version drift');
if (runtimeSmoke.promotion_allowed !== false || !sameSet(runtimeSmoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('runtime smoke profile/promotion drift');
for (const marker of ['workflow-admin/list-workflows', 'Permission::WORKFLOWS_LIST', 'TenantContext', 'WorkflowService::new', '.list(tenant.id)', 'map_workflow_summary']) if (!nativeAdapter.includes(marker)) fail(`native adapter missing ${marker}`);
for (const marker of ['query Workflows', 'workflowTemplates', 'mutation CreateWorkflowFromTemplate', 'GraphqlRequest::new', 'token', 'tenant_slug']) if (!graphqlAdapter.includes(marker)) fail(`graphql adapter missing ${marker}`);
for (const marker of ['execute_selected_transport(', 'selected_transport_path()', 'native_server_adapter::fetch_workflows_native', 'graphql_adapter::fetch_workflows(context.token, context.tenant_slug)']) if (!transportFacade.includes(marker)) fail(`transport facade missing ${marker}`);
for (const entry of runtimeSmoke.native_entrypoints) if (!entry.operation || !entry.source || !entry.server_fn_endpoint) fail('runtime smoke native entrypoint is incomplete');
for (const entry of runtimeSmoke.graphql_fallback_entrypoints) if (!entry.operation || !entry.source || !entry.query_marker || !entry.projection_field) fail('runtime smoke graphql entrypoint is incomplete');
for (const marker of ['native_first_before_graphql_fallback', 'host_token_preserved_for_graphql_fallback', 'host_tenant_slug_preserved_for_graphql_fallback', 'combined_native_and_graphql_error_visible_to_ui']) if (!runtimeSmoke.facade_assertions.includes(marker)) fail(`runtime smoke facade assertion missing ${marker}`);
if (runtimeSmoke.fallback_smoke?.status !== 'source_locked_live_runtime_pending') fail('runtime fallback smoke must stay live-runtime-pending');
if (runtimeOrderSmoke.generated_from !== registryPath || runtimeOrderSmoke.runner !== registry.evidence.runtime_order_smoke_runner || runtimeOrderSmoke.status !== 'executable_no_compile' || runtimeOrderSmoke.contract_version !== registry.contract_version) fail('runtime order smoke header drift');
if (!sameSet(runtimeOrderSmoke.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) fail('runtime order fallback profile drift');
if (!sameSet(runtimeOrderSmoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail('runtime order degraded mode drift');
for (const c of runtimeOrderSmoke.cases) {
  const bodyStart = ports.indexOf(`async fn ${c.operation}`);
  if (bodyStart === -1) fail(`runtime order operation missing ${c.operation}`);
  const body = ports.slice(bodyStart);
  for (const marker of c.source_order) {
    if (!body.includes(marker)) fail(`${c.operation} runtime order source marker missing ${marker}`);
  }
}
console.log('[verify-workflow-fba] workflow FBA provider metadata, port semantics, static evidence and compile-free runtime smoke are consistent');
