import { readFile } from 'node:fs/promises';

const files = {
  workspace: 'Cargo.toml',
  appsAdmin: 'apps/admin/Cargo.toml',
  leptosCargo: 'crates/fly-leptos/Cargo.toml',
  leptosRoot: 'crates/fly-leptos/src/root.rs',
  browser: 'crates/fly-leptos/src/browser_runtime.rs',
  manifest: 'crates/rustok-page-builder/rustok-module.toml',
  adminCargo: 'crates/rustok-page-builder/admin/Cargo.toml',
  adminUi: 'crates/rustok-page-builder/admin/src/ui/leptos.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/admin_canvas.rs',
  canvasDocument: 'crates/rustok-page-builder/admin/src/editor/canvas_document.rs',
  canvasRuntime: 'crates/rustok-page-builder/admin/src/editor/canvas_runtime.js',
  canvasProtocol: 'crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs',
  facade: 'crates/rustok-page-builder/admin/src/transport/mod.rs',
  controller: 'crates/rustok-page-builder/admin/src/model.rs',
  pagesCargo: 'crates/rustok-pages/admin/Cargo.toml',
  pagesLib: 'crates/rustok-pages/admin/src/lib.rs',
  pagesBuilder: 'crates/rustok-pages/admin/src/builder.rs',
  pagesComposition: 'crates/rustok-pages/admin/src/composition.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
  uiIndex: 'docs/modules/UI_PACKAGES_INDEX.md',
};

const source = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
  ),
);

const failures = [];
const requireMarker = (key, marker, message) => {
  if (!source[key].includes(marker)) failures.push(message);
};
const forbidMarker = (key, marker, message) => {
  if (source[key].includes(marker)) failures.push(message);
};

requireMarker('workspace', '"crates/rustok-page-builder/admin"', 'Page Builder admin must be an explicit workspace member');
requireMarker('leptosCargo', '[target.\'cfg(target_arch = "wasm32")\'.dependencies]', 'fly-leptos browser dependencies must be wasm32-targeted');
for (const dependency of ['wasm-bindgen', 'web-sys', 'js-sys', '"Document"']) {
  requireMarker('leptosCargo', dependency, `fly-leptos must declare ${dependency}`);
}
requireMarker('leptosRoot', '#[cfg(target_arch = "wasm32")]', 'fly-leptos browser runtime must be cfg-gated');
for (const marker of [
  'pub struct EventListenerHandle',
  'pub struct ResizeObserverHandle',
  'pub struct WindowMessageSubscription',
  'pub struct IframeJsonSubscription',
  'pub struct IframeMessagePort',
  'subscribe_by_element_id',
  'event.source()',
  'Object::is',
  '.post_message(',
  'set_pointer_capture',
  'release_pointer_capture',
]) {
  requireMarker('browser', marker, `browser runtime is missing ${marker}`);
}
requireMarker('browser', 'origin == "*"', 'browser runtime must reject wildcard origins');

for (const marker of [
  'ui_classification = "admin_only"',
  '[provides.admin_ui]',
  'leptos_crate = "rustok-page-builder-admin"',
  'route_segment = "page-builder"',
  '[provides.admin_ui.i18n]',
  'default_locale = "en"',
  'supported_locales = ["en", "ru"]',
  'leptos_locales_path = "admin/locales"',
]) {
  requireMarker('manifest', marker, `Page Builder manifest is missing ${marker}`);
}

for (const dependency of [
  'fly =',
  'fly-ui =',
  'fly-leptos =',
  'rustok-page-builder =',
  'rustok-ui-core.workspace = true',
  'rustok-ui-i18n-leptos.workspace = true',
  'csr = ["leptos/csr"]',
]) {
  requireMarker('adminCargo', dependency, `admin package is missing ${dependency}`);
}
for (const forbidden of ['rustok-api', 'rustok-graphql', 'leptos_axum', 'reqwest']) {
  forbidMarker('adminCargo', forbidden, `admin package must not select transport through ${forbidden}`);
}
for (const hostMarker of [
  'rustok-page-builder-admin = { path = "../../crates/rustok-page-builder/admin"',
  '"rustok-page-builder-admin/csr"',
  '"rustok-page-builder-admin/hydrate"',
  '"rustok-page-builder-admin/ssr"',
]) {
  requireMarker('appsAdmin', hostMarker, `apps/admin is missing ${hostMarker}`);
}

for (const marker of [
  'pub fn PageBuilderAdmin()',
  'PageBuilderAdminWithController',
  'PageBuilderAdminHostContext',
  'PageBuilderAdminFacade',
  'Arc<dyn PageBuilderAdminFacade>',
  'UiRouteContext',
  'crate::i18n::t',
]) {
  requireMarker('adminUi', marker, `admin UI is missing ${marker}`);
}
for (const marker of [
  'render_canvas_srcdoc',
  'IframeJsonSubscription',
  'decode_canvas_message',
  'GeometrySnapshot',
  'UiIntent::SetViewport',
  'UiIntent::SetSelectedOverlay',
  'UiIntent::SetHoveredOverlay',
  'mark_save_started',
  'mark_save_failed',
  'acknowledge_save_for_hash',
  'sandbox="allow-scripts"',
]) {
  requireMarker('adminCanvas', marker, `admin canvas is missing ${marker}`);
}
forbidMarker('adminCanvas', 'allow-same-origin', 'default admin iframe must not combine scripts with same-origin privileges');

for (const marker of [
  'Content-Security-Policy',
  'include_str!("canvas_runtime.js")',
  'data-fly-component-id',
  'safe_attribute_name',
  'safe_style',
  'javascript:',
  'strip_tags',
]) {
  requireMarker('canvasDocument', marker, `instrumented canvas renderer is missing ${marker}`);
}
for (const marker of [
  'getBoundingClientRect',
  'ResizeObserver',
  'geometry_snapshot',
  'focus_requested',
  'hover_requested',
  'setTimeout(announce, 0)',
  'setTimeout(announce, 100)',
  'zoom: 1',
]) {
  requireMarker('canvasRuntime', marker, `iframe canvas runtime is missing ${marker}`);
}
for (const marker of [
  'CanvasComponentGeometry',
  'CanvasBridgeEnvelope',
  'GeometrySnapshot',
  'HoverRequested',
  'decode_canvas_message',
  'last_sequence',
]) {
  requireMarker('canvasProtocol', marker, `canvas protocol is missing ${marker}`);
}

for (const marker of [
  'pub trait PageBuilderAdminFacade: Send + Sync',
  'impl<T> PageBuilderAdminFacade for Arc<T>',
  'PageBuilderCapabilityRequest',
  'PageBuilderCapabilityResponse',
]) {
  requireMarker('facade', marker, `admin facade boundary is missing ${marker}`);
}
requireMarker('controller', 'FlyUiStateMachine', 'admin controller must use the framework-neutral Fly UI state machine');
requireMarker('controller', 'FlyEditor', 'admin controller must use the Fly engine');
requireMarker('controller', 'PageBuilderCapabilityRequest::Publish', 'admin controller must emit canonical publish requests');
requireMarker('controller', 'acknowledge_save_for_hash', 'controller must acknowledge the dispatched project hash');

for (const marker of [
  'rustok-page-builder-admin = { path = "../../rustok-page-builder/admin"',
  'rustok-page-builder = { path = "../../rustok-page-builder"',
]) {
  requireMarker('pagesCargo', marker, `Pages admin must declare ${marker}`);
}
requireMarker('pagesLib', 'pub use composition::PagesAdmin;', 'Pages must export the Fly-backed composition entrypoint');
for (const marker of [
  'impl PageBuilderAdminFacade for PagesBuilderFacade',
  'PageBuilderCapabilityRequest::Publish',
  'transport::fetch_page',
  'transport::update_page',
  'REVISION_CONFLICT',
  'canonicalize_builder_project',
  'copy_frame_component',
  'synchronize_frame_component',
  'Arc<dyn Fn() -> PagesBuilderSaveSnapshot + Send + Sync>',
]) {
  requireMarker('pagesBuilder', marker, `Pages consumer facade is missing ${marker}`);
}
for (const marker of [
  'PageBuilderAdminHostContext',
  'provide_context',
  'PageBuilderAdmin',
  'PagesBuilderFacade',
  'Arc<dyn PageBuilderAdminFacade>',
  'use_route_query_value',
  'crate::ui::leptos::PagesAdmin',
]) {
  requireMarker('pagesComposition', marker, `Pages composition is missing ${marker}`);
}
forbidMarker('pagesBuilder', 'graphql_adapter', 'Pages builder facade must not bypass the Pages transport facade');
forbidMarker('pagesComposition', 'graphql_adapter', 'Pages composition must not select a transport adapter');

const flattenKeys = (value, prefix = '') => Object.entries(value).flatMap(([key, nested]) => {
  const path = prefix ? `${prefix}.${key}` : key;
  return nested && typeof nested === 'object' && !Array.isArray(nested)
    ? flattenKeys(nested, path)
    : [path];
}).sort();
const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
for (const [name, locale] of [['en', en], ['ru', ru]]) {
  if (!locale.page_builder) failures.push(`${name} locale is missing page_builder messages`);
}
if (JSON.stringify(flattenKeys(en)) !== JSON.stringify(flattenKeys(ru))) {
  failures.push('Page Builder locale key parity failed');
}
requireMarker('uiIndex', '../../crates/rustok-page-builder/admin/README.md', 'UI package index must link the Page Builder admin package');

if (failures.length > 0) {
  console.error('Fly admin/browser runtime verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly admin/browser runtime verification passed.');
