import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-ai-product-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const snippet of snippets) if (!text.includes(snippet)) fail(`${label} missing ${snippet}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-ai-product/contracts/ai-product-fba-registry.json';
const evidencePath = 'crates/rustok-ai-product/contracts/evidence/ai-product-consumer-static-matrix.json';
const fallbackSmokePath = 'crates/rustok-ai-product/contracts/evidence/ai-product-runtime-fallback-smoke.json';
const providerPath = 'crates/rustok-product/contracts/product-fba-registry.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const fallbackSmoke = json(fallbackSmokePath);
const provider = json(providerPath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'ai-product' || registry.crate !== 'rustok-ai-product' || registry.role !== 'consumer_support_adapter' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('registry identity/status drift');
if (registry.consumer_profile !== 'ai_product_generation_context') fail('consumer profile drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing product provider dependency');
if (dependency.module !== 'product' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'ProductCatalogReadPort') fail('provider contract/port drift');
sameSet(dependency.consuming_tasks ?? [], ['product_attributes'], 'product catalog read consuming tasks');
const productConsumer = provider.consumers?.find(c => c.module === 'ai-product');
if (!productConsumer) fail('product provider registry lacks ai-product consumer profile');
sameSet(dependency.fallback_profiles, productConsumer.fallback_profiles, 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, productConsumer.degraded_modes, 'consumer/provider degraded modes');
for (const operation of dependency.operations) if (!(provider.ports?.[0]?.operations ?? []).includes(operation)) fail(`consumer operation ${operation} is absent from product provider`);

const source = read(registry.support_adapter.source);
hasAll(source, [
  'PRODUCT_COPY_TASK_SLUG: &str = "product_copy"',
  'PRODUCT_ATTRIBUTES_TASK_SLUG: &str = "product_attributes"',
  'PRODUCT_COPY_TOOL_NAME: &str = "direct.commerce.product_copy"',
  'PRODUCT_ATTRIBUTES_TOOL_NAME: &str = "direct.commerce.product_attributes"',
  'register_product_ai_vertical_handlers',
  'validate_product_copy_payload',
  'validate_product_attributes_payload',
  'ProductAiAgentDescriptor',
  'ProductAiWorkflowDescriptor',
  'PRODUCT_AI_AGENTS',
  'PRODUCT_AI_WORKFLOWS',
  'product_ai_agents',
  'product_ai_workflows',
  'validate_product_agent_stage_input',
  'slug: "product_enrichment"'
], 'support adapter source');

const agentCatalog = registry.agent_catalog;
if (!agentCatalog || agentCatalog.owner !== 'rustok-ai-product') fail('product agent catalog owner drift');
if (agentCatalog.catalog_api !== 'product_ai_agents' || agentCatalog.workflow_api !== 'product_ai_workflows' || agentCatalog.stage_input_validation_api !== 'validate_product_agent_stage_input') fail('product agent catalog API drift');
sameSet(agentCatalog.roles ?? [], ['product_copywriter', 'product_attribute_enricher'], 'product agent roles');
if (agentCatalog.workflow !== 'product_enrichment' || agentCatalog.all_stages_require_approval !== true) fail('product agent workflow policy drift');

const aiAgentCatalog = read('crates/rustok-ai/src/agent.rs');
hasAll(aiAgentCatalog, [
  'rustok_ai_product::product_ai_agents()',
  'rustok_ai_product::product_ai_workflows()',
  'rustok_ai_product::validate_product_agent_stage_input',
  'AgentStageValidator::Product',
  'with_stage_validators',
  'owner: "rustok-ai-product"'
], 'AI owner catalog composition');
const aiService = read('crates/rustok-ai/src/service.rs');
hasAll(aiService, [
  'catalog.validate_stage_execution(',
  'Self::run_task_job_with_authority(',
  'TaskJobExecutionAuthority::RegisteredAgentAssignment'
], 'product agent canonical task-run composition');

const attributesHandler = read('crates/rustok-ai/src/direct_product_attributes.rs');
hasAll(attributesHandler, [
  'runtime.product_catalog_read_port()',
  'read_product_projection(',
  'tokio::time::timeout(',
  'ai_product.catalog_read_port_unavailable',
  'ai_product.catalog_read_port_deadline_exceeded',
  '"catalog_enrichment": "skipped"',
  '"review_required": true',
  '"persistence": "none"'
], 'product attributes owner-port adapter');
if (attributesHandler.includes('CatalogService')) {
  fail('product attributes must not bypass ProductCatalogReadPort with CatalogService');
}

const runtimeTypes = read('crates/rustok-ai/src/service/types.rs');
hasAll(runtimeTypes, [
  'SharedAiProductCatalogReadPort',
  'product_catalog_read_port',
  'with_product_catalog_read_port'
], 'AI product catalog runtime composition');
const commerceRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
hasAll(commerceRuntime, [
  'Arc<dyn rustok_product::ProductCatalogReadPort>',
  'SharedAiProductCatalogReadPort'
], 'server product catalog runtime composition');
const directTests = read('crates/rustok-ai/src/direct.rs');
hasAll(directTests, [
  'direct_product_attributes_returns_review_only_suggestions_without_product_write',
  'direct_product_attributes_degrades_when_catalog_port_is_unavailable',
  'direct_product_attributes_degrades_when_catalog_port_exceeds_its_deadline'
], 'product attributes direct runtime evidence');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.cases[0]?.assertions ?? [], registry.contract_tests.cases[0]?.assertions ?? [], 'evidence/registry assertions');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');
if (fallbackSmoke.generated_from !== registryPath || !['source_smoke_locked', 'runtime_verified'].includes(fallbackSmoke.status)) fail('fallback smoke header drift');
if (fallbackSmoke.profile !== registry.contract_tests.fallback_smoke.profiles[0]) fail('fallback smoke profile drift');
if (fallbackSmoke.degraded_mode !== registry.contract_tests.fallback_smoke.degraded_modes[0]) fail('fallback smoke degraded mode drift');
sameSet(fallbackSmoke.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'fallback smoke cases');

const plan = read('crates/rustok-ai-product/docs/implementation-plan.md');
hasAll(plan, [`- FBA status: \`${registry.status}\``, 'ai-product-fba-registry.json', 'ProductCatalogReadPort', 'ai-product-consumer-static-matrix.json', 'ai-product-runtime-fallback-smoke.json'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `ai-product` |', 'crates/rustok-ai-product/contracts/ai-product-fba-registry.json', 'crates/rustok-ai-product/contracts/evidence/ai-product-runtime-fallback-smoke.json'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`ai-product`', 'ProductCatalogReadPort', 'ai-product-fba-registry.json', 'ai-product-runtime-fallback-smoke.json'], 'unified plan');

console.log('[verify-ai-product-fba] ai-product FBA product consumer support metadata and static evidence are consistent');
