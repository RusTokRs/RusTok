import { readFile } from 'node:fs/promises';

const paths = {
  flyLib: 'crates/fly/src/lib.rs',
  componentVisit: 'crates/fly/src/component_visit.rs',
  safeUrl: 'crates/fly/src/safe_url.rs',
  actionFacade: 'crates/fly/src/action.rs',
  actionModel: 'crates/fly/src/action/model.rs',
  actionValidation: 'crates/fly/src/action/validation.rs',
  actionMaterialize: 'crates/fly/src/action/materialize.rs',
  actionTests: 'crates/fly/src/action/tests.rs',
  runtimePipeline: 'crates/fly/src/runtime_pipeline.rs',
  browserContract: 'crates/fly-browser/src/lib.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  browserAdapter: 'crates/rustok-page-builder/admin/src/ui/browser_adapter.rs',
  adminLib: 'crates/rustok-page-builder/admin/src/lib.rs',
  editorMod: 'crates/rustok-page-builder/admin/src/editor/mod.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  ssrForms: 'crates/rustok-page-builder/admin/src/editor/ssr_forms.rs',
  ssrEditor: 'crates/rustok-page-builder/admin/src/editor/ssr_actions_forms.rs',
  browserTests: 'crates/rustok-page-builder/admin/src/ssr_actions_forms_browser_tests.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
  workflow: '.github/workflows/fly-page-builder.yml',
};

const source = Object.fromEntries(await Promise.all(
  Object.entries(paths).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
));
const failures = [];

const requireMarker = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const rejectMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};
const requireMarkers = (key, markers, label) => {
  for (const marker of markers) requireMarker(key, marker, `${label} is missing ${marker}`);
};
const requireOrder = (key, fragments, label) => {
  let cursor = -1;
  for (const fragment of fragments) {
    const index = source[key].indexOf(fragment, cursor + 1);
    if (index < 0) {
      failures.push(`${label} is missing ${fragment}`);
      return;
    }
    if (index <= cursor) {
      failures.push(`${label} does not preserve required ordering`);
      return;
    }
    cursor = index;
  }
};
const localeValue = (locale, path) => path
  .split('.')
  .reduce((value, segment) => value && typeof value === 'object' ? value[segment] : undefined, locale);
const flattenKeys = (value, prefix = '') => Object.entries(value).flatMap(([key, nested]) => {
  const path = prefix ? `${prefix}.${key}` : key;
  return nested && typeof nested === 'object' && !Array.isArray(nested)
    ? flattenKeys(nested, path)
    : [path];
}).sort();

requireMarkers('flyLib', [
  'mod component_visit;',
  'mod safe_url;',
  'pub use component_visit::{visit_project_components, ComponentVisit};',
], 'Fly interaction infrastructure');
requireMarkers('componentVisit', [
  'pub struct ComponentVisit',
  'pub fn visit_project_components(',
  'pub(crate) fn visit_project_components_mut(',
  'Mutation stays crate-private',
  'immutable_and_mutable_walks_share_page_depth_and_path_contract',
], 'read-only public component visitor');
requireMarkers('safeUrl', [
  'pub(crate) fn validate_safe_url',
  'pub(crate) fn normalize_safe_url',
  'rejects_network_paths_backslashes_controls_and_unsafe_schemes',
  'rejects_absolute_urls_without_authority_or_scheme_targets',
], 'shared safe URL boundary');
requireMarkers('actionFacade', [
  'mod materialize;',
  'mod model;',
  'mod validation;',
  'pub use materialize::*;',
  'pub use model::*;',
  'pub use validation::*;',
  '#[cfg(test)]\nmod tests;',
], 'action domain facade');
for (const forbidden of [
  'pub enum ComponentAction',
  'pub struct ComponentForm',
  'fn materialize_node(',
  'fn validate_node(',
]) {
  rejectMarker(
    'actionFacade',
    forbidden,
    `action facade must not own implementation marker ${forbidden}`,
  );
}
requireMarkers('actionModel', [
  'pub const FLY_ACTION_FIELD',
  'pub const FLY_FORM_FIELD',
  'pub enum ComponentAction',
  'pub struct ComponentForm',
  'pub struct ActionMaterialization',
  'GENERATED_INTERACTION_ATTRIBUTES',
], 'action and form models');
requireMarkers('actionValidation', [
  'pub fn validate_component_actions',
  'component_visit::visit_project_components',
  'safe_url::validate_safe_url as validate_shared_safe_url',
  'validate_shared_safe_url(value, label)',
  'component_form_interaction_contract_conflict',
  'non-default form encoding requires post method',
  'visit_project_components(&document.project',
], 'action and form validation');
requireMarkers('actionMaterialize', [
  'pub fn materialize_component_actions',
  'component_visit::visit_project_components_mut',
  'visit_project_components_mut(&mut materialized.project',
  'clear_interaction_materialization',
  'ActionResolution',
], 'action and form materialization');
for (const [key, forbidden] of [
  ['actionValidation', 'root.visit(0, "page.component"'],
  ['actionValidation', 'fn validate_node('],
  ['actionMaterialize', 'fn materialize_node('],
  ['actionMaterialize', '#[allow(clippy::too_many_arguments)]'],
]) {
  rejectMarker(key, forbidden, `${key} must use the shared component visitor instead of ${forbidden}`);
}
requireMarkers('actionTests', [
  'actions_and_forms_materialize_to_native_and_custom_contracts',
  'materialization_clears_stale_interaction_attributes',
  'network_paths_and_backslash_urls_are_blocking_validation',
  'duplicate_forms_and_interaction_conflicts_are_rejected',
  'non_post_encoding_is_rejected',
  'anonymous_action_diagnostics_use_the_shared_canonical_path',
], 'split action and form regression coverage');
requireOrder('runtimePipeline', [
  'validate_component_actions(&dynamic_document)',
  'materialize_component_actions(&linked_document, &effective_context)',
], 'runtime action/form validation and materialization');

const mutationIntents = [
  'set_component_action',
  'remove_component_action',
  'set_component_form',
  'remove_component_form',
  'set_native_form_field',
];
for (const intent of mutationIntents) {
  requireMarker('browserContract', `"${intent}"`, `fly-browser must classify ${intent} as mutating`);
  requireMarker('ssrForms', `"${intent}"`, `SSR form dispatcher must route ${intent}`);
  requireMarker('ssrEditor', `data-fly-intent-form="${intent}"`, `SSR editor must emit ${intent}`);
}
requireMarkers('browserContract', [
  'command_producing_and_draft_intents_are_mutating',
], 'fly-browser mutation regression coverage');
requireOrder('browserIntent', [
  'validate_revision(controller, &envelope)?',
  'dispatch_named_intent(controller, &envelope.intent, &envelope.payload)?',
], 'browser revision protection');
requireMarker('browserIntent', '.ssr_form_intent(other, payload)', 'browser dispatcher must delegate SSR forms');
requireMarkers('browserAdapter', [
  'input[type="number"][name]',
  'number.value !== ""',
], 'SSR form number normalization');

requireMarkers('editorMod', [
  'mod ssr_actions_forms;',
  'SsrActionsFormsPanel',
  'SsrComponentActionRequest',
  'SsrComponentFormRequest',
  'SsrNativeFormFieldRequest',
], 'SSR action/form editor module wiring');
requireMarkers('adminCanvas', [
  'SsrActionsFormsPanel',
  '<SsrActionsFormsPanel runtime=ssr_actions_forms_runtime />',
], 'SSR action/form editor canvas mount');
requireMarkers('ssrEditor', [
  'pub struct SsrComponentActionRequest',
  'pub struct SsrComponentFormRequest',
  'pub struct SsrNativeFormFieldRequest',
  'pub fn ssr_component_action_intent',
  'pub fn ssr_component_form_intent',
  'pub fn ssr_native_form_field_intent',
  'action_from_request(request.clone())',
  'form_from_request(request.clone(), extensions)',
  'preserved_action_extensions',
  'validate_component_actions(&candidate)',
  'validate_native_field_constraints',
  'action_editor_preserves_unknown_extensions',
  'form_editor_preserves_unknown_extensions',
  'native_field_editor_sets_and_clears_html_constraints',
  'native_field_editor_rejects_inapplicable_constraints',
  'numeric_field_editor_rejects_invalid_or_inverted_bounds',
], 'typed SSR action/form/field editor');
requireMarkers('adminLib', [
  '#[cfg(test)]\nmod ssr_actions_forms_browser_tests;',
], 'browser regression test registration');
requireMarkers('browserTests', [
  'browser_dispatches_action_form_and_native_field_contracts',
  'stale_action_form_mutation_is_rejected_before_dispatch',
], 'browser action/form/field regression coverage');

const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder en/ru locale key parity failed');
}
for (const [localeName, locale] of [['en', en], ['ru', ru]]) {
  for (const key of [
    'page_builder.actionsForms.title',
    'page_builder.actionsForms.empty',
    'page_builder.actionsForms.actionTitle',
    'page_builder.actionsForms.formTitle',
    'page_builder.actionsForms.fieldTitle',
    'page_builder.actionsForms.save',
    'page_builder.actionsForms.remove',
  ]) {
    const value = localeValue(locale, key);
    if (typeof value !== 'string' || value.trim() === '') {
      failures.push(`Page Builder ${localeName} locale is missing non-empty ${key}`);
    }
  }
}

requireMarkers('workflow', [
  'cargo fmt -p fly -p fly-browser -p rustok-page-builder-admin -- --check',
  'cargo test -p fly-browser --lib',
  'cargo clippy -p fly-browser -p rustok-page-builder-admin --lib -- -D warnings',
  'node scripts/verify/verify-fly-actions-forms.mjs',
], 'focused Fly workflow');

if (failures.length > 0) {
  console.error('Fly actions, forms, and fields verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly actions, forms, and fields verified.');