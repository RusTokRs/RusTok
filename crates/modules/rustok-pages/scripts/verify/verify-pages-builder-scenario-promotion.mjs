import { readFile } from 'node:fs/promises';
import process from 'node:process';

const root = process.cwd();
const read = (path) => readFile(`${root}/${path}`, 'utf8');

const [
  migrationMod,
  migration,
  entity,
  service,
  graphql,
  pageBuilderRelease,
  adminPanel,
  adminHost,
  adminCanvas,
  adminModel,
  casAdapter,
  statusAdapter,
  composition,
] = await Promise.all([
  read('crates/rustok-pages/src/migrations/mod.rs'),
  read('crates/rustok-pages/src/migrations/m20260714_000002_add_scenario_baseline_promotion_metadata.rs'),
  read('crates/rustok-pages/src/entities/page_builder_scenario_baseline.rs'),
  read('crates/rustok-pages/src/services/scenario_baseline.rs'),
  read('crates/rustok-pages/src/graphql/scenario_baseline.rs'),
  read('crates/rustok-page-builder/src/runtime_scenario_release.rs'),
  read('crates/rustok-page-builder/admin/src/editor/runtime_scenario_regression.rs'),
  read('crates/rustok-page-builder/admin/src/ui/leptos.rs'),
  read('crates/rustok-page-builder/admin/src/editor/modular_canvas.rs'),
  read('crates/rustok-pages/admin/src/model.rs'),
  read('crates/rustok-pages/admin/src/transport/scenario_baseline_cas_adapter.rs'),
  read('crates/rustok-pages/admin/src/transport/scenario_release_adapter.rs'),
  read('crates/rustok-pages/admin/src/composition.rs'),
]);

const required = [
  [migrationMod, 'm20260714_000002_add_scenario_baseline_promotion_metadata', 'promotion metadata migration is not registered'],
  [migration, 'PreviousBaselineHash', 'previous baseline hash migration column is missing'],
  [migration, 'PromotedBy', 'promoted actor migration column is missing'],
  [migration, 'PromotionNote', 'promotion note migration column is missing'],
  [migration, 'PromotedAt', 'promotion timestamp migration column is missing'],
  [entity, 'pub previous_baseline_hash: Option<String>', 'baseline entity previous hash is missing'],
  [entity, 'pub promoted_by: Option<Uuid>', 'baseline entity actor is missing'],
  [entity, 'pub promotion_note: Option<String>', 'baseline entity review note is missing'],
  [entity, 'pub promoted_at: Option<DateTimeWithTimeZone>', 'baseline entity promotion timestamp is missing'],
  [service, 'SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED', 'stable promotion-note error code is missing'],
  [service, 'existing.is_some() && promotion_note.is_none()', 'replacement does not require a review note'],
  [service, 'Column::PreviousBaselineHash', 'promotion does not persist the previous hash'],
  [service, 'Column::PromotedBy', 'promotion does not persist the actor'],
  [service, 'Column::PromotedAt', 'promotion does not persist the timestamp'],
  [service, 'pub struct PageBuilderScenarioBaselineRecord', 'typed promotion record is missing'],
  [graphql, 'pub promotion_note: Option<String>', 'GraphQL promotion note input/output is missing'],
  [graphql, 'auth.user_id', 'GraphQL promotion does not bind the authenticated actor'],
  [graphql, 'input.promotion_note.as_deref()', 'GraphQL promotion note is not passed to the service'],
  [graphql, 'previous_baseline_hash', 'GraphQL status does not expose previous baseline hash'],
  [graphql, 'promoted_at', 'GraphQL status does not expose promotion timestamp'],
  [pageBuilderRelease, 'pub struct PageBuilderScenarioBaselineChange', 'typed host baseline change is missing'],
  [pageBuilderRelease, 'pub promotion_note: Option<String>', 'typed host change does not carry review notes'],
  [adminPanel, 'scenarioBaselinePromotionNote', 'admin promotion note input is missing'],
  [adminPanel, 'A review note is required before replacing', 'admin does not block note-less replacement'],
  [adminPanel, 'PageBuilderScenarioBaselineChange::save', 'admin does not emit typed baseline changes'],
  [adminHost, 'Callback<PageBuilderScenarioBaselineChange>', 'admin host callback is not typed'],
  [adminCanvas, 'Callback<PageBuilderScenarioBaselineChange>', 'admin canvas callback is not typed'],
  [adminModel, 'pub promotion_note: Option<String>', 'Pages admin status model lacks promotion note'],
  [casAdapter, 'promotionNote', 'Pages admin CAS mutation does not send promotion note'],
  [statusAdapter, 'previousBaselineHash promotedBy promotionNote promotedAt', 'Pages admin status query omits promotion metadata'],
  [composition, 'PageBuilderScenarioBaselineChange', 'Pages composition does not consume typed baseline changes'],
  [composition, 'change.promotion_note', 'Pages composition drops the review note'],
  [composition, 'server_status.get_untracked().baseline_hash.clone()', 'Pages composition does not use server-confirmed CAS state'],
];

const failures = required
  .filter(([source, marker]) => !source.includes(marker))
  .map(([, , message]) => message);

if (service.includes('promotion_note.unwrap_or_default()')) {
  failures.push('service must not silently synthesize a promotion review note');
}
if (composition.includes('promotion_note: Some("')) {
  failures.push('consumer composition must not hard-code operator review notes');
}
if (casAdapter.indexOf('expected_baseline_hash') > casAdapter.indexOf('promotion_note')) {
  // Both values must be present in the same mutation input; ordering is not semantically important.
}

if (failures.length > 0) {
  console.error('Pages scenario baseline promotion verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Pages scenario baseline promotion wiring verified.');
