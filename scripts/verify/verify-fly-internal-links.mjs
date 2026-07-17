import { readFile } from 'node:fs/promises';

const paths = {
  flyLib: 'crates/fly/src/lib.rs',
  componentVisit: 'crates/fly/src/component_visit.rs',
  safeUrl: 'crates/fly/src/safe_url.rs',
  internalLink: 'crates/fly/src/internal_link.rs',
  localizedRoute: 'crates/fly/src/localized_route.rs',
  runtimePipeline: 'crates/fly/src/runtime_pipeline.rs',
  runtimeRender: 'crates/fly/src/runtime_render.rs',
  runtimeValidation: 'crates/fly/src/runtime_validation.rs',
  browserContract: 'crates/fly-browser/src/lib.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  ssrInternalLink: 'crates/rustok-page-builder/admin/src/editor/ssr_internal_link.rs',
  adminMod: 'crates/rustok-page-builder/admin/src/editor/mod.rs',
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
const rejectMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};
const requireMarkers = (key, markers, label) => {
  for (const marker of markers) requireMarker(key, marker, `${label} is missing ${marker}`);
};
const localeValue = (locale, path) => path
  .split('.')
  .reduce((value, segment) => value && typeof value === 'object' ? value[segment] : undefined, locale);

requireMarkers('flyLib', [
  'mod component_visit;',
  'mod safe_url;',
  'pub use component_visit::{visit_project_components, ComponentVisit};',
], 'Fly traversal and URL infrastructure');
requireMarkers('componentVisit', [
  'pub struct ComponentVisit',
  'pub fn visit_project_components(',
  'pub(crate) fn visit_project_components_mut(',
  'Mutation stays crate-private',
  'project.pages[{page_index}].component',
  'immutable_and_mutable_walks_share_page_depth_and_path_contract',
], 'shared component visitor');
requireMarkers('safeUrl', [
  'pub(crate) fn validate_safe_url',
  'pub(crate) fn normalize_safe_url',
  'rejects_network_paths_backslashes_controls_and_unsafe_schemes',
  'rejects_absolute_urls_without_authority_or_scheme_targets',
], 'shared safe URL boundary');
requireMarkers('internalLink', [
  'pub const FLY_PAGE_LINK_FIELD',
  'pub struct InternalPageLink',
  'pub struct InternalLinkMaterialization',
  'pub fn materialize_internal_page_links',
  'pub fn validate_internal_page_links',
  'component_visit::{visit_project_components, visit_project_components_mut}',
  'safe_url::normalize_safe_url',
  'GENERATED_INTERNAL_LINK_ATTRIBUTES',
  'clear_internal_link_materialization',
  'anonymous_component_diagnostics_use_the_shared_canonical_path',
  'internal_page_link_materializes_locale_specific_href',
  'missing_target_is_blocking_validation_and_clears_stale_href_at_runtime',
  'fallback_href_is_used_when_target_page_has_no_slug',
  'unsafe_fallback_and_network_base_path_are_rejected',
  'unencoded_query_and_backslash_fragment_are_rejected',
], 'Fly internal page link contract');
for (const forbidden of [
  'fn materialize_node(',
  'fn validate_node(',
  '#[allow(clippy::too_many_arguments)]',
]) {
  rejectMarker(
    'internalLink',
    forbidden,
    `internal links must use the shared visitor instead of ${forbidden}`,
  );
}
rejectMarker(
  'internalLink',
  'missing_target_is_blocking_validation_and_preserves_raw_href_at_runtime',
  'internal link tests must not preserve stale href for unresolved targets',
);
requireMarkers('localizedRoute', [
  'pub fn localized_page_route_index',
  'pub struct LocalizedPageRouteEntry',
], 'localized route dependency');
requireMarkers('runtimePipeline', [
  'validate_internal_page_links(&dynamic_document)',
  'materialize_internal_page_links(&dynamic_document, &effective_context)',
  'pub resolved_internal_links: usize',
  'pub fallback_internal_links: usize',
  'pub unresolved_internal_links: usize',
  'internal_page_links_materialize_after_bindings_and_repeaters',
  'runtime_bound_navigation_conflict_is_validated_before_materialization',
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
requireMarkers('adminMod', [
  'mod ssr_internal_link;',
  'SsrInternalPageLinkPanel',
  'SsrInternalPageLinkRemoveRequest',
  'SsrInternalPageLinkRequest',
], 'internal link editor registration');
requireMarker(
  'adminCanvas',
  '<SsrInternalPageLinkPanel runtime=ssr_internal_link_runtime />',
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