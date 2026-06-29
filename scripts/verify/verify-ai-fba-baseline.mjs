import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-ai-fba-baseline] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const snippet of snippets) if (!text.includes(snippet)) fail(`${label} missing ${snippet}`); }
function hasNone(text, snippets, label) { for (const snippet of snippets) if (text.includes(snippet)) fail(`${label} must not include ${snippet}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}
function verifyEvidence(registry, registryPath) {
  const evidence = json(registry.evidence.static_matrix);
  const smoke = json(registry.evidence.runtime_fallback_smoke);
  if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail(`${registry.module} evidence header drift`);
  sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), `${registry.module} evidence cases`);
  sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, `${registry.module} fallback profiles`);
  sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, `${registry.module} degraded modes`);
  if (smoke.generated_from !== registryPath || smoke.status !== 'source_smoke_locked') fail(`${registry.module} fallback smoke header drift`);
}

function verifySupportAdapter({ module, registryPath, sourceMarkers, bindingMarkers = [], forbiddenBindingMarkers = [], planMarkers }) {
  const registry = json(registryPath);
  if (registry.schema_version !== 1 || registry.module !== module || registry.status !== 'in_progress') fail(`${module} registry identity/status drift`);
  if (registry.contract_tests?.source !== registryPath || registry.contract_tests?.runner !== 'scripts/verify/verify-ai-fba-baseline.mjs') fail(`${module} contract test source/runner drift`);
  verifyEvidence(registry, registryPath);
  const source = read(registry.support_adapter.source);
  hasAll(source, sourceMarkers, `${module} support adapter source`);
  if (registry.support_adapter.runtime_binding) {
    const binding = read(registry.support_adapter.runtime_binding);
    hasAll(binding, bindingMarkers, `${module} runtime binding`);
    hasNone(binding, forbiddenBindingMarkers, `${module} runtime binding ownership`);
  }
  const plan = read(registry.evidence.local_plan);
  hasAll(plan, ['- FBA status: `in_progress`', registryPath, registry.evidence.static_matrix, registry.evidence.runtime_fallback_smoke, ...planMarkers], `${module} local plan`);
  const central = read(registry.evidence.central_registry);
  hasAll(central, [`| \`${module}\` |`, registryPath, registry.evidence.runtime_fallback_smoke], `${module} central registry`);
  return registry;
}

verifySupportAdapter({
  module: 'ai-content',
  registryPath: 'crates/rustok-ai-content/contracts/ai-content-fba-registry.json',
  sourceMarkers: [
    'CONTENT_MODERATION_TASK_SLUG: &str = "content_moderation"',
    'BLOG_DRAFT_TASK_SLUG: &str = "blog_draft"',
    'CONTENT_AI_POLICY_MATRIX',
    'content_ai_sensitive_tools',
    'register_content_ai_vertical_handlers',
    'validate_blog_draft_payload',
    'validate_moderation_decision'
  ],
  bindingMarkers: ['register_content_ai_vertical_handlers', 'CONTENT_MODERATION_TASK_SLUG', 'BLOG_DRAFT_TASK_SLUG'],
  forbiddenBindingMarkers: ['"content_moderation"', '"blog_draft"'],
  planMarkers: ['content_ai_policy_matrix', 'require_operator_review']
});

const aiOrder = verifySupportAdapter({
  module: 'ai-order',
  registryPath: 'crates/rustok-ai-order/contracts/ai-order-fba-registry.json',
  sourceMarkers: [
    'ORDER_ANALYTICS_TASK_SLUG: &str = "order_analytics"',
    'ORDER_OPS_ASSISTANT_TASK_SLUG: &str = "order_ops_assistant"',
    'register_order_ai_vertical_handlers',
    'validate_order_analytics_payload',
    'validate_order_ops_assistant_payload',
    'validate_order_ops_assistant_confidence'
  ],
  bindingMarkers: ['register_order_ai_vertical_handlers', 'ORDER_ANALYTICS_TASK_SLUG', 'ORDER_OPS_ASSISTANT_TASK_SLUG'],
  forbiddenBindingMarkers: ['"order_analytics"', '"order_ops_assistant"'],
  planMarkers: ['CheckoutCompletionPort', 'generate_summary_without_live_status']
});
const orderProvider = json('crates/rustok-order/contracts/order-fba-registry.json');
const orderDependency = aiOrder.provider_dependencies?.[0];
if (orderDependency?.contract_version !== orderProvider.contract_version || orderDependency?.port !== 'CheckoutCompletionPort') fail('ai-order provider dependency drift');
if (!(orderProvider.ports?.[0]?.operations ?? []).includes('read_order_status')) fail('order provider lacks read_order_status operation');

const authRegistryPath = 'crates/rustok-auth/contracts/auth-fba-registry.json';
const authRegistry = json(authRegistryPath);
if (authRegistry.schema_version !== 1 || authRegistry.module !== 'auth' || authRegistry.role !== 'core_capability_provider' || authRegistry.status !== 'in_progress') fail('auth registry identity/status drift');
if (authRegistry.contract_tests?.source !== authRegistryPath || authRegistry.contract_tests?.runner !== 'scripts/verify/verify-ai-fba-baseline.mjs') fail('auth contract test source/runner drift');
verifyEvidence(authRegistry, authRegistryPath);
const authSource = read('crates/rustok-auth/src/lib.rs');
hasAll(authSource, ['AUTH_USER_PERMISSIONS', 'Permission::USERS_CREATE', 'Permission::USERS_READ', 'Permission::USERS_UPDATE', 'Permission::USERS_DELETE', 'Permission::USERS_LIST', 'Permission::USERS_MANAGE', 'fn permissions(&self) -> Vec<Permission>'], 'auth source');
const authAdminCore = read(authRegistry.admin_boundary.core_policy);
hasAll(authAdminCore, ['prepare_login_request', 'prepare_register_request', 'prepare_password_reset_request', 'prepare_profile_name', 'classify_profile_update_error'], 'auth admin core');
const authPlan = read(authRegistry.evidence.local_plan);
hasAll(authPlan, ['- FBA status: `in_progress`', authRegistryPath, authRegistry.evidence.static_matrix, authRegistry.evidence.runtime_fallback_smoke, 'AUTH_USER_PERMISSIONS'], 'auth local plan');
const central = read(authRegistry.evidence.central_registry);
hasAll(central, ['| `auth` |', authRegistryPath, authRegistry.evidence.runtime_fallback_smoke], 'auth central registry');

const aiRegistryPath = 'crates/rustok-ai/contracts/ai-fba-registry.json';
const aiRegistry = json(aiRegistryPath);
if (aiRegistry.schema_version !== 1 || aiRegistry.module !== 'ai' || aiRegistry.role !== 'capability_orchestrator' || aiRegistry.status !== 'in_progress') fail('ai registry identity/status drift');
if (aiRegistry.contract_tests?.source !== aiRegistryPath || aiRegistry.contract_tests?.runner !== 'scripts/verify/verify-ai-fba-baseline.mjs') fail('ai contract test source/runner drift');
verifyEvidence(aiRegistry, aiRegistryPath);
for (const adapter of aiRegistry.support_adapters) {
  const adapterRegistry = json(adapter.registry);
  if (adapterRegistry.module !== adapter.module) fail(`ai support adapter ${adapter.module} registry drift`);
  const binding = read(adapter.runtime_binding);
  hasAll(binding, [adapter.registration_api], `ai support adapter ${adapter.module} runtime binding`);
}
const router = read(aiRegistry.runtime_contracts.router_policy_source);
hasAll(router, ['ResolvedExecutionPlan', 'candidate', 'fallback'], 'ai router policy source');
const direct = read(aiRegistry.runtime_contracts.direct_registry_source);
hasAll(direct, ['DirectExecutionRegistry', 'DirectTaskHandler'], 'ai direct registry source');
const aiPlan = read(aiRegistry.evidence.local_plan);
hasAll(aiPlan, ['- FBA status: `in_progress`', aiRegistryPath, aiRegistry.evidence.static_matrix, aiRegistry.evidence.runtime_fallback_smoke, 'verify-ai-fba-baseline.mjs'], 'ai local plan');
hasAll(central, ['| `ai` |', aiRegistryPath, aiRegistry.evidence.runtime_fallback_smoke], 'ai central registry');
const packageJson = read('package.json');
hasAll(packageJson, ['verify:ai:fba-baseline', 'verify:ai-content:fba', 'verify:ai-order:fba', 'verify:auth:fba', 'verify:ai:fba'], 'package scripts');

console.log('[verify-ai-fba-baseline] ai/content/order/auth FBA baseline metadata and static evidence are consistent');
