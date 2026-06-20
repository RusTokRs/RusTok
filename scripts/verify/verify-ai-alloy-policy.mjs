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
if (registry.module !== 'ai-alloy' || registry.crate !== 'rustok-ai-alloy' || registry.role !== 'domain_support_adapter' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.consumer_profile !== 'alloy_script_descriptor') fail('consumer profile drift');
if (registry.execution_policy?.composition_owner !== 'rustok-ai' || registry.execution_policy?.domain_owner !== 'rustok-ai-alloy') fail('execution ownership drift');
if (registry.execution_policy?.runtime_payload_json !== 'absent_blank_or_json_object') fail('runtime payload shape drift');

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
  '!parsed.is_object()'
], 'support adapter source');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');

const plan = read('crates/rustok-ai-alloy/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `in_progress`', 'ai-alloy-policy-registry.json', 'ai-alloy-policy-static-matrix.json', 'alloy_script_execution_policy'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `rustok-ai-alloy` |', 'crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json', 'scripts/verify/verify-ai-alloy-policy.mjs'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`ai-alloy`', 'ai-alloy-policy-registry.json', 'alloy_script_execution_policy'], 'unified plan');

console.log('[verify-ai-alloy-policy] ai-alloy execution policy metadata and static evidence are consistent');
