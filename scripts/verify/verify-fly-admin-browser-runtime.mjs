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
  facade: 'crates/rustok-page-builder/admin/src/transport/mod.rs',
  controller: 'crates/rustok-page-builder/admin/src/model.rs',
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
for (const dependency of ['wasm-bindgen', 'web-sys', 'js-sys']) {
  requireMarker('leptosCargo', dependency, `fly-leptos must declare ${dependency}`);
}
requireMarker('leptosRoot', '#[cfg(target_arch = "wasm32")]', 'fly-leptos browser runtime must be cfg-gated');
for (const marker of [
  'pub struct EventListenerHandle',
  'pub struct ResizeObserverHandle',
  'pub struct WindowMessageSubscription',
  'pub struct IframeMessagePort',
  'set_pointer_capture',
  'release_pointer_capture',
  'event.source()',
  'Object::is',
  '.post_message(',
]) {
  requireMarker('browser', marker, `browser runtime is missing ${marker}`);
}
requireMarker('browser', 'expected_origin == "*"', 'browser runtime must reject wildcard inbound origins');
requireMarker('browser', 'target_origin == "*"', 'browser runtime must reject wildcard outbound origins');
requireMarker('browser', 'envelope.is_accepted', 'browser runtime must apply protocol/instance/replay validation');

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
]) {
  requireMarker('adminCargo', dependency, `admin package is missing ${dependency}`);
}
for (const forbidden of ['rustok-api', 'rustok-graphql', 'leptos_axum', 'reqwest']) {
  forbidMarker('adminCargo', forbidden, `admin package must not select transport through ${forbidden}`);
}
for (const hostMarker of [
  'rustok-page-builder-admin = { path = "../../crates/rustok-page-builder/admin"',
  '"rustok-page-builder-admin/hydrate"',
  '"rustok-page-builder-admin/ssr"',
]) {
  requireMarker('appsAdmin', hostMarker, `apps/admin is missing ${hostMarker}`);
}
for (const marker of ['pub fn PageBuilderAdmin()', 'PageBuilderAdminWithController', 'UiRouteContext', 'crate::i18n::t']) {
  requireMarker('adminUi', marker, `admin UI is missing ${marker}`);
}
requireMarker('adminCanvas', 'UiRouteContext', 'admin canvas must consume the host route locale context');
requireMarker('adminCanvas', 'crate::i18n::t', 'admin canvas must use shared locale messages');
requireMarker('facade', 'PageBuilderCapabilityRequest', 'admin facade must consume the canonical capability request envelope');
requireMarker('facade', 'PageBuilderCapabilityResponse', 'admin facade must return the canonical capability response envelope');
requireMarker('controller', 'FlyUiStateMachine', 'admin controller must use the framework-neutral Fly UI state machine');
requireMarker('controller', 'FlyEditor', 'admin controller must use the Fly engine');
requireMarker('controller', 'PageBuilderCapabilityRequest::Publish', 'admin controller must emit canonical publish requests');
requireMarker('adminCanvas', 'sandbox="allow-scripts"', 'admin canvas must use an isolated iframe sandbox');
forbidMarker('adminCanvas', 'allow-same-origin', 'default admin iframe must not combine scripts with same-origin privileges');

const en = JSON.parse(source.localeEn);
const ru = JSON.parse(source.localeRu);
for (const [name, locale] of [['en', en], ['ru', ru]]) {
  if (!locale.page_builder) failures.push(`${name} locale is missing page_builder messages`);
}
if (JSON.stringify(Object.keys(en.page_builder).sort()) !== JSON.stringify(Object.keys(ru.page_builder).sort())) {
  failures.push('Page Builder locale top-level key parity failed');
}
requireMarker('uiIndex', '../../crates/rustok-page-builder/admin/README.md', 'UI package index must link the Page Builder admin package');

if (failures.length > 0) {
  console.error('Fly admin/browser runtime verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly admin/browser runtime verification passed.');
