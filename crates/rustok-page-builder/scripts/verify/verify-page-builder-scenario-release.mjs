import { readFile } from 'node:fs/promises';
import process from 'node:process';

const root = process.cwd();
const read = (path) => readFile(`${root}/${path}`, 'utf8');

const [
  flyLib,
  releaseCore,
  pageBuilderLib,
  releaseApi,
  flyService,
  adminHost,
  adminCanvas,
  regressionPanel,
  serviceTests,
] = await Promise.all([
  read('crates/fly/src/lib.rs'),
  read('crates/fly/src/runtime_scenario_release.rs'),
  read('crates/rustok-page-builder/src/lib.rs'),
  read('crates/rustok-page-builder/src/runtime_scenario_release.rs'),
  read('crates/rustok-page-builder/src/adapters/fly_service.rs'),
  read('crates/rustok-page-builder/admin/src/ui/leptos.rs'),
  read('crates/rustok-page-builder/admin/src/editor/modular_canvas.rs'),
  read('crates/rustok-page-builder/admin/src/editor/runtime_scenario_regression.rs'),
  read('crates/rustok-page-builder/tests/runtime_scenario_release.rs'),
]);

const required = [
  [flyLib, 'mod runtime_scenario_release;', 'Fly release module is not registered'],
  [flyLib, 'pub use runtime_scenario_release::*;', 'Fly release API is not exported'],
  [releaseCore, 'FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1', 'release baseline format marker is missing'],
  [releaseCore, 'pub fn evaluate_runtime_scenario_release', 'release evaluator is missing'],
  [releaseCore, 'snapshot_has_valid_hash', 'snapshot integrity validation is missing'],
  [releaseCore, 'RuntimeScenarioReleaseMode::BlockBroken', 'broken regression policy is missing'],
  [pageBuilderLib, 'pub mod runtime_scenario_release;', 'consumer release API is not exported'],
  [releaseApi, 'pub trait PageBuilderScenarioBaselineStore', 'baseline persistence port is missing'],
  [releaseApi, 'SCENARIO_REGRESSION_BLOCKED', 'stable release error code is missing'],
  [flyService, 'with_scenario_release_gate', 'Fly service release gate builder is missing'],
  [flyService, '.load_scenario_baseline(context, &input.page_id)', 'publish does not load the persisted baseline'],
  [flyService, 'if !evaluation.allowed', 'publish does not reject failed release evaluation'],
  [adminHost, 'with_runtime_scenario_baseline', 'admin host cannot receive a persisted baseline'],
  [adminHost, 'on_runtime_scenario_baseline', 'admin host cannot persist baseline changes'],
  [adminCanvas, 'initial_baseline=runtime_scenario_baseline', 'admin baseline is not mounted'],
  [regressionPanel, 'RuntimeScenarioReleaseBaseline::capture', 'admin does not capture release baselines'],
  [regressionPanel, 'callback.run(Some(release_baseline))', 'admin does not emit baseline persistence changes'],
  [serviceTests, 'broken_regression_blocks_before_project_write', 'blocked-write regression test is missing'],
  [serviceTests, 'assert_eq!(writes.load(Ordering::SeqCst), 0)', 'blocked publish does not assert zero writes'],
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

const gateIndex = flyService.indexOf('evaluate_runtime_scenario_release');
const saveIndex = flyService.indexOf('.save_project(');
if (gateIndex < 0 || saveIndex < 0 || gateIndex > saveIndex) {
  failures.push('scenario release gate must run before project persistence');
}

if (releaseCore.includes('eval(') || releaseCore.includes('Function(')) {
  failures.push('scenario release evaluation must not execute scripts');
}

if (failures.length > 0) {
  console.error('Page Builder scenario release verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Page Builder scenario release wiring verified.');
