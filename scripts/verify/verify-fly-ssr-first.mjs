import { readFile } from 'node:fs/promises';

const paths = {
  appsAdmin: 'apps/admin/Cargo.toml',
  appsAdminMain: 'apps/admin/src/main.rs',
  appsAdminShell: 'apps/admin/src/app/shell.rs',
  flyBrowserCargo: 'crates/fly-browser/Cargo.toml',
  flyBrowserLib: 'crates/fly-browser/src/lib.rs',
  flyBrowserJs: 'crates/fly-browser/assets/fly-browser.js',
  flyLeptosCargo: 'crates/fly-leptos/Cargo.toml',
  flyLeptosRoot: 'crates/fly-leptos/src/root.rs',
  runtimePipeline: 'crates/fly/src/runtime_pipeline.rs',
  pageBuilderLib: 'crates/rustok-page-builder/src/lib.rs',
  pageBuilderBrowserHost: 'crates/rustok-page-builder/src/browser_host.rs',
  adminCargo: 'crates/rustok-page-builder/admin/Cargo.toml',
  adminAdapter: 'crates/rustok-page-builder/admin/src/ui/browser_adapter.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  adminEditorMod: 'crates/rustok-page-builder/admin/src/editor/mod.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  capabilityAccess: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  draftSession: 'crates/rustok-page-builder/admin/src/draft_session.rs',
  ssrAssets: 'crates/rustok-page-builder/admin/src/editor/ssr_assets.rs',
  pagesCargo: 'crates/rustok-pages/admin/Cargo.toml',
  pagesContributionBrowser: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  pagesProblem: 'crates/rustok-pages/admin/src/browser_problem.rs',
  pagesLib: 'crates/rustok-pages/admin/src/lib.rs',
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
const rejectMarker = (key, marker, message) => {
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

requireMarkers('appsAdmin', [
  'default = ["ssr"]',
  '"dep:axum"',
  '"dep:tokio"',
  'rustok-page-builder-admin/ssr',
  'rustok-pages-admin/ssr',
  'wasm-bindgen = { version = "0.2", optional = true }',
  'web-sys = { version = "0.3", optional = true',
], 'native admin SSR profile');
for (const forbidden of [
  'leptos = { workspace = true, features = ["csr"] }',
  'leptos_i18n = { workspace = true, features = ["csr"] }',
]) {
  rejectMarker('appsAdmin', forbidden, `apps/admin must not force ${forbidden}`);
}
requireMarkers('appsAdminMain', [
  '#[tokio::main]',
  'generate_route_list(App)',
  '/api/admin/pages/{page_id}/builder/intents',
  'leptos_auth::api::fetch_current_user(',
  'dispatch_pages_browser_intent_with_capabilities(',
  'PagesBrowserIntentProblem',
  'let problem = PagesBrowserIntentProblem::from(&error);',
  'StatusCode::from_u16(problem.status)',
  'serde_json::to_value(problem)',
], 'native admin Page Builder adapter');
for (const forbidden of [
  'auth.user.as_ref().map(|user| user.role.as_str())',
  'BrowserCapabilityAccessError::Denied(_)',
  'PagesBrowserIntentError::PageNotFound',
  '"code": "FLY_CAPABILITY_DENIED"',
]) {
  rejectMarker('appsAdminMain', forbidden, `admin host must not own ${forbidden}`);
}
requireMarkers('appsAdminShell', [
  '<!DOCTYPE html>',
  '<App/>',
  'intentionally omits `HydrationScripts`',
], 'classic server-rendered admin shell');

requireMarkers('flyBrowserCargo', [
  'name = "fly-browser"',
  'serde = { workspace = true, features = ["derive"] }',
  'serde_json.workspace = true',
], 'standalone Fly browser protocol crate');
for (const forbidden of ['wasm-bindgen', 'wasm_bindgen', 'web-sys', 'web_sys']) {
  rejectMarker('flyBrowserCargo', forbidden, `fly-browser Cargo must not depend on ${forbidden}`);
  rejectMarker('flyBrowserLib', forbidden, `fly-browser Rust contract must not depend on ${forbidden}`);
}
requireMarkers('flyBrowserLib', [
  'FLY_BROWSER_ADAPTER_JS',
  'pub enum BrowserIntentKind',
  'pub const ALL: [Self; 48]',
  'pub struct BrowserIntentEnvelope',
  'pub fn kind(&self) -> Option<BrowserIntentKind>',
  'pub fn is_mutating(&self)',
  'Self::UpsertAsset => "upsert_asset"',
  'Self::RemoveAsset => "remove_asset"',
  'Self::SelectAsset => "select_asset"',
  'intent_kind_names_are_unique_and_round_trip',
], 'framework-neutral typed browser protocol');
requireMarkers('flyBrowserJs', [
  'export class FlyBrowserAdapter',
  'event.source !== this.iframe.contentWindow',
  'event.origin !== this.expectedOrigin',
  'credentials: "same-origin"',
  'this.activeDrag',
  'draft_token:',
  'draft_generation:',
  'globalThis.location.reload()',
], 'progressive enhancement browser bridge');

requireMarkers('flyLeptosCargo', [
  'default = ["ssr"]',
  'ssr = ["leptos/ssr"]',
  'wasm-client = [',
  'dep:wasm-bindgen',
  'dep:web-sys',
], 'fly-leptos optional WASM profile');
requireMarkers('flyLeptosRoot', [
  '#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]',
  'mod browser_interaction;',
  'mod browser_runtime;',
], 'fly-leptos browser module gate');
rejectMarker(
  'flyLeptosRoot',
  '#[cfg(target_arch = "wasm32")]',
  'fly-leptos browser modules must require the explicit wasm-client feature',
);

requireMarker(
  'pageBuilderLib',
  'pub mod browser_host;',
  'Page Builder core must export the browser host contract',
);
requireMarkers('pageBuilderBrowserHost', [
  'PAGE_BUILDER_BROWSER_ADAPTER',
  'PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS',
  'PageBuilderBrowserModuleDescriptor',
  'page_builder_browser_module',
  'PAGE_BUILDER_BROWSER_SCRIPT_TYPE',
  'escape_browser_config_for_inline_script',
  'FlyBrowser?.bootstrap?.(__flyBrowserConfig)',
  'Symbol.for("fly.browser.ssr.controls")',
  'adapters: new WeakSet()',
  'fly:browser-ready',
  'adapter.abortController?.signal',
  'data-fly-intent-form',
  '__flyFormPayload',
  'delete payload[number.name]',
  'history.replaceState',
], 'framework-neutral Page Builder browser host');
for (const forbidden of [
  'autoMount === false',
  'FlyBrowser?.mountAll(__flyBrowserConfig)',
  'use leptos',
  'use dioxus',
]) {
  rejectMarker(
    'pageBuilderBrowserHost',
    forbidden,
    `Page Builder browser host must not own ${forbidden}`,
  );
}

requireMarkers('adminCargo', [
  'default = ["ssr", "browser-js"]',
  'browser-js = ["dep:wasm-bindgen", "dep:web-sys"]',
  'wasm-client = ["fly-leptos/wasm-client", "browser-js"]',
  'fly-browser = { path = "../../fly-browser" }',
  'fly-leptos = { path = "../../fly-leptos", default-features = false }',
], 'Page Builder admin feature boundary');
requireMarkers('adminAdapter', [
  'FLY_BROWSER_ADAPTER_JS',
  'page_builder_browser_module',
  'type=script_type',
  'data-fly-browser-adapter=adapter',
  'inner_html=source',
], 'thin Leptos browser adapter component');
for (const forbidden of [
  '__flyFormPayload',
  'fly:browser-ready',
  'adapter.abortController?.signal',
  'Symbol.for("fly.browser.ssr.controls")',
  'history.replaceState',
  'autoMount === false',
  'FlyBrowser?.mountAll(__flyBrowserConfig)',
]) {
  rejectMarker('adminAdapter', forbidden, `Leptos browser adapter must not own ${forbidden}`);
}
requireMarkers('adminCanvas', [
  'data-fly-browser-root="true"',
  'data-fly-runtime="ssr"',
  'data-fly-intent-endpoint',
  '<CapabilityPolicyPanel runtime=capability_runtime />',
  '<SsrInternalPageLinkPanel runtime=ssr_internal_link_runtime />',
  '<SsrActionsFormsPanel runtime=ssr_actions_forms_runtime />',
  '<SsrAssetPanel runtime=ssr_assets_runtime />',
], 'SSR authoring canvas');
requireMarkers('adminEditorMod', [
  'mod ssr_forms;',
  'mod ssr_internal_link;',
  'mod ssr_actions_forms;',
  'mod ssr_assets;',
  'SsrInternalPageLinkPanel',
  'SsrActionsFormsPanel',
  'SsrAssetApplyRequest, SsrAssetPanel, SsrAssetRemoveRequest, SsrAssetUpsertRequest,',
], 'SSR editor module graph');
requireMarkers('ssrAssets', [
  'pub fn ssr_asset_upsert_intent(',
  'pub fn ssr_asset_remove_intent(',
  'pub fn ssr_asset_apply_intent(',
  'source_allowed(&descriptor.source, descriptor.kind, &AssetPolicy::default())',
  'data-fly-intent-form="upsert_asset"',
  'data-fly-intent-form="select_asset"',
  'data-fly-intent-form="remove_asset"',
], 'SSR asset authoring module');
requireMarkers('browserIntent', [
  'pub fn dispatch_browser_intent(',
  'RevisionConflict',
  'ProjectHashConflict',
  'ssr_form_intent',
  'GrapesJsCodec::encode_value',
], 'classic SSR intent dispatcher');
requireMarkers('capabilityAccess', [
  'use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind};',
  'pub enum BrowserCapabilityAccessError',
  'Denied(#[from] BrowserCapabilityDenial)',
  'Dispatch(#[from] BrowserIntentDispatchError)',
  'Result<(), BrowserCapabilityAccessError>',
  'let Some(kind) = envelope.kind()',
  'BrowserIntentKind::SelectAsset =>',
], 'typed SSR capability preflight');
for (const forbidden of ['CAPABILITY_DENIAL_PREFIX', 'match envelope.intent.as_str()']) {
  rejectMarker('capabilityAccess', forbidden, `capability preflight must not contain ${forbidden}`);
}
requireMarkers('draftSession', [
  'pub runtime_context: Value',
  'commit_with_context',
  'GenerationConflict',
], 'versioned SSR draft session');

const pipelineStages = [
  'materialize_project_locale_context(document, input_context)',
  'materialize_project_translations(document, &locale_policy_context)',
  'materialize_runtime_locale_context(&translation_context)',
  'materialize_localized_page_metadata(document, &localized_input_context)',
  'materialize_context(&localized_document, &localized_input_context)',
  'materialize_bindings(&localized_document, &effective_context)',
  'materialize_runtime(&bound_document, &effective_context)',
  'validate_internal_page_links(&dynamic_document)',
  'validate_component_actions(&dynamic_document)',
  'materialize_internal_page_links(&dynamic_document, &effective_context)',
  'materialize_component_actions(&linked_document, &effective_context)',
];
let previousIndex = -1;
for (const stage of pipelineStages) {
  const index = source.runtimePipeline.indexOf(stage);
  if (index < 0) {
    failures.push(`runtime pipeline is missing ${stage}`);
  } else if (index <= previousIndex) {
    failures.push(`runtime pipeline stage is out of order: ${stage}`);
  }
  previousIndex = index;
}
for (const forbidden of [
  'materialize_context(document, &localized_input_context)',
  'materialize_bindings(document, &effective_context)',
  'materialize_runtime(&document, &effective_context)',
]) {
  rejectMarker('runtimePipeline', forbidden, `runtime pipeline must not regress to ${forbidden}`);
}
requireMarkers('runtimePipeline', [
  'locale_resolution_runs_before_computed_values_and_bindings',
  'internal_page_links_materialize_after_bindings_and_repeaters',
  'actions_and_forms_materialize_in_the_canonical_runtime_pipeline',
  'runtime_binding_can_supply_action_before_native_materialization',
  'runtime_bound_navigation_conflict_is_validated_before_materialization',
], 'runtime pipeline regression coverage');

requireMarkers('pagesCargo', [
  'default = ["ssr"]',
  'rustok-page-builder-admin/ssr',
], 'Pages SSR integration profile');
requireMarkers('pagesContributionBrowser', [
  'pub enum PagesBrowserIntentAccessError',
  'Capability(#[from] BrowserCapabilityAccessError)',
  'Pages(#[from] PagesBrowserIntentError)',
  'validate_browser_palette_access(&envelope, &pages_palette_block_access())',
  'validate_browser_capability_access(&envelope, capabilities)',
  'pages_preflight_preserves_typed_capability_denial',
], 'Pages SSR browser preflight');
requireMarkers('pagesProblem', [
  'pub struct PagesBrowserIntentProblem',
  'pub fn from_error(error: &PagesBrowserIntentAccessError)',
  'code: Some(BROWSER_CAPABILITY_DENIAL_CODE.to_string())',
  'fn status_for_error(',
  'capability_denial_has_stable_problem_contract',
], 'framework-neutral Pages problem mapping');
requireMarkers('pagesLib', [
  'mod browser_problem;',
  'pub use browser_problem::PagesBrowserIntentProblem;',
  'PagesBrowserIntentAccessError,',
], 'Pages typed browser exports');

const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder en/ru locale key parity failed');
}

if (failures.length > 0) {
  console.error('Fly SSR-first verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly SSR-first architecture verified.');
