import { readFile } from 'node:fs/promises';
import process from 'node:process';

const root = process.cwd();
const read = (path) => readFile(`${root}/${path}`, 'utf8');

const [
  flyLib,
  validation,
  contextSchema,
  contextContract,
  contextCompatibility,
  contextDependency,
  contextJsonSchema,
  contextMigration,
  runtimeGate,
  runtimePipeline,
  scenarioRender,
  scenarioSnapshot,
  pageBuilderLib,
  runtimeContextApi,
  compatibilityApi,
  dependencyApi,
  migrationApi,
  scenarioRenderApi,
  scenarioSnapshotApi,
  editorMod,
  modularCanvas,
  contextPanel,
  scenarioMatrixPanel,
  scenarioRegressionPanel,
] = await Promise.all([
  read('crates/fly/src/lib.rs'),
  read('crates/fly/src/validation.rs'),
  read('crates/fly/src/context_schema.rs'),
  read('crates/fly/src/context_contract.rs'),
  read('crates/fly/src/context_compatibility.rs'),
  read('crates/fly/src/context_dependency.rs'),
  read('crates/fly/src/context_json_schema.rs'),
  read('crates/fly/src/context_migration.rs'),
  read('crates/fly/src/runtime_gate.rs'),
  read('crates/fly/src/runtime_pipeline.rs'),
  read('crates/fly/src/runtime_scenario_render.rs'),
  read('crates/fly/src/runtime_scenario_snapshot.rs'),
  read('crates/rustok-page-builder/src/lib.rs'),
  read('crates/rustok-page-builder/src/runtime_context.rs'),
  read('crates/rustok-page-builder/src/runtime_context_compatibility.rs'),
  read('crates/rustok-page-builder/src/runtime_context_dependency.rs'),
  read('crates/rustok-page-builder/src/runtime_context_migration.rs'),
  read('crates/rustok-page-builder/src/runtime_scenario_render.rs'),
  read('crates/rustok-page-builder/src/runtime_scenario_snapshot.rs'),
  read('crates/rustok-page-builder/admin/src/editor/mod.rs'),
  read('crates/rustok-page-builder/admin/src/editor/modular_canvas.rs'),
  read('crates/rustok-page-builder/admin/src/editor/context_schema_panel.rs'),
  read('crates/rustok-page-builder/admin/src/editor/runtime_scenario_matrix.rs'),
  read('crates/rustok-page-builder/admin/src/editor/runtime_scenario_regression.rs'),
]);

const required = [
  [flyLib, 'mod context_schema;', 'Fly context schema module is not registered'],
  [flyLib, 'pub use context_schema::*;', 'Fly context schema API is not exported'],
  [flyLib, 'mod runtime_scenario_snapshot;', 'scenario snapshot module is not registered'],
  [flyLib, 'pub use runtime_scenario_snapshot::*;', 'scenario snapshot API is not exported'],
  [validation, 'validate_runtime_extensions(document)', 'canonical project validation must include runtime extensions'],
  [contextSchema, 'pub enum ContextExpression', 'safe computed expression AST is missing'],
  [contextSchema, 'pub fn materialize_context', 'context defaults/computed materialization is missing'],
  [contextContract, 'pub fn preflight_runtime_context', 'runtime context preflight is missing'],
  [contextCompatibility, 'pub fn diff_runtime_context_contracts', 'runtime contract diff is missing'],
  [contextDependency, 'pub fn analyze_runtime_context_dependencies', 'runtime dependency graph is missing'],
  [contextJsonSchema, 'pub fn export_runtime_context_json_schema', 'runtime JSON Schema export is missing'],
  [contextMigration, 'pub fn migrate_runtime_context', 'runtime context migration is missing'],
  [runtimeGate, 'pub fn evaluate_runtime_publish_gate', 'runtime publish gate is missing'],
  [runtimePipeline, 'pub fn materialize_project_with_runtime_context', 'effective context pipeline is missing'],
  [scenarioRender, 'pub fn render_runtime_scenario_matrix', 'scenario render matrix is missing'],
  [scenarioRender, 'duplicate_html_groups', 'scenario duplicate-output detection is missing'],
  [scenarioSnapshot, 'FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1', 'scenario snapshot format marker is missing'],
  [scenarioSnapshot, 'pub fn diff_runtime_scenario_render_snapshots', 'scenario regression diff is missing'],
  [pageBuilderLib, 'pub mod runtime_context;', 'consumer runtime context API is not exported'],
  [pageBuilderLib, 'pub mod runtime_scenario_render;', 'consumer scenario render API is not exported'],
  [pageBuilderLib, 'pub mod runtime_scenario_snapshot;', 'consumer scenario snapshot API is not exported'],
  [runtimeContextApi, 'PageBuilderRuntimeContextInspector', 'consumer context inspector is missing'],
  [compatibilityApi, 'PageBuilderRuntimeContractCompatibilityInspector', 'consumer compatibility inspector is missing'],
  [dependencyApi, 'PageBuilderRuntimeDependencyInspector', 'consumer dependency inspector is missing'],
  [migrationApi, 'PageBuilderRuntimeContextMigrator', 'consumer context migrator is missing'],
  [scenarioRenderApi, 'PageBuilderRuntimeScenarioRenderer', 'consumer scenario renderer is missing'],
  [scenarioSnapshotApi, 'PageBuilderRuntimeScenarioRegressionInspector', 'consumer scenario regression inspector is missing'],
  [editorMod, 'RuntimeScenarioMatrixPanel', 'scenario matrix panel is not registered'],
  [editorMod, 'RuntimeScenarioRegressionPanel', 'scenario regression panel is not registered'],
  [modularCanvas, '<RuntimeScenarioMatrixPanel', 'scenario matrix panel is not mounted'],
  [modularCanvas, '<RuntimeScenarioRegressionPanel', 'scenario regression panel is not mounted'],
  [contextPanel, 'EditorCommand::Context', 'context authoring does not use editor transactions'],
  [scenarioMatrixPanel, 'render_runtime_scenario_matrix', 'admin scenario matrix does not use Fly renderer'],
  [scenarioRegressionPanel, 'RuntimeScenarioReleaseBaseline::capture', 'admin regression panel does not capture canonical release baselines'],
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

if (contextSchema.includes('eval(') || contextSchema.includes('Function(')) {
  failures.push('computed expressions must not use JavaScript eval or Function');
}
const contextStage = runtimePipeline.indexOf('materialize_context');
const bindingStage = runtimePipeline.indexOf('materialize_bindings');
const dynamicStage = runtimePipeline.indexOf('materialize_runtime');
if (contextStage < 0 || bindingStage < 0 || dynamicStage < 0) {
  failures.push('runtime pipeline stages are incomplete');
} else {
  if (contextStage > bindingStage) {
    failures.push('context defaults/computed values must run before bindings');
  }
  if (bindingStage > dynamicStage) {
    failures.push('bindings must run before conditions and repeaters');
  }
}

if (failures.length > 0) {
  console.error('Page Builder runtime context verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Page Builder runtime context wiring verified.');
