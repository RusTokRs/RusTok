import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-search-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const s of snippets) if (!text.includes(s)) fail(`${label} missing ${s}`); }
function hasNone(text, snippets, label) { for (const s of snippets) if (text.includes(s)) fail(`${label} contains forbidden ${s}`); }
function sameList(actual, expected) { return JSON.stringify(actual) === JSON.stringify(expected); }

const registryPath = 'crates/rustok-search/contracts/search-fba-registry.json';
const evidencePath = 'crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json';
const runtimeSmokePath = 'crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json';
const runtimeContractPath = 'crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json';
const runtimeInvocationPath = 'crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json';
const canonicalUrlEvidencePath = 'crates/rustok-search/contracts/evidence/search-canonical-url-contract.json';
const canonicalUrlVerifierPath = 'scripts/verify/verify-search-canonical-url-contract.mjs';
const canonicalUrlSelfTestPath = 'scripts/verify/verify-search-canonical-url-contract.test.mjs';
const blogProjectionEvidencePath = 'crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json';
const blogProjectionVerifierPath = 'scripts/verify/verify-search-blog-projection.mjs';
const blogProjectionSelfTestPath = 'scripts/verify/verify-search-blog-projection.test.mjs';
const removedNavigationPath = 'crates/rustok-search/storefront/src/transport/navigation.rs';
const packageJsonPath = 'package.json';
const expectedVerifySteps = [
  'node scripts/verify/verify-search-fba.mjs',
  'npm run verify:search:canonical-url',
  'npm run verify:search:blog-projection',
  'npm run verify:search:fba:runtime-smoke',
  'npm run verify:search:fba:runtime-contract',
  'npm run verify:search:fba:runtime-invocation',
];
const expectedTestSteps = [
  'npm run test:verify:search:canonical-url',
  'npm run test:verify:search:blog-projection',
  'npm run test:verify:search:fba:runtime-smoke',
  'npm run test:verify:search:fba:runtime-contract',
  'npm run test:verify:search:fba:runtime-invocation',
];
const registry = json(registryPath);
const evidence = json(evidencePath);
const runtimeSmoke = json(runtimeSmokePath);
const runtimeContract = json(runtimeContractPath);
const runtimeInvocation = json(runtimeInvocationPath);
const canonicalUrlEvidence = json(canonicalUrlEvidencePath);
const blogProjectionEvidence = json(blogProjectionEvidencePath);
const packageJson = json(packageJsonPath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'search' || registry.role !== 'provider' || registry.status !== 'boundary_ready') fail('registry identity/status drift');
if (registry.contract_version !== 'search.query.v1') fail('contract_version drift');
if (registry.deployment_topology?.current_class !== 'modular_monolith' || registry.deployment_topology?.extraction_class !== 'whole_module_service' || registry.deployment_topology?.remote_transport !== 'grpc' || registry.deployment_topology?.remote_status !== 'planned') fail('search extraction topology drift');
hasAll(JSON.stringify(registry.deployment_topology.split_blockers), ['search_ingestion_control_contract', 'search_connector_writer_contract', 'query_time_index_sql_reads', 'grpc_conformance', 'isolated_database_evidence'], 'search split blockers');
if (registry.connector_boundary?.owner !== 'search' || registry.connector_boundary?.internal_contract !== 'SearchEngine' || registry.connector_boundary?.planned_writer_contract !== 'SearchEngineWriter' || registry.connector_boundary?.consumer_access !== 'search_ports_only' || registry.connector_boundary?.credentials_exposed_to_consumers !== false) fail('search connector ownership drift');
hasAll(JSON.stringify(registry.connector_boundary), ['postgres', 'meilisearch', 'typesense', 'algolia'], 'connector registry');
const ports = registry.ports ?? [];
for (const expected of ['SearchQueryPort', 'SearchSuggestionPort']) {
  if (!ports.find((p) => p.name === expected)) fail(`missing ${expected}`);
}
for (const port of ports) {
  if (port.context !== 'rustok_api::ports::PortContext' || port.error !== 'rustok_api::ports::PortError') fail(`${port.name} context/error drift`);
  if (!Array.isArray(port.read_operations) || port.read_operations.length === 0) fail(`${port.name} lacks read operations`);
  if ((port.write_operations ?? []).length !== 0) fail(`${port.name} unexpectedly declares write operations`);
}

if (!sameList(packageJson.scripts?.['verify:search:fba']?.split(' && ') ?? [], expectedVerifySteps)) fail('package Search FBA verify chain drift');
if (!sameList(packageJson.scripts?.['test:verify:search:fba']?.split(' && ') ?? [], expectedTestSteps)) fail('package Search FBA test chain drift');
if (packageJson.scripts?.['verify:search:canonical-url'] !== `node ${canonicalUrlVerifierPath}`) fail('canonical URL leaf verifier command drift');
if (packageJson.scripts?.['test:verify:search:canonical-url'] !== `node ${canonicalUrlSelfTestPath}`) fail('canonical URL leaf self-test command drift');
if (packageJson.scripts?.['verify:search:blog-projection'] !== `node ${blogProjectionVerifierPath}`) fail('Blog projection leaf verifier command drift');
if (packageJson.scripts?.['test:verify:search:blog-projection'] !== `node ${blogProjectionSelfTestPath}`) fail('Blog projection leaf self-test command drift');
for (const filePath of [canonicalUrlSelfTestPath, blogProjectionVerifierPath, blogProjectionSelfTestPath, blogProjectionEvidencePath]) {
  if (!fs.existsSync(filePath)) fail(`Search FBA leaf file is missing ${filePath}`);
}

const manifest = read('crates/rustok-search/rustok-module.toml');
hasAll(manifest, ['[fba.provider]', 'registry = "contracts/search-fba-registry.json"', 'contract_version = "search.query.v1"'], 'manifest');
const cargo = read('crates/rustok-search/Cargo.toml');
hasAll(cargo, ['rustok-api'], 'Cargo.toml');
const lib = read('crates/rustok-search/src/lib.rs');
hasAll(lib, ['pub mod ports;', 'pub use ports::*;', 'canonical_search_result_url'], 'lib.rs');
const source = read('crates/rustok-search/src/ports.rs');
hasAll(source, ['pub trait SearchQueryPort', 'pub trait SearchSuggestionPort', 'impl SearchQueryPort for PgSearchEngine', 'impl SearchSuggestionPort for PgSearchEngine', 'PortCallPolicy', 'PortContext', 'PortError', 'search_error_to_port_error'], 'ports.rs');
const queryImpl = source.slice(source.indexOf('impl SearchQueryPort for PgSearchEngine'));
if (!queryImpl.includes('context.require_policy(PortCallPolicy::read())?')) fail('execute_search does not require shared read policy semantics');
if (queryImpl.includes('context.require_write_semantics()?')) fail('execute_search unexpectedly requires write semantics');
if (!queryImpl.includes('request.locale.get_or_insert_with(|| context.locale.clone())')) fail('execute_search lacks locale context fallback');
const suggestionImpl = source.slice(source.indexOf('impl SearchSuggestionPort for PgSearchEngine'));
if (!suggestionImpl.includes('context.require_policy(PortCallPolicy::read())?')) fail('suggest does not require shared read policy semantics');
if (suggestionImpl.includes('context.require_write_semantics()?')) fail('suggest unexpectedly requires write semantics');
if (!suggestionImpl.includes('request.locale.get_or_insert_with(|| context.locale.clone())')) fail('suggest lacks locale context fallback');
if (!suggestionImpl.includes('SearchSuggestionService::suggestions(self.connection(), request)')) fail('suggest does not use embedded PostgreSQL suggestion fallback');
const pgEngine = read('crates/rustok-search/src/pg_engine.rs');
hasAll(pgEngine, ['pub(crate) fn connection(&self) -> &DatabaseConnection', '&self.db'], 'pg_engine.rs');
const engine = read('crates/rustok-search/src/engine.rs');
hasAll(engine, ['pub trait SearchEngine', 'Self::Postgres', 'Self::Meilisearch', 'Self::Typesense', 'Self::Algolia', 'pub fn canonical_search_result_url', 'BLOG_ENTITY_TYPE', 'valid_blog_slug'], 'engine connector and navigation boundary');

const genericSettings = read('apps/server/src/services/settings_service.rs');
hasAll(genericSettings, [
  'ensure_supported_category(cat)?;',
  '.filter(|row| category::ALL.contains(&row.category.as_str()))',
  'generic_category_allowlist_excludes_search_owner_settings',
  '`rustok-search` owns its settings',
], 'generic platform settings boundary');
hasNone(genericSettings, [
  'pub const SEARCH',
  'category::SEARCH',
  'serde_json::to_value(&rs.search)',
], 'generic platform settings boundary');
const ownerSettings = read('crates/rustok-search/src/search_settings.rs');
hasAll(ownerSettings, [
  'pub struct SearchSettingsService',
  'pub async fn load_effective',
  'pub async fn save',
  'table_name = "search_settings"',
], 'Search-owned settings boundary');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
const registryCases = registry.contract_tests.cases.map((c) => c.operation).sort().join('|');
const evidenceCases = evidence.cases.map((c) => c.operation).sort().join('|');
if (registryCases !== evidenceCases) fail('evidence case matrix drift');
if (registry.evidence.runtime_fallback_smoke !== runtimeSmokePath) fail('registry runtime fallback evidence path drift');
if (registry.evidence.runtime_contract_smoke !== runtimeContractPath) fail('registry runtime contract evidence path drift');
if (registry.evidence.runtime_invocation_trace !== runtimeInvocationPath) fail('registry runtime invocation trace evidence path drift');
if (runtimeSmoke.generated_from !== registryPath || runtimeSmoke.status !== registry.contract_tests.fallback_smoke.status) fail('runtime fallback smoke header drift');
if (registry.contract_tests.fallback_smoke.status !== 'executable_no_compile') fail('runtime fallback smoke must be executable no-compile evidence');
if (registry.contract_tests.fallback_smoke.runner !== 'scripts/verify/verify-search-fba-runtime-smoke.mjs') fail('runtime fallback smoke runner drift');
if (runtimeSmoke.runner !== registry.contract_tests.fallback_smoke.runner) fail('runtime fallback smoke evidence runner drift');
const smokeOps = runtimeSmoke.cases.map((c) => c.operation).sort().join('|');
if (smokeOps !== registryCases) fail('runtime fallback smoke case matrix drift');
if (runtimeContract.generated_from !== registryPath || runtimeContract.status !== 'executable_no_compile') fail('runtime contract smoke header drift');
if (runtimeContract.runner !== 'scripts/verify/verify-search-fba-runtime-contract.mjs') fail('runtime contract smoke runner drift');
if (registry.contract_tests.runtime_contract_smoke?.runner !== runtimeContract.runner) fail('runtime contract registry runner drift');
if (runtimeInvocation.generated_from !== registryPath || runtimeInvocation.status !== 'executable_no_compile_invocation_trace') fail('runtime invocation trace header drift');
if (runtimeInvocation.runner !== 'scripts/verify/verify-search-fba-runtime-invocation.mjs') fail('runtime invocation trace runner drift');
if (registry.contract_tests.runtime_invocation_trace?.runner !== runtimeInvocation.runner) fail('runtime invocation trace registry runner drift');
const runtimeContractOps = runtimeContract.cases.map((c) => c.operation).sort().join('|');
if (runtimeContractOps !== registryCases) fail('runtime contract smoke case matrix drift');
const runtimeInvocationOps = runtimeInvocation.cases.map((c) => c.operation).sort().join('|');
if (runtimeInvocationOps !== registryCases) fail('runtime invocation trace case matrix drift');
for (const profile of registry.contract_tests.fallback_smoke.profiles ?? []) {
  if (!runtimeSmoke.profiles.includes(profile)) fail(`runtime fallback smoke missing profile ${profile}`);
}
for (const mode of registry.contract_tests.fallback_smoke.degraded_modes ?? []) {
  if (!JSON.stringify(runtimeSmoke.cases).includes(mode)) fail(`runtime fallback smoke missing degraded mode ${mode}`);
}

if (canonicalUrlEvidence.module !== 'search' || canonicalUrlEvidence.surface !== 'canonical_result_url' || canonicalUrlEvidence.owner !== 'rustok-search') fail('canonical URL evidence identity drift');
if (canonicalUrlEvidence.status !== 'source_verified_no_compile' || canonicalUrlEvidence.compile_policy !== 'not_run_by_request') fail('canonical URL evidence status drift');
const canonicalContract = canonicalUrlEvidence.production_contract ?? {};
for (const [key, expected] of Object.entries({
  normalized_result: 'crates/rustok-search/src/engine.rs',
  public_export: 'crates/rustok-search/src/lib.rs',
  graphql_projection: 'crates/rustok-search/src/graphql/types.rs',
  storefront_native_projection: 'crates/rustok-search/storefront/src/transport/native_server_adapter.rs',
  storefront_transport_facade: 'crates/rustok-search/storefront/src/transport/mod.rs',
  admin_native_root: 'crates/rustok-search/admin/src/transport/native_server_adapter.rs',
  admin_native_mapping: 'crates/rustok-search/admin/src/transport/native_server_adapter/mapping.rs',
  admin_shell_projection: 'apps/admin/src/widgets/app_shell/native_server_adapter.rs',
})) {
  if (canonicalContract[key] !== expected) fail(`canonical URL ${key} path drift`);
}
if ('compatibility_fallback' in canonicalContract) fail('canonical URL compatibility fallback must not exist');
const canonicalVerifier = read(canonicalUrlVerifierPath);
hasAll(canonicalVerifier, ['compatibility implementation must be deleted', 'canonical_search_result_url(&item)', 'no_transport_fallback'], 'canonical URL verifier');
const canonicalSelfTest = read(canonicalUrlSelfTestPath);
hasAll(canonicalSelfTest, ['accepts one Search-owned canonical URL policy', 'forum_reply_canonical_route', 'admin_forum_permission_gate'], 'canonical URL self-test');
if (fs.existsSync(removedNavigationPath)) fail('storefront navigation compatibility file must not exist');
const storefrontFacade = read(canonicalContract.storefront_transport_facade);
hasNone(storefrontFacade, ['mod navigation', 'enrich_search_result_urls', 'blog_result_url'], 'storefront transport facade');
const adminMapping = read(canonicalContract.admin_native_mapping);
hasAll(adminMapping, ['rustok_search::canonical_search_result_url(&item)'], 'admin Search mapping');
hasNone(adminMapping, ['fn derive_search_result_url', '"/modules/blog"'], 'admin Search mapping');
const adminShell = read(canonicalContract.admin_shell_projection);
hasAll(adminShell, ['rustok_search::canonical_search_result_url(&item)', '("blog_post", "blog" | "rustok-blog")'], 'admin global Search mapping');
hasNone(adminShell, ['fn derive_admin_search_result_url', '"/modules/blog"'], 'admin global Search mapping');

if (blogProjectionEvidence.module !== 'search' || blogProjectionEvidence.surface !== 'blog_post_projection') fail('Blog projection evidence identity drift');
if (blogProjectionEvidence.status !== 'executable_no_run' || blogProjectionEvidence.compile_policy !== 'not_run_by_request') fail('Blog projection evidence status drift');
if (blogProjectionEvidence.production_contract?.source_guardrail !== blogProjectionVerifierPath) fail('Blog projection source guardrail path drift');
for (const target of [
  'crates/rustok-search/tests/blog_ingestion_contract_test.rs',
  'crates/rustok-search/tests/blog_projection_postgres_test.rs',
]) {
  if (!(blogProjectionEvidence.test_targets ?? []).includes(target)) fail(`Blog projection evidence missing test target ${target}`);
}
const blogProjectionVerifier = read(blogProjectionVerifierPath);
hasAll(blogProjectionVerifier, ['search-blog-projection-postgres-harness.json', 'targeted_missing_post_cleanup', 'module_toggle_cleanup_rebuild'], 'Blog projection verifier');
const blogProjectionSelfTest = read(blogProjectionSelfTestPath);
hasAll(blogProjectionSelfTest, ['accepts canonical owner-tag source', 'rejects metadata tags as Search projection source', 'rejects missing Taxonomy table availability gate'], 'Blog projection self-test');

const plan = read('crates/rustok-search/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `boundary_ready`', 'search-fba-registry.json', 'SearchQueryPort', 'search-contract-test-static-matrix.json', 'search-runtime-fallback-smoke.json', 'search-runtime-contract-smoke.json', 'search-runtime-invocation-trace.json', 'whole-module extraction pilot', 'SearchEngine', '2026-07-16-media-search-extraction-boundaries.md', 'search-canonical-url-contract.json', 'single owner policy', 'no transport fallback', 'verify:search:canonical-url', 'test:verify:search:canonical-url', 'verify:search:blog-projection', 'test:verify:search:blog-projection'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `search` |', 'crates/rustok-search/contracts/search-fba-registry.json', '`phase_b_ready` | `boundary_ready`'], 'central registry');

console.log('[verify-search-fba] Search provider metadata, exact canonical URL and Blog projection leaf commands, port semantics, owner-only settings, current-only navigation ownership, static evidence, and executable no-compile runtime contracts are consistent');
