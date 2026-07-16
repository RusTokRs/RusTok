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
  if (smoke.generated_from !== registryPath || !['source_smoke_locked', 'runtime_verified'].includes(smoke.status)) fail(`${registry.module} fallback smoke header drift`);
}

function verifySupportAdapter({ module, registryPath, sourceMarkers, bindingMarkers = [], forbiddenBindingMarkers = [], planMarkers }) {
  const registry = json(registryPath);
  if (registry.schema_version !== 1 || registry.module !== module || !['in_progress', 'boundary_ready'].includes(registry.status)) fail(`${module} registry identity/status drift`);
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
  hasAll(plan, [`- FBA status: \`${registry.status}\``, registryPath, registry.evidence.static_matrix, registry.evidence.runtime_fallback_smoke, ...planMarkers], `${module} local plan`);
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
if (authRegistry.schema_version !== 1 || authRegistry.module !== 'auth' || authRegistry.role !== 'core_capability_provider' || !['in_progress', 'boundary_ready'].includes(authRegistry.status)) fail('auth registry identity/status drift');
if (authRegistry.contract_tests?.source !== authRegistryPath || authRegistry.contract_tests?.runner !== 'scripts/verify/verify-ai-fba-baseline.mjs') fail('auth contract test source/runner drift');
verifyEvidence(authRegistry, authRegistryPath);
const authSource = read('crates/rustok-auth/src/lib.rs');
hasAll(authSource, ['AUTH_USER_PERMISSIONS', 'Permission::USERS_CREATE', 'Permission::USERS_READ', 'Permission::USERS_UPDATE', 'Permission::USERS_DELETE', 'Permission::USERS_LIST', 'Permission::USERS_MANAGE', 'fn permissions(&self) -> Vec<Permission>'], 'auth source');
const authAdminCore = read(authRegistry.admin_boundary.core_policy);
hasAll(authAdminCore, ['prepare_login_request', 'prepare_register_request', 'prepare_password_reset_request', 'prepare_profile_name', 'classify_auth_transport_error', 'AuthTransportErrorKind'], 'auth admin core');
const authPlan = read(authRegistry.evidence.local_plan);
hasAll(authPlan, [`- FBA status: \`${authRegistry.status}\``, authRegistryPath, authRegistry.evidence.static_matrix, authRegistry.evidence.runtime_fallback_smoke, 'AUTH_USER_PERMISSIONS'], 'auth local plan');
const central = read(authRegistry.evidence.central_registry);
hasAll(central, ['| `auth` |', authRegistryPath, authRegistry.evidence.runtime_fallback_smoke], 'auth central registry');

const aiRegistryPath = 'crates/rustok-ai/contracts/ai-fba-registry.json';
const aiRegistry = json(aiRegistryPath);
if (aiRegistry.schema_version !== 1 || aiRegistry.module !== 'ai' || aiRegistry.role !== 'capability_orchestrator' || !['in_progress', 'boundary_ready'].includes(aiRegistry.status)) fail('ai registry identity/status drift');
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
const aiAdminModel = read('crates/rustok-ai/admin/src/model.rs');
hasAll(aiAdminModel, ['agent_model_assignments: Vec<AiAgentModelAssignmentPayload>'], 'ai admin bootstrap model');
const aiAdminNative = read('crates/rustok-ai/admin/src/transport/native_server_adapter.rs');
hasAll(aiAdminNative, ['list_tenant_agent_model_assignments', 'map_agent_model_assignment'], 'ai native assignment bootstrap');
const aiAdminGraphql = read('crates/rustok-ai/admin/src/transport/graphql_adapter.rs');
hasAll(aiAdminGraphql, ['aiAgentModelAssignments {', 'agentPrincipalId providerProfileId modelOverride executionMode isActive', 'aiTenantRbacRoles { slug displayName permissionSlugs }', 'AI_CREATE_AGENT_PRINCIPAL_MUTATION', 'AI_UPDATE_AGENT_PRINCIPAL_MUTATION', 'AI_CREATE_AGENT_MODEL_ASSIGNMENT_MUTATION', 'AI_UPDATE_AGENT_MODEL_ASSIGNMENT_MUTATION', 'AI_CREATE_AGENT_WORKFLOW_RUN_MUTATION'], 'ai GraphQL assignment bootstrap');
const aiQuery = read('crates/rustok-ai/src/graphql/query.rs');
hasAll(aiQuery, ['agent_principal_id: Option<Uuid>', 'list_tenant_agent_model_assignments'], 'ai assignment query');
hasAll(aiQuery, ['tenant RBAC catalog is unavailable'], 'ai RBAC catalog read failure');
const aiGraphqlTypes = read('crates/rustok-ai/src/graphql/types.rs');
const providerUsagePolicyInput = aiGraphqlTypes.match(/pub struct AiProviderUsagePolicyInputGql \{([\s\S]*?)\n\}/);
if (!providerUsagePolicyInput) fail('ai provider usage-policy input is missing');
hasNone(providerUsagePolicyInput[1], ['restricted_role_slugs'], 'ai provider usage-policy input');
const aiProviderNative = read('crates/rustok-ai/admin/src/transport/native_server_adapter.rs');
hasNone(aiProviderNative, ['restricted_role_slugs: Vec<String>'], 'ai native provider input');
const aiProviderUi = read('crates/rustok-ai/admin/src/ui/leptos.rs');
hasNone(aiProviderUi, ['provider_restricted_roles'], 'ai provider UI input');
const aiRuntimeFactory = read('crates/rustok-ai/src/service/types.rs');
hasAll(aiRuntimeFactory, ['pub fn ai_host_runtime_from_context(', 'AiHostRuntime::new(', 'pub(crate) fn new(', 'AI requires SharedAiSecretResolverRegistry', 'AI requires SharedAiEgressPolicy', 'AI requires SharedAiProviderTargetCatalog'], 'ai runtime factory');
hasNone(aiRuntimeFactory, ['with_secret_registry', 'with_egress_policy', 'with_provider_targets', 'AiProviderTargetCatalog::from_environment()'], 'ai runtime factory fallbacks');
const aiDeploymentRuntime = read('crates/rustok-ai/src/runtime_extensions.rs');
hasAll(aiDeploymentRuntime, ['RUSTOK_AI_SECRET_RESOLVERS_JSON', 'DeploymentSecretResolverConfig', 'VaultResolver::new', 'KubernetesSecretResolver::in_cluster', 'LazyAwsResolver', 'LazyGcpResolver', 'AzureKeyVaultResolver::from_default_credential', 'secret resolver aliases must be unique and non-empty'], 'AI deployment secret resolver composition');
const runtimeExtensions = read('crates/rustok-core/src/module.rs');
hasAll(runtimeExtensions, ['pub fn apply_to_host_runtime(', 'host.with_extension_values('], 'neutral module-to-host runtime extension bridge');
const runtimeScheduler = read('crates/rustok-runtime/src/lib.rs');
hasAll(runtimeScheduler, ['pub trait ModuleWorkRegistration', 'pub struct ModuleWorkRegistrations', 'pub async fn run_until_stopped(', 'stop.changed()'], 'generic module work lifecycle');
const genericServerRuntime = read('apps/server/src/services/app_runtime.rs');
hasAll(genericServerRuntime, ['initialize_module_work_runtime', 'ModuleWorkRegistrations', 'run_until_stopped(stop'], 'generic server work composition');
hasNone(genericServerRuntime, ['rustok_ai', 'AiHostRuntime', 'AiProviderTargetCatalog', 'SecretResolverRegistry'], 'generic server work composition AI ownership');
const aiReadme = read('crates/rustok-ai/README.md');
hasAll(aiReadme, ['RUSTOK_AI_SECRET_RESOLVERS_JSON', '`aws_secrets_manager`', '`gcp_secret_manager`', '`azure_key_vault`', 'Tenant profiles persist only'], 'AI deployment secret resolver documentation');
const aiModule = read('crates/rustok-ai/src/lib.rs');
hasAll(aiModule, ['pub struct AiModule', 'fn kind(&self) -> rustok_core::ModuleKind', 'rustok_core::ModuleKind::Core'], 'AI deployment-scoped module registration');
const aiAgents = read('crates/rustok-ai/src/agent.rs');
hasAll(aiAgents, ['Product,', 'Code,', 'Orchestrator,', 'Review,', 'AgentKind::Product'], 'AI product/code agent taxonomy');
hasNone(aiAgents, ['AgentKind::Domain', '    Domain,'], 'AI legacy domain agent taxonomy');
const aiGraphqlRuntime = read('crates/rustok-ai/src/graphql_runtime.rs');
hasAll(aiGraphqlRuntime, ['ai_host_runtime_from_context(inputs.host())'], 'ai GraphQL runtime factory binding');
hasNone(aiGraphqlRuntime, ['AiHostRuntime::new(', 'AiGraphqlRoleSlugProvider', 'SELECT roles.slug'], 'ai GraphQL runtime ownership');
hasNone(aiProviderNative, ['AiHostRuntime::new(', 'SELECT roles.slug'], 'ai native runtime/RBAC ownership');
const aiRouter = read('crates/rustok-ai/src/router.rs');
hasAll(aiRouter, ['provider role restriction awaits the platform tenant RBAC catalog'], 'ai router legacy role policy');
const aiSchedulerAdapter = read('crates/rustok-ai/src/scheduler.rs');
hasAll(aiSchedulerAdapter, ['impl ModuleWorkSource for AiAgentWorkflowWorkAdapter', 'impl ModuleWorkHandler for AiAgentWorkflowWorkAdapter', 'claim_agent_workflow_stage', 'execute_agent_workflow_stage', 'requeue_expired_agent_stage_leases', 'self.recover_expired_leases().await?', 'pub async fn register_with', 'scheduler.register(adapter.clone(), adapter)'], 'ai generic scheduler adapter');
const moduleScheduler = read('crates/rustok-runtime/src/lib.rs');
hasAll(moduleScheduler, ['workers: Arc<RwLock<BTreeMap<String, RegisteredModuleWork>>>', 'pub fn new() -> Self', 'source: Arc<dyn ModuleWorkSource>', 'handler: Arc<dyn ModuleWorkHandler>'], 'generic module work scheduler registry');
const tenantRbacContract = read('crates/rustok-api/src/tenant_rbac.rs');
hasAll(tenantRbacContract, ['pub trait TenantRbacCatalog: Send + Sync', 'pub struct SharedTenantRbacCatalog'], 'tenant RBAC catalog contract');
const tenantRbacProvider = read('crates/rustok-rbac/src/catalog.rs');
hasAll(tenantRbacProvider, ['impl TenantRbacCatalog for BuiltinTenantRbacCatalog', 'validate_assignment'], 'tenant RBAC catalog provider');
const rbacModule = read('crates/rustok-rbac/src/lib.rs');
hasAll(rbacModule, ['SharedTenantRbacCatalog(Arc::new(BuiltinTenantRbacCatalog))'], 'tenant RBAC runtime registration');
const aiService = read('crates/rustok-ai/src/service.rs');
hasAll(aiService, ['fn resolve_agent_principal_rbac(', '.validate_assignment(tenant_id, &role_slugs, &[])', 'selected agent roles do not grant every permission required', 'role_slugs: principal.role_slugs.iter().cloned().collect()'], 'ai agent role assignment service');
const createPrincipalInput = aiGraphqlTypes.match(/pub struct CreateAiAgentPrincipalInputGql \{([\s\S]*?)\n\}/);
const updatePrincipalInput = aiGraphqlTypes.match(/pub struct UpdateAiAgentPrincipalInputGql \{([\s\S]*?)\n\}/);
if (!createPrincipalInput || !updatePrincipalInput) fail('ai agent-principal mutation inputs are missing');
hasAll(createPrincipalInput[1], ['role_slugs: Vec<String>'], 'ai create-principal input');
hasAll(updatePrincipalInput[1], ['role_slugs: Vec<String>'], 'ai update-principal input');
const aiMutation = read('crates/rustok-ai/src/graphql/mutation.rs');
hasAll(aiMutation, ['tenant RBAC catalog is unavailable', 'tenant_rbac_catalog.as_ref()', 'role_slugs: input.role_slugs'], 'ai agent role assignment mutations');
hasAll(aiProviderNative, ['pub async fn create_agent_principal(', 'pub async fn update_agent_principal(', 'endpoint = "ai/create-agent-principal"', 'endpoint = "ai/update-agent-principal"', 'SharedTenantRbacCatalog', 'role_slugs,'], 'ai native agent role assignment mutations');
hasNone(aiProviderNative, ['tenant_rbac_catalog\n                .as_ref()', 'tenant_rbac_catalog\r\n                .as_ref()'], 'ai native RBAC catalog fallback');
hasAll(aiProviderNative, ['pub async fn create_agent_model_assignment(', 'pub async fn update_agent_model_assignment(', 'endpoint = "ai/create-agent-model-assignment"', 'endpoint = "ai/update-agent-model-assignment"', 'provider_profile_id: parse_uuid', 'execution_mode: parse_execution_mode'], 'ai native agent model-assignment mutations');
hasAll(aiProviderNative, ['pub async fn create_agent_workflow_run(', 'endpoint = "ai/create-agent-workflow-run"', 'stage_principal_ids', 'stage_model_assignment_ids', 'stage_input_payloads'], 'ai native workflow-run mutation');
const aiAgentPanel = read('crates/rustok-ai/admin/src/ui/components/agent_panel.rs');
hasAll(aiAgentPanel, ['AiAgentPrincipalCreateForm', 'AiAgentPrincipalUpdateForm', 'descriptor_catalog_for_create', 'role_choices_for_create', 'role_choices_for_update', 'on_create_principal.run', 'on_update_principal.run'], 'ai catalog-driven role editor');
hasNone(aiAgentPanel, ['role_slugs_csv', 'permission_slugs_csv', 'descriptor_owner: RwSignal'], 'ai catalog-driven role editor inputs');
hasAll(aiAgentPanel, ['AiAgentModelAssignmentCreateForm', 'AiAgentModelAssignmentUpdateForm', 'assignment_principal_choices', 'assignment_provider_choices', 'AiExecutionModeSelector', 'on_create_assignment.run', 'on_update_assignment.run'], 'ai catalog-driven model-assignment editor');
hasNone(aiAgentPanel, ['ai.field.agentPrincipalId', 'ai.field.providerProfileId', 'Principal id', 'Provider profile id'], 'ai catalog-driven model-assignment editor inputs');
hasAll(aiProviderUi, ['transport::create_agent_principal(', 'transport::update_agent_principal(', 'transport::create_agent_model_assignment(', 'transport::update_agent_model_assignment(', 'selected_agent_roles'], 'ai native role editor binding');
const aiPlan = read(aiRegistry.evidence.local_plan);
hasAll(aiPlan, ['- FBA status: `boundary_ready`', aiRegistryPath, aiRegistry.evidence.static_matrix, aiRegistry.evidence.runtime_fallback_smoke, 'verify-ai-fba-baseline.mjs'], 'ai local plan');
hasAll(central, ['| `ai` |', aiRegistryPath, aiRegistry.evidence.runtime_fallback_smoke], 'ai central registry');
const packageJson = read('package.json');
hasAll(packageJson, ['verify:ai:fba-baseline', 'verify:ai-content:fba', 'verify:ai-order:fba', 'verify:auth:fba', 'verify:ai:fba'], 'package scripts');

console.log('[verify-ai-fba-baseline] ai/content/order/auth FBA baseline metadata and static evidence are consistent');
