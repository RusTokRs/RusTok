import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => { console.error(`[verify-workflow-fba] ${message}`); process.exit(1); };
const sameSet = (a, b) => Array.isArray(a) && Array.isArray(b) && a.length === b.length && b.every((x) => a.includes(x));

const registryPath = 'crates/rustok-workflow/contracts/workflow-fba-registry.json';
const evidencePath = 'crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const manifest = read('crates/rustok-workflow/rustok-module.toml');
const plan = read('crates/rustok-workflow/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');
const lib = read('crates/rustok-workflow/src/lib.rs');
const ports = read('crates/rustok-workflow/src/ports.rs');
const dto = read('crates/rustok-workflow/src/dto/mod.rs');

if (registry.schema_version !== 1) fail('registry schema_version must be 1');
if (registry.module !== 'workflow' || registry.role !== 'provider' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.contract_version !== 'workflow.read_projection.v1') fail('contract version drift');
const [port] = registry.ports ?? [];
if (!port || port.name !== 'WorkflowReadPort') fail('WorkflowReadPort missing');
for (const op of ['list_workflows', 'get_workflow']) if (!port.operations.includes(op)) fail(`port lacks ${op}`);
if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail('context/error drift');
if (port.deadline_required !== true || port.idempotency_required !== false) fail('read projection semantics drift');
if (!manifest.includes('[fba.provider]') || !manifest.includes('registry = "contracts/workflow-fba-registry.json"') || !manifest.includes('contract_version = "workflow.read_projection.v1"')) fail('manifest FBA metadata drift');
if (!lib.includes('pub mod ports;') || !lib.includes('pub use ports::*;')) fail('lib.rs must export ports');
for (const marker of ['trait WorkflowReadPort', 'impl WorkflowReadPort for WorkflowService', 'context.require_deadline_semantics()?', 'workflow_tenant_id(&context)?', 'workflow.tenant_id_invalid', 'PortErrorKind::NotFound']) if (!ports.includes(marker)) fail(`ports source missing ${marker}`);
if (ports.includes('require_write_semantics()?')) fail('workflow read port must not require write idempotency');
if (!dto.includes('Serialize, Deserialize')) fail('workflow DTOs must remain serializable');
if (!plan.includes('- FBA status: `in_progress`') || !plan.includes(registryPath) || !plan.includes('WorkflowReadPort') || !plan.includes('workflow-contract-test-static-matrix.json')) fail('local plan FBA evidence drift');
if (!central.includes('| `workflow` |') || !central.includes(registryPath) || !central.includes('`phase_b_ready` | `in_progress`')) fail('central readiness board drift');
if (evidence.schema_version !== 1 || evidence.module !== 'workflow' || evidence.status !== 'static_matrix_locked') fail('evidence identity drift');
if (evidence.generated_from !== registryPath || evidence.runner !== 'scripts/verify/verify-workflow-fba.mjs' || evidence.contract_version !== registry.contract_version) fail('evidence source/runner/version drift');
if (!sameSet(evidence.profiles, registry.contract_tests.profiles)) fail('evidence profile drift');
for (const c of registry.contract_tests.cases) {
  const e = evidence.cases.find((entry) => entry.operation === c.operation);
  if (!e || e.execution_status !== 'static_locked_runtime_pending' || !sameSet(e.assertions, c.assertions)) fail(`evidence case drift for ${c.operation}`);
}
if (!sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('fallback profile drift');
console.log('[verify-workflow-fba] workflow FBA provider metadata, port semantics and static evidence are consistent');
