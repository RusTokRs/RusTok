import { readFile } from 'node:fs/promises';
import process from 'node:process';

const root = process.cwd();
const read = (path) => readFile(`${root}/${path}`, 'utf8');

const [
  cargo,
  entityMod,
  entity,
  migrationMod,
  migration,
  serviceMod,
  service,
  graphqlMod,
  graphqlBaseline,
  mutation,
  adminModel,
  adminAdapter,
  adminCasAdapter,
  adminStatusAdapter,
  adminTransport,
  composition,
  releaseCore,
  releaseApi,
] = await Promise.all([
  read('crates/rustok-pages/Cargo.toml'),
  read('crates/rustok-pages/src/entities/mod.rs'),
  read('crates/rustok-pages/src/entities/page_builder_scenario_baseline.rs'),
  read('crates/rustok-pages/src/migrations/mod.rs'),
  read('crates/rustok-pages/src/migrations/m20260714_000001_create_page_builder_scenario_baselines.rs'),
  read('crates/rustok-pages/src/services/mod.rs'),
  read('crates/rustok-pages/src/services/scenario_baseline.rs'),
  read('crates/rustok-pages/src/graphql/mod.rs'),
  read('crates/rustok-pages/src/graphql/scenario_baseline.rs'),
  read('crates/rustok-pages/src/graphql/mutation.rs'),
  read('crates/rustok-pages/admin/src/model.rs'),
  read('crates/rustok-pages/admin/src/transport/graphql_adapter.rs'),
  read('crates/rustok-pages/admin/src/transport/scenario_baseline_cas_adapter.rs'),
  read('crates/rustok-pages/admin/src/transport/scenario_release_adapter.rs'),
  read('crates/rustok-pages/admin/src/transport/mod.rs'),
  read('crates/rustok-pages/admin/src/composition.rs'),
  read('crates/fly/src/runtime_scenario_release.rs'),
  read('crates/rustok-page-builder/src/runtime_scenario_release.rs'),
]);

const required = [
  [cargo, 'rustok-page-builder = { path = "../rustok-page-builder", default-features = false }', 'Pages must depend on the Page Builder release contract'],
  [entityMod, 'pub mod page_builder_scenario_baseline;', 'scenario baseline entity is not registered'],
  [entity, 'table_name = "page_builder_scenario_baselines"', 'scenario baseline table mapping is missing'],
  [entity, 'pub tenant_id: Uuid', 'scenario baseline entity is not tenant scoped'],
  [entity, 'pub baseline_hash: String', 'scenario baseline integrity hash column is missing'],
  [migrationMod, 'm20260714_000001_create_page_builder_scenario_baselines', 'scenario baseline migration is not registered'],
  [migration, 'idx_page_builder_scenario_baselines_tenant_page', 'tenant/page uniqueness index is missing'],
  [migration, '.unique()', 'scenario baseline must be unique per tenant/page'],
  [migration, 'ForeignKeyAction::Cascade', 'scenario baseline must cascade with page deletion'],
  [serviceMod, 'PageBuilderScenarioBaselineService', 'scenario baseline service is not exported'],
  [service, 'enforce_owned_scope', 'scenario baseline service does not enforce page ownership'],
  [service, 'baseline.validate()', 'scenario baseline service does not validate integrity'],
  [service, 'baseline.baseline_hash != model.baseline_hash', 'stored baseline columns are not cross-checked'],
  [service, 'RuntimeScenarioReleasePolicy::block_broken()', 'Pages publish evaluation does not block broken regressions'],
  [service, 'ensure_published_candidate_allowed', 'published Page Builder updates are not scenario-gated'],
  [service, 'if page.status != "published"', 'draft updates must remain outside the release gate'],
  [service, 'PAGE_BUILDER_SCENARIO_BASELINE_CONFLICT_ERROR_CODE', 'stable baseline conflict code is missing'],
  [service, 'save_if_current', 'baseline compare-and-swap save is missing'],
  [service, 'delete_if_current', 'baseline compare-and-swap delete is missing'],
  [service, 'Column::BaselineHash.eq(expected_hash)', 'baseline mutations are not conditionally scoped by expected hash'],
  [service, '(Some(_), None) if enforce_expected_state', 'expected absent baseline does not reject an existing row'],
  [graphqlMod, '#[derive(MergedObject, Default)]', 'baseline GraphQL objects are not merged into Pages schema'],
  [graphqlBaseline, 'page_builder_scenario_baseline', 'baseline GraphQL query is missing'],
  [graphqlBaseline, 'page_builder_scenario_release_status', 'server release status query is missing'],
  [graphqlBaseline, 'save_page_builder_scenario_baseline', 'baseline GraphQL save mutation is missing'],
  [graphqlBaseline, 'delete_page_builder_scenario_baseline', 'baseline GraphQL delete mutation is missing'],
  [graphqlBaseline, 'expected_baseline_hash', 'baseline GraphQL mutations do not accept an expected hash'],
  [graphqlBaseline, '.save_if_current(', 'baseline GraphQL save bypasses compare-and-swap'],
  [graphqlBaseline, '.delete_if_current(', 'baseline GraphQL delete bypasses compare-and-swap'],
  [graphqlBaseline, 'visual_changes', 'server release status does not expose visual changes'],
  [graphqlBaseline, 'breaking_changes', 'server release status does not expose breaking changes'],
  [mutation, '.ensure_publish_allowed(tenant_id, id)', 'publishPage does not enforce scenario regression gate'],
  [mutation, '.ensure_published_candidate_allowed(tenant_id, id, project_data)', 'updatePage does not gate candidate live builder content'],
  [adminModel, 'pub struct PageBuilderScenarioReleaseStatus', 'Pages admin release status model is missing'],
  [adminAdapter, 'PAGE_BUILDER_SCENARIO_BASELINE_QUERY', 'Pages admin baseline query is missing'],
  [adminCasAdapter, 'expectedBaselineHash', 'Pages admin CAS mutation does not send the expected hash'],
  [adminCasAdapter, 'scenario baseline', 'Pages admin CAS adapter is missing'],
  [adminStatusAdapter, 'PAGE_BUILDER_SCENARIO_RELEASE_STATUS_QUERY', 'Pages admin server release status query is missing'],
  [adminTransport, 'scenario_baseline_cas_adapter::save', 'Pages admin transport does not use CAS save'],
  [adminTransport, 'scenario_baseline_cas_adapter::delete', 'Pages admin transport does not use CAS delete'],
  [adminTransport, 'fetch_page_builder_scenario_release_status', 'Pages admin transport does not expose release status'],
  [composition, 'with_runtime_scenarios(scenarios)', 'Pages builder host does not provide preview scenarios'],
  [composition, 'with_runtime_scenario_baseline', 'Pages builder host does not load persisted baseline'],
  [composition, 'on_runtime_scenario_baseline', 'Pages builder host does not persist baseline changes'],
  [composition, 'server_status.get_untracked().baseline_hash.clone()', 'Pages builder does not use the server-confirmed expected hash'],
  [composition, 'ServerReleaseStatus', 'Pages builder does not display server release status'],
  [composition, 'Baseline was written but server status could not be verified', 'baseline persistence is not confirmed by server evaluation'],
  [releaseCore, 'FLY_RUNTIME_SCENARIO_RELEASE_BASELINE', 'Fly release baseline format is missing'],
  [releaseApi, 'SCENARIO_REGRESSION_BLOCKED', 'stable release rejection code is missing'],
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

const gateIndex = mutation.indexOf('.ensure_publish_allowed(tenant_id, id)');
const publishIndex = mutation.indexOf('.publish(tenant_id, page_security(&auth), id)');
if (gateIndex < 0 || publishIndex < 0 || gateIndex > publishIndex) {
  failures.push('publishPage must evaluate the scenario baseline before publishing the page');
}

const candidateGateIndex = mutation.indexOf('.ensure_published_candidate_allowed(tenant_id, id, project_data)');
const updateIndex = mutation.indexOf('.update(');
if (candidateGateIndex < 0 || updateIndex < 0 || candidateGateIndex > updateIndex) {
  failures.push('published updatePage candidate must be evaluated before the page body is written');
}

if (service.includes('project_data.get("nodes")')) {
  failures.push('scenario release service must use canonical GrapesJS project data');
}
if (composition.includes('body_content_json: baseline')) {
  failures.push('scenario baseline must remain separate from Pages body project_data');
}
if (adminTransport.includes('graphql_adapter::save_page_builder_scenario_baseline(')) {
  failures.push('Pages admin transport must not use the current unconditional baseline save');
}
if (adminTransport.includes('graphql_adapter::delete_page_builder_scenario_baseline(')) {
  failures.push('Pages admin transport must not use the current unconditional baseline delete');
}

if (failures.length > 0) {
  console.error('Pages Page Builder scenario baseline verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Pages Page Builder scenario baseline wiring verified.');
