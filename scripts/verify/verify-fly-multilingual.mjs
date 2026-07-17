import { readFile } from 'node:fs/promises';

const paths = {
  runtimeLocale: 'crates/fly/src/runtime_locale.rs',
  localePolicy: 'crates/fly/src/locale_policy.rs',
  localeCoverage: 'crates/fly/src/locale_coverage.rs',
  localizedRoute: 'crates/fly/src/localized_route.rs',
  translations: 'crates/fly/src/translation.rs',
  commandEditor: 'crates/fly/src/command/editor.rs',
  commandTests: 'crates/fly/src/command/tests.rs',
  pageMetadataLocale: 'crates/fly/src/page_metadata_locale.rs',
  runtimePipeline: 'crates/fly/src/runtime_pipeline.rs',
  runtimeValidation: 'crates/fly/src/runtime_validation.rs',
  browserContract: 'crates/fly-browser/src/lib.rs',
  pageBuilderLocale: 'crates/rustok-page-builder/src/locale.rs',
  pageBuilderRender: 'crates/rustok-page-builder/src/render.rs',
  storefrontLocalizedRoute: 'crates/rustok-page-builder-storefront/src/localized_route.rs',
  pagesIntent: 'crates/rustok-pages/admin/src/browser_intent.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  ssrForms: 'crates/rustok-page-builder/admin/src/editor/ssr_forms.rs',
  ssrLocale: 'crates/rustok-page-builder/admin/src/editor/ssr_locale.rs',
  ssrLocalePolicy: 'crates/rustok-page-builder/admin/src/editor/ssr_locale_policy.rs',
  ssrLocaleCoverage: 'crates/rustok-page-builder/admin/src/editor/ssr_locale_coverage.rs',
  ssrTranslations: 'crates/rustok-page-builder/admin/src/editor/ssr_translations.rs',
  ssrInspector: 'crates/rustok-page-builder/admin/src/editor/ssr_inspector.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
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
requireMarkers('localePolicy', [
  'pub const FLY_LOCALES_FIELD',
  'pub struct ProjectLocalePolicy',
  'pub enforce_required_locales: bool',
  'pub fn set_project_locale_policy',
  'pub fn clear_project_locale_policy',
  'pub fn materialize_project_locale_context',
  'pub fn validate_project_locale_policy',
  'runtime_locale_invalid',
  'runtime_locale_unsupported',
  'translation_required_locale_missing',
  'localized_metadata_required_locale_missing',
  'legacy_locale_aliases_are_canonicalized',
  'invalid_runtime_locale_is_diagnosed_before_defaulting',
  'required_locale_coverage_is_warning_until_enforcement_is_enabled',
], 'Fly project locale policy');
requireMarkers('localeCoverage', [
  'pub enum LocaleCoverageKind',
  'pub struct LocaleCoverageGap',
  'pub struct LocaleCoverageSummary',
  'pub struct LocaleCoverageReport',
  'pub fn analyze_project_locale_coverage',
  'pub fn summary_for',
  'pub fn required_gaps',
  'coverage_reports_exact_translation_and_metadata_gaps',
  'coverage_discovers_optional_locales_without_policy',
  'invalid_policy_prevents_strict_readiness',
], 'Fly locale coverage report');
requireMarkers('localizedRoute', [
  'pub struct LocalizedPageRouteEntry',
  'pub struct LocalizedPageRouteResolution',
  'pub fn localized_page_route_index',
  'pub fn resolve_localized_page_route',
  'pub fn validate_localized_page_routes',
  'localized_slug_resolution_selects_page_and_render_locale',
  'unique_localized_slug_can_infer_locale',
  'duplicate_slug_for_same_locale_is_rejected_and_validated',
], 'Fly localized page route resolver');
requireMarkers('translations', [
  'pub const FLY_TRANSLATIONS_FIELD',
  'pub enum TranslationCommand',
  'SetLocalePolicy',
  'ClearLocalePolicy',
  'pub fn apply_translation_command',
  'pub fn materialize_project_translations',
  'pub fn validate_translation_definitions',
  'locale_policy_commands_share_translation_transaction_surface',
], 'Fly project translation catalog');
requireMarkers('commandEditor', [
  'EditorCommand::Translation',
  'apply_translation_command(document, command)',
], 'undoable translation command dispatch');
requireMarkers('commandTests', [
  'translation_commands_participate_in_history',
  'editor.undo().expect("undo translation command")',
  'editor.redo().expect("redo translation command")',
], 'translation command history coverage');
requireMarkers('pageMetadataLocale', [
  'pub struct LocalizedPageMetadataMaterialization',
  'pub fn materialize_localized_page_metadata',
  'localized_metadata_is_selected_without_mutating_source_document',
  'unresolved_metadata_wrapper_is_preserved_losslessly',
], 'localized page metadata runtime');
requireMarkers('runtimePipeline', [
  'materialize_project_locale_context(document, input_context)',
  'materialize_project_translations(document, &locale_policy_context)',
  'materialize_runtime_locale_context(&translation_context)',
  'materialize_localized_page_metadata(document, &localized_input_context)',
  'materialize_context(&localized_document, &localized_input_context)',
  'materialize_bindings(&localized_document, &effective_context)',
  'project_locale_policy_defaults_before_translation_materialization',
  'project_translation_catalog_materializes_before_bindings',
], 'multilingual Fly runtime ordering');
requireMarkers('runtimeValidation', [
  'validate_project_locale_policy(document)',
  'validate_translation_definitions(document)',
  'validate_localized_page_routes(document)',
  'strict_project_locale_policy_promotes_missing_coverage_to_errors',
  'duplicate_localized_slugs_block_publish_validation',
], 'locale, translation, and route publish validation');
requireMarkers('browserContract', [
  '"upsert_translation"',
  '"remove_translation"',
  '"set_locale_policy"',
  '"clear_locale_policy"',
  'command_producing_and_draft_intents_are_mutating',
], 'locale and translation mutation protection');
requireMarkers('pageBuilderLocale', [
  'pub struct PageBuilderLocaleContext',
  'pub fn from_request',
  'pub fn parse_accept_language',
  'accept_language_is_sorted_by_quality_and_stable_order',
], 'SSR locale negotiation API');
requireMarkers('pageBuilderRender', [
  'pub fn with_locale',
  'render_localized_runtime_document_html',
  'localized_runtime_render_uses_project_translation_catalog',
], 'localized runtime render API');
requireMarkers('storefrontLocalizedRoute', [
  'pub struct StorefrontLocalizedRouteOutput',
  'pub fn render_storefront_localized_slug',
  'pub fn render_storefront_localized_request',
  'pub fn LocalizedPageBuilderStorefront',
  'data-fly-localized-route="true"',
  'data-canonical-slug',
  'data-canonical-redirect',
  'localized_slug_renders_body_and_head_with_matched_locale',
  'request_locale_context_preserves_business_data',
], 'localized storefront route rendering');
requireMarkers('pagesIntent', [
  'set_runtime_locale',
  'runtime_locale_from_payload',
  'RUNTIME_LOCALE_FIELD',
  'RUNTIME_FALLBACK_LOCALES_FIELD',
], 'Pages SSR locale draft intent');
requireMarkers('browserIntent', [
  'SsrLocalePolicyRequest',
  '"set_locale_policy"',
  '"clear_locale_policy"',
  'locale_policy_form_uses_revision_protected_translation_history',
  'clearing_missing_locale_policy_is_a_clean_no_op',
], 'SSR locale policy dispatcher');
requireMarkers('ssrForms', [
  'SsrTranslationUpsertRequest',
  'SsrTranslationRemoveRequest',
  'EditorCommand::batch(commands)',
  'removing_translation_removes_its_bindings_in_one_history_entry',
], 'SSR translation form commands');
requireMarkers('ssrLocale', [
  'data-fly-ssr-locale="true"',
  'data-fly-intent-form="set_runtime_locale"',
], 'localized SSR locale panel');
requireMarkers('ssrLocalePolicy', [
  'pub struct SsrLocalePolicyRequest',
  'data-fly-ssr-locale-policy="true"',
  'data-fly-intent-form="set_locale_policy"',
  'data-fly-intent-form="clear_locale_policy"',
  'locale_policy_form_participates_in_editor_history',
  'clearing_missing_policy_is_an_idempotent_no_op',
  'strict_policy_rolls_back_when_required_translation_is_missing',
], 'localized SSR locale policy panel');
requireMarkers('ssrLocaleCoverage', [
  'data-fly-ssr-locale-coverage="true"',
  'data-fly-locale-summary',
  'data-fly-locale-gap',
  'analyze_project_locale_coverage',
  'coverage_panel_model_exposes_required_gap_paths',
  'page_builder.localeCoverage.title',
  'page_builder.localeCoverage.gapsTitle',
], 'localized SSR locale coverage panel');
requireMarkers('ssrTranslations', [
  'data-fly-ssr-translations="true"',
  'data-fly-intent-form="upsert_translation"',
  'data-fly-intent-form="remove_translation"',
  'TranslationCatalog::from_document',
], 'localized SSR translation panel');
requireMarkers('ssrInspector', [
  'crate::i18n::t',
  'UiRouteContext',
  'page_builder.ssrInspector.title',
], 'localized SSR inspector');
requireMarkers('adminCanvas', [
  'SsrLocalePanel',
  'SsrLocalePolicyPanel',
  'SsrLocaleCoveragePanel',
  'SsrTranslationsPanel',
  'SsrInspectorPanel',
], 'multilingual SSR editor composition');

const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder en/ru locale key parity failed');
}
const requiredKeys = [
  'page_builder.ssrInspector.title',
  'page_builder.ssrInspector.description',
  'page_builder.ssrInspector.runtimeContext',
  'page_builder.ssrInspector.localeTitle',
  'page_builder.ssrInspector.pageMetadata',
  'page_builder.ssrInspector.pageLifecycle',
  ...[
    'title', 'description', 'defaultLabel', 'supportedLabel', 'requiredLabel',
    'fallbackLabel', 'listHelp', 'enforceLabel', 'enforceHelp', 'save', 'clear',
  ].map((key) => `page_builder.localePolicy.${key}`),
  ...[
    'title', 'description', 'policyInvalid', 'ready', 'incomplete', 'strictOn',
    'strictOff', 'requiredBadge', 'optionalBadge', 'translations', 'metadata',
    'missing', 'gapsTitle', 'noGaps', 'translationGap', 'metadataGap',
  ].map((key) => `page_builder.localeCoverage.${key}`),
  ...[
    'title', 'description', 'empty', 'createTitle', 'idLabel', 'valuesLabel',
    'valuesHelp', 'fallbackLabel', 'save', 'existing', 'bindTitle', 'bindHelp',
    'bindKind', 'bindName', 'bind', 'remove',
  ].map((key) => `page_builder.translations.${key}`),
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