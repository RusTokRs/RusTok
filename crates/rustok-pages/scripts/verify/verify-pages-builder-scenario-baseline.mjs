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
  lifecycle,
  reviewedPublish,
  document,
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
  read('crates/rustok-pages/src/services/page/lifecycle.rs'),
  read('crates/rustok-pages/src/services/page/reviewed_publish.rs'),
  read('crates/rustok-pages/src/services/page/document.rs'),
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
  [service, 'PAGE_BUILDER_SCENARIO_BASELINE_CONFLICT_ERROR_CODE', 'stable baseline conflict code is missing'],
  [service, 'save_if_current', 'baseline compare-and-swap save is missing'],
  [service, 'delete_if_current', 'baseline compare-and-swap delete is missing'],
  [reviewedPublish, 'ensure_candidates_allowed_in_tx', 'reviewed publish does not evaluate promoted scenario candidates'],
  [reviewedPublish, 'compile_builder_sources_with_reviewed_runtime', 'reviewed publish does not compile the exact reviewed runtime'],
  [reviewedPublish, 'bind_existing_body_in_tx', 'reviewed publish does not bind the compiled artifact atomically'],
  [reviewedPublish, 'insert_publish_operation_in_tx', 'reviewed publish does not persist the durable receipt'],
  [lifecycle, 'publish_non_builder_if_current', 'non-builder lifecycle command is missing'],
  [lifecycle, 'PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED', 'non-builder lifecycle does not reject Page Builder documents with a stable code'],
  [document, 'PAGE_DOCUMENT_REVISION_CONFLICT', 'document save has no independent revision conflict'],
  [document, 'page_active.updated_at', 'document save does not record draft activity'],
  [graphqlMod, '#[derive(MergedObject, Default)]', 'baseline GraphQL objects are not merged into Pages schema'],
  [graphqlBaseline, 'page_builder_scenario_baseline', 'scenario baseline GraphQL query is missing'],
  [graphqlBaseline, 'page_builder_scenario_release_status', 'server release status query is missing'],
  [graphqlBaseline, 'save_page_builder_scenario_baseline', 'baseline GraphQL save mutation is missing'],
  [graphqlBaseline, 'delete_page_builder_scenario_baseline', 'baseline GraphQL delete mutation is missing'],
  [graphqlBaseline, 'expected_baseline_hash', 'baseline GraphQL mutations do not accept an expected hash'],
  [mutation, 'save_page_document', 'savePageDocument mutation is missing'],
  [mutation, 'publish_reviewed', 'publishPage does not use the reviewed atomic command'],
  [adminModel, 'pub struct PageBuilderScenarioReleaseStatus', 'Pages admin release status model is missing'],
  [adminAdapter, 'SAVE_PAGE_DOCUMENT_MUTATION', 'Pages admin does not use the document-only mutation'],
  [adminAdapter, 'resolve_publish_scenario(&baseline, selected_scenario_id.as_deref())', 'Pages admin does not resolve the explicit selected promoted scenario'],
  [adminCasAdapter, 'expectedBaselineHash', 'Pages admin CAS mutation does not send the expected hash'],
  [adminStatusAdapter, 'PAGE_BUILDER_SCENARIO_RELEASE_STATUS_QUERY', 'Pages admin server release status query is missing'],
  [adminTransport, 'scenario_baseline_cas_adapter::save', 'Pages admin transport does not use CAS save'],
  [adminTransport, 'fetch_page_builder_scenario_release_status', 'Pages admin transport does not expose release status'],
  [composition, 'with_runtime_scenarios(scenarios)', 'Pages builder host does not provide preview scenarios'],
  [composition, 'with_runtime_scenario_baseline', 'Pages builder host does not load persisted baseline'],
  [composition, 'on_runtime_scenario_baseline', 'Pages builder host does not persist baseline changes'],
  [releaseCore, 'FLY_RUNTIME_SCENARIO_RELEASE_BASELINE', 'Fly release baseline format is missing'],
  [releaseApi, 'SCENARIO_REGRESSION_BLOCKED', 'stable release rejection code is missing'],
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

const candidateGate = reviewedPublish.indexOf('ensure_candidates_allowed_in_tx');
const reviewedCompile = reviewedPublish.indexOf('compile_builder_sources_with_reviewed_runtime');
const artifactBind = reviewedPublish.indexOf('bind_existing_body_in_tx');
const transition = reviewedPublish.indexOf('PageTransition::Publish');
const receipt = reviewedPublish.indexOf('insert_publish_operation_in_tx');
if (
  candidateGate < 0 ||
  reviewedCompile < 0 ||
  artifactBind < 0 ||
  transition < 0 ||
  receipt < 0 ||
  candidateGate > reviewedCompile ||
  reviewedCompile > artifactBind ||
  artifactBind > transition ||
  transition > receipt
) {
  failures.push('reviewed publish must gate, compile, bind, transition and persist receipt in order');
}
if (
  lifecycle.includes('pub async fn publish(') ||
  lifecycle.includes('pub async fn publish_if_current(') ||
  lifecycle.includes('compile_builder_sources') ||
  lifecycle.includes('bind_existing_body_in_tx')
) {
  failures.push('non-builder lifecycle must not expose or implement legacy Page Builder publication');
}
if (document.includes('bind_existing_body_in_tx') || document.includes('PageTransition::Publish')) {
  failures.push('document save must not publish or replace the published artifact binding');
}
if (mutation.includes('update_page') || adminAdapter.includes('UPDATE_PAGE_MUTATION')) {
  failures.push('universal updatePage must not return as a release path');
}
if (service.includes('project_data.get("nodes")')) {
  failures.push('scenario release service must use canonical GrapesJS project data');
}

if (failures.length > 0) {
  console.error('Pages Page Builder scenario baseline verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Pages Page Builder scenario baseline wiring verified');
