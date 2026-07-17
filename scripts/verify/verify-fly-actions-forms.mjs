import { readFile } from 'node:fs/promises';

const paths = {
  action: 'crates/fly/src/action.rs',
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

requireMarkers('action', [
  'pub const FLY_ACTION_FIELD',
  'pub const FLY_FORM_FIELD',
  'pub enum ComponentAction',
  'pub struct ComponentForm',
  'pub fn materialize_component_actions',
  'pub fn validate_component_actions',
], 'Fly action and form contract');
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
  'validate_component_actions(&candidate)',
  'form_editor_preserves_unknown_extensions',
  'native_field_editor_sets_and_clears_html_constraints',
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
