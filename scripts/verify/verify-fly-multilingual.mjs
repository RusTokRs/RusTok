import { readFile } from 'node:fs/promises';

const paths = {
  runtimeLocale: 'crates/fly/src/runtime_locale.rs',
  translations: 'crates/fly/src/translation.rs',
  runtimePipeline: 'crates/fly/src/runtime_pipeline.rs',
  runtimeValidation: 'crates/fly/src/runtime_validation.rs',
  pageBuilderLocale: 'crates/rustok-page-builder/src/locale.rs',
  pageBuilderRender: 'crates/rustok-page-builder/src/render.rs',
  pagesIntent: 'crates/rustok-pages/admin/src/browser_intent.rs',
  ssrLocale: 'crates/rustok-page-builder/admin/src/editor/ssr_locale.rs',
  ssrInspector: 'crates/rustok-page-builder/admin/src/editor/ssr_inspector.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
};

const source = Object.fromEntries(
  await Promise.all(
    Object.entries(paths).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
  ),
);

const failures = [];
const requireMarker = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const requireMarkers = (key, markers, label) => {
  for (const marker of markers) requireMarker(key, marker, `${label} is missing ${marker}`);
};
const flattenKeys = (value, prefix = '') => Object.entries(value).flatMap(([key, nested]) => {
  const path = prefix ? `${prefix}.${key}` : key;
  return nested && typeof nested === 'object' && !Array.isArray(nested)
    ? flattenKeys(nested, path)
    : [path];
}).sort();
const localeValue = (locale, path) => path
  .split('.')
  .reduce((value, segment) => value && typeof value === 'object' ? value[segment] : undefined, locale);

requireMarkers('runtimeLocale', [
  'pub const RUNTIME_LOCALE_FIELD',
  'pub const RUNTIME_FALLBACK_LOCALES_FIELD',
  'pub const LOCALIZED_VALUES_FIELD',
  'pub fn materialize_runtime_locale_context',
  'pub fn normalize_locale_tag',
  'runtime_localized_value_fallback',
  'runtime_localized_value_unresolved',
  'regional_locale_falls_back_to_language',
  'unresolved_localized_value_is_preserved_losslessly',
], 'Fly runtime locale resolver');
requireMarkers('translations', [
  'pub const FLY_TRANSLATIONS_FIELD',
  'pub const RUNTIME_TRANSLATIONS_CONTEXT_FIELD',
  'pub struct TranslationEntry',
  'pub enum TranslationCommand',
  'pub struct TranslationCatalog',
  'pub fn apply_translation_command',
  'pub fn materialize_project_translations',
  'pub fn validate_translation_definitions',
  'unknown_entries',
  'catalog_materializes_into_binding_context',
], 'Fly project translation catalog');
requireMarkers('runtimePipeline', [
  'materialize_project_translations(document, input_context)',
  'materialize_runtime_locale_context(&translation_context)',
  'materialize_context(document, &localized_input_context)',
  'materialize_bindings(document, &effective_context)',
  'materialize_runtime(&document, &effective_context)',
  'project_translation_catalog_materializes_before_bindings',
], 'multilingual Fly runtime ordering');
requireMarkers('runtimeValidation', [
  'validate_translation_definitions(document)',
  'translation_locale_invalid',
  'duplicate_translation_id',
], 'translation publish validation');
requireMarkers('pageBuilderLocale', [
  'pub struct PageBuilderLocaleContext',
  'pub fn from_request',
  'pub fn parse_accept_language',
  'RUNTIME_LOCALE_FIELD',
  'RUNTIME_FALLBACK_LOCALES_FIELD',
  'accept_language_is_sorted_by_quality_and_stable_order',
], 'SSR locale negotiation API');
requireMarkers('pageBuilderRender', [
  'pub fn with_locale',
  'render_localized_runtime_document_html',
  'localized_runtime_render_uses_project_translation_catalog',
], 'localized runtime render API');
requireMarkers('pagesIntent', [
  'set_runtime_locale',
  'runtime_locale_from_payload',
  'normalize_locale_tag',
  'RUNTIME_LOCALE_FIELD',
  'RUNTIME_FALLBACK_LOCALES_FIELD',
], 'Pages SSR locale draft intent');
requireMarkers('ssrLocale', [
  'data-fly-ssr-locale="true"',
  'data-fly-intent-form="set_runtime_locale"',
  'page_builder.ssrInspector.localeTitle',
  'page_builder.ssrInspector.fallbackLocalesLabel',
], 'localized SSR locale panel');
requireMarkers('ssrInspector', [
  'crate::i18n::t',
  'UiRouteContext',
  'page_builder.ssrInspector.title',
  'page_builder.ssrInspector.pageLifecycle',
], 'localized SSR inspector');

const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder en/ru locale key parity failed');
}
const requiredKeys = [
  'page_builder.ssrInspector.title',
  'page_builder.ssrInspector.description',
  'page_builder.ssrInspector.runtimeContext',
  'page_builder.ssrInspector.runtimeContextAria',
  'page_builder.ssrInspector.runtimeContextHelp',
  'page_builder.ssrInspector.applyRuntimeContext',
  'page_builder.ssrInspector.localeTitle',
  'page_builder.ssrInspector.localeHelp',
  'page_builder.ssrInspector.localeLabel',
  'page_builder.ssrInspector.localePlaceholder',
  'page_builder.ssrInspector.fallbackLocalesLabel',
  'page_builder.ssrInspector.fallbackLocalesPlaceholder',
  'page_builder.ssrInspector.applyLocale',
  'page_builder.ssrInspector.canvasComponent',
  'page_builder.ssrInspector.componentProperty',
  'page_builder.ssrInspector.pageMetadata',
  'page_builder.ssrInspector.pageLifecycle',
];
for (const [localeName, locale] of [['en', en], ['ru', ru]]) {
  for (const key of requiredKeys) {
    const value = localeValue(locale, key);
    if (typeof value !== 'string' || value.trim() === '') {
      failures.push(`Page Builder ${localeName} locale is missing non-empty ${key}`);
    }
  }
}

if (failures.length > 0) {
  console.error('Fly multilingual verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly multilingual runtime verified.');
