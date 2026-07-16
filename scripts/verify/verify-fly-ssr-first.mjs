import { readFile } from 'node:fs/promises';

const paths = {
  appsAdmin: 'apps/admin/Cargo.toml',
  appsAdminMain: 'apps/admin/src/main.rs',
  appsAdminShell: 'apps/admin/src/app/shell.rs',
  authCargo: 'crates/leptos-auth/Cargo.toml',
  authStorage: 'crates/leptos-auth/src/storage.rs',
  flyBrowserCargo: 'crates/fly-browser/Cargo.toml',
  flyBrowserLib: 'crates/fly-browser/src/lib.rs',
  flyBrowserJs: 'crates/fly-browser/assets/fly-browser.js',
  flyLeptosCargo: 'crates/fly-leptos/Cargo.toml',
  flyLeptosRoot: 'crates/fly-leptos/src/root.rs',
  adminCargo: 'crates/rustok-page-builder/admin/Cargo.toml',
  adminAdapter: 'crates/rustok-page-builder/admin/src/ui/browser_adapter.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  adminEditorMod: 'crates/rustok-page-builder/admin/src/editor/mod.rs',
  adminTransport: 'crates/rustok-page-builder/admin/src/transport/mod.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  draftSession: 'crates/rustok-page-builder/admin/src/draft_session.rs',
  ssrDrop: 'crates/rustok-page-builder/admin/src/editor/ssr_drop.rs',
  ssrForms: 'crates/rustok-page-builder/admin/src/editor/ssr_forms.rs',
  ssrInspector: 'crates/rustok-page-builder/admin/src/editor/ssr_inspector.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
  palette: 'crates/rustok-page-builder/admin/src/editor/palette_layers.rs',
  toolbar: 'crates/rustok-page-builder/admin/src/editor/toolbar.rs',
  pagesCargo: 'crates/rustok-pages/admin/Cargo.toml',
  pagesIntent: 'crates/rustok-pages/admin/src/browser_intent.rs',
  pagesComposition: 'crates/rustok-pages/admin/src/composition.rs',
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
const forbidMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
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

requireMarkers('appsAdmin', [
  'default = ["ssr"]',
  '"dep:axum"',
  '"dep:tokio"',
  'rustok-page-builder-admin/ssr',
  'rustok-pages-admin/ssr',
  'wasm-bindgen = { version = "0.2", optional = true }',
  'web-sys = { version = "0.3", optional = true',
], 'apps/admin SSR profile');
for (const forbidden of [
  'leptos = { workspace = true, features = ["csr"] }',
  'leptos_i18n = { workspace = true, features = ["csr"] }',
]) {
  forbidMarker('appsAdmin', forbidden, `apps/admin must not force ${forbidden}`);
}
requireMarkers('appsAdminMain', [
  '#[tokio::main]',
  'generate_route_list(App)',
  '/api/admin/pages/{page_id}/builder/intents',
  'dispatch_pages_browser_intent',
], 'native admin SSR host');
requireMarkers('appsAdminShell', [
  '<!DOCTYPE html>',
  '<App/>',
  'intentionally omits `HydrationScripts`',
], 'classic admin document shell');
requireMarkers('authCargo', [
  "[target.'cfg(target_arch = \"wasm32\")'.dependencies]",
  'gloo-storage = { workspace = true }',
], 'auth browser storage boundary');
requireMarkers('authStorage', [
  'ServerAuthSnapshot',
  '#[cfg(target_arch = "wasm32")]',
  '#[cfg(not(target_arch = "wasm32"))]',
  'use_context::<ServerAuthSnapshot>()',
], 'request-scoped SSR auth storage');

requireMarker('flyBrowserCargo', 'name = "fly-browser"', 'standalone fly-browser package is missing');
requireMarkers('flyBrowserLib', [
  'FLY_BROWSER_ADAPTER_JS',
  'BrowserIntentEnvelope',
  'draft_token',
  'draft_generation',
  'pub fn is_mutating',
  '"patch_component_property"',
  '"patch_page_metadata"',
  '"create_page"',
], 'fly-browser contract');
for (const forbidden of ['wasm_bindgen', 'web_sys', 'wasm-bindgen', 'web-sys']) {
  forbidMarker('flyBrowserLib', forbidden, `fly-browser Rust contract must not depend on ${forbidden}`);
}
requireMarkers('flyBrowserJs', [
  'class FlyBrowserAdapter',
  'event.source !== this.iframe.contentWindow',
  'event.origin !== this.expectedOrigin',
  'credentials: "same-origin"',
  'activeDrag',
  'commitDrop',
  'draft_token:',
  'globalThis.location.reload()',
], 'SSR browser bridge');

requireMarkers('flyLeptosCargo', [
  'default = ["ssr"]',
  'ssr = ["leptos/ssr"]',
  'wasm-client = [',
  'optional = true',
], 'fly-leptos SSR feature boundary');
requireMarkers('flyLeptosRoot', [
  '#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]',
  'mod browser_runtime;',
  'mod browser_interaction;',
], 'fly-leptos browser module gate');
forbidMarker(
  'flyLeptosRoot',
  '#[cfg(target_arch = "wasm32")]',
  'fly-leptos browser modules must require wasm-client',
);

requireMarkers('adminCargo', [
  'default = ["ssr", "browser-js"]',
  'browser-js = []',
  'wasm-client = ["fly-leptos/wasm-client"]',
  'fly-leptos = { path = "../../fly-leptos", default-features = false }',
], 'Page Builder admin SSR boundary');
requireMarker(
  'adminTransport',
  'Future<Output = Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>>\n            + Send',
  'native Page Builder facade futures must be Send for Axum',
);
requireMarkers('adminAdapter', [
  'FLY_BROWSER_ADAPTER_JS',
  'data-fly-browser-adapter="fly_browser_v1"',
  'data-fly-intent-form',
  '__flyFormPayload',
  'fly_draft',
  'history.replaceState',
  'escape_json_for_script',
], 'SSR adapter component');
requireMarkers('adminEditorMod', [
  'mod ssr_forms;',
  'mod ssr_inspector;',
  'SsrInspectorPanel',
], 'SSR editor module wiring');
requireMarkers('adminCanvas', [
  'data-fly-browser-root="true"',
  'data-fly-runtime="ssr"',
  'data-fly-intent-endpoint',
  'SsrInspectorPanel',
], 'Admin canvas SSR wiring');
requireMarkers('browserIntent', [
  'dispatch_browser_intent',
  'RevisionConflict',
  'ProjectHashConflict',
  'ssr_form_intent',
  '"patch_component_property"',
  'GrapesJsV1Codec::encode_value',
], 'classic SSR intent dispatcher');
requireMarkers('draftSession', [
  'pub runtime_context: Value',
  'commit_with_context',
  'ordinary_commit_preserves_existing_runtime_context',
  'GenerationConflict',
], 'versioned SSR draft session');
requireMarkers('ssrDrop', [
  'pub enum SsrDropSource',
  'pub struct SsrDropRequest',
  'evaluate_placement',
  'EditorCommand::Insert',
  'EditorCommand::Move',
], 'stateless SSR drop policy');
requireMarkers('ssrForms', [
  'ssr_form_intent',
  'SsrComponentPropertyRequest',
  'SsrPageMetadataRequest',
  'PageCommand::Add',
  'PageCommand::Remove',
], 'classic SSR form commands');
requireMarkers('ssrInspector', [
  'data-fly-ssr-inspector="true"',
  'data-fly-intent-form="set_runtime_context"',
  'name="context_json"',
  'data-fly-intent-form="patch_component_property"',
  'data-fly-intent-form="patch_page_metadata"',
  'crate::i18n::t',
  'UiRouteContext',
  'page_builder.ssrInspector.title',
  'page_builder.ssrInspector.runtimeContext',
  'page_builder.ssrInspector.pageLifecycle',
], 'localized classic SSR inspector');
for (const forbidden of [
  '<h2 class="font-semibold">"Classic SSR inspector"</h2>',
  '<summary class="cursor-pointer text-xs font-semibold">"Runtime preview context"</summary>',
  'placeholder="SEO title"',
  '<strong class="text-xs">"Add page"</strong>',
]) {
  forbidMarker('ssrInspector', forbidden, `SSR inspector must not hardcode UI copy: ${forbidden}`);
}
const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder en/ru locale key parity failed');
}
const requiredInspectorLocaleKeys = [
  'page_builder.ssrInspector.title',
  'page_builder.ssrInspector.description',
  'page_builder.ssrInspector.runtimeContext',
  'page_builder.ssrInspector.runtimeContextAria',
  'page_builder.ssrInspector.runtimeContextHelp',
  'page_builder.ssrInspector.applyRuntimeContext',
  'page_builder.ssrInspector.canvasComponent',
  'page_builder.ssrInspector.componentProperty',
  'page_builder.ssrInspector.fieldKind',
  'page_builder.ssrInspector.attributeKind',
  'page_builder.ssrInspector.inlineStyleKind',
  'page_builder.ssrInspector.propertyNamePlaceholder',
  'page_builder.ssrInspector.valuePlaceholder',
  'page_builder.ssrInspector.removeProperty',
  'page_builder.ssrInspector.applyComponentPatch',
  'page_builder.ssrInspector.pageMetadata',
  'page_builder.ssrInspector.savePageMetadata',
  'page_builder.ssrInspector.pageLifecycle',
  'page_builder.ssrInspector.addPage',
  'page_builder.ssrInspector.renamePage',
  'page_builder.ssrInspector.removePage',
  'page_builder.ssrInspector.pageIdPlaceholder',
  'page_builder.ssrInspector.newPageIdPlaceholder',
  'page_builder.ssrInspector.pageNamePlaceholder',
  'page_builder.ssrInspector.newPageNamePlaceholder',
  'page_builder.ssrInspector.pageFallback',
];
for (const [localeName, locale] of [['en', en], ['ru', ru]]) {
  for (const key of requiredInspectorLocaleKeys) {
    const value = localeValue(locale, key);
    if (typeof value !== 'string' || value.trim() === '') {
      failures.push(`Page Builder ${localeName} locale is missing non-empty ${key}`);
    }
  }
}
for (const marker of ['data-fly-block-id', 'data-fly-component-id']) {
  requireMarker('palette', marker, `SSR control hooks are missing ${marker}`);
}
requireMarkers('toolbar', [
  'data-fly-action=format!("intent:{ssr_intent}")',
  'ssr_intent="undo"',
  'ssr_intent="save"',
  'feature = "wasm-client"',
], 'SSR toolbar hooks');

requireMarkers('pagesCargo', [
  'default = ["ssr"]',
  'leptos-auth/ssr',
  'rustok-page-builder-admin/ssr',
  'fly-browser =',
], 'Pages SSR feature boundary');
requireMarkers('pagesIntent', [
  'dispatch_pages_browser_intent',
  'pages_browser_draft_store',
  'set_runtime_context',
  'runtime_context_from_payload',
  'commit_with_context',
  'persistence.is_some()',
], 'Pages SSR intent service');
requireMarkers('pagesComposition', [
  'FLY_DRAFT_QUERY_KEY',
  'use_route_query_value(FLY_DRAFT_QUERY_KEY)',
  'pages_browser_draft_store().load',
  'draft.runtime_context',
  'with_runtime_context(runtime_context)',
], 'Pages draft restoration');

if (failures.length > 0) {
  console.error('Fly SSR-first verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly SSR-first browser boundary verified.');
