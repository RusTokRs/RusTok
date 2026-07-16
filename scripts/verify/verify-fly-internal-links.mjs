import { readFile } from 'node:fs/promises';

const paths = {
  internalLink: 'crates/fly/src/internal_link.rs',
  localizedRoute: 'crates/fly/src/localized_route.rs',
  runtimePipeline: 'crates/fly/src/runtime_pipeline.rs',
  runtimeRender: 'crates/fly/src/runtime_render.rs',
  runtimeValidation: 'crates/fly/src/runtime_validation.rs',
  browserContract: 'crates/fly-browser/src/lib.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  ssrInternalLink: 'crates/rustok-page-builder/admin/src/editor/ssr_internal_link.rs',
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
const localeValue = (locale, path) => path
  .split('.')
  .reduce((value, segment) => value && typeof value === 'object' ? value[segment] : undefined, locale);

requireMarkers('internalLink', [
  'pub const FLY_PAGE_LINK_FIELD',
  'pub struct InternalPageLink',
  'pub struct InternalLinkMaterialization',
  'pub fn materialize_internal_page_links',
  'pub fn validate_internal_page_links',
  'internal_page_link_materializes_locale_specific_href',
  'missing_target_is_blocking_validation_and_preserves_raw_href_at_runtime',
  'fallback_href_is_used_when_target_page_has_no_slug',
], 'Fly internal page link contract');
requireMarkers('localizedRoute', [
  'pub fn localized_page_route_index',
  'pub struct LocalizedPageRouteEntry',
], 'localized route dependency');
requireMarkers('runtimePipeline', [
  'materialize_internal_page_links(document, &localized_input_context)',
  'materialize_localized_page_metadata(&linked_document, &localized_input_context)',
  'pub resolved_internal_links: usize',
  'pub fallback_internal_links: usize',
  'pub unresolved_internal_links: usize',
  'internal_page_links_materialize_after_locale_selection_and_before_bindings',
], 'internal link runtime ordering');
requireMarkers('runtimeRender', [
  'pub resolved_internal_links: usize',
  'pub fallback_internal_links: usize',
  'pub unresolved_internal_links: usize',
  'resolved_internal_links,',
], 'internal link render counters');
requireMarkers('runtimeValidation', [
  'validate_internal_page_links(document)',
  'missing_internal_page_link_target_blocks_publish_validation',
], 'internal link publish validation');
requireMarkers('browserContract', [
  '"set_internal_page_link"',
  '"remove_internal_page_link"',
  'command_producing_and_draft_intents_are_mutating',
], 'internal link mutation protection');
requireMarkers('browserIntent', [
  'SsrInternalPageLinkRequest',
  'SsrInternalPageLinkRemoveRequest',
  '"set_internal_page_link"',
  '"remove_internal_page_link"',
  'ssr_internal_page_link_intent',
  'ssr_remove_internal_page_link_intent',
  'internal_page_link_form_uses_revision_protected_patch_history',
], 'internal link browser dispatcher');
requireMarkers('ssrInternalLink', [
  'pub struct SsrInternalPageLinkRequest',
  'pub struct SsrInternalPageLinkRemoveRequest',
  'data-fly-ssr-internal-link="true"',
  'data-fly-intent-form="set_internal_page_link"',
  'data-fly-intent-form="remove_internal_page_link"',
  'data-fly-selected-component-input="true"',
  'InternalPageLink',
  'internal_link_form_uses_patch_history_and_preserves_extensions',
  'missing_target_is_rejected_before_dispatch',
], 'localized SSR internal link editor');
requireMarker(
  'adminCanvas',
  'SsrInternalPageLinkPanel',
  'internal link panel is not mounted in the admin canvas',
);

const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
const requiredKeys = [
  'page_builder.internalLink.title',
  'page_builder.internalLink.description',
  'page_builder.internalLink.empty',
  'page_builder.internalLink.targetLabel',
  'page_builder.internalLink.basePathLabel',
  'page_builder.internalLink.queryLabel',
  'page_builder.internalLink.fragmentLabel',
  'page_builder.internalLink.fallbackLabel',
  'page_builder.internalLink.save',
  'page_builder.internalLink.remove',
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
  console.error('Fly internal page link verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly internal page links verified.');
