import { readFile } from 'node:fs/promises';

const paths = {
  appsAdmin: 'apps/admin/Cargo.toml',
  appsAdminMain: 'apps/admin/src/main.rs',
  appsAdminShell: 'apps/admin/src/app/shell.rs',
  authCargo: 'crates/leptos-auth/Cargo.toml',
  authStorage: 'crates/leptos-auth/src/storage.rs',
  authContext: 'crates/leptos-auth/src/context.rs',
  flyBrowserCargo: 'crates/fly-browser/Cargo.toml',
  flyBrowserLib: 'crates/fly-browser/src/lib.rs',
  flyBrowserJs: 'crates/fly-browser/assets/fly-browser.js',
  flyLeptosCargo: 'crates/fly-leptos/Cargo.toml',
  flyLeptosRoot: 'crates/fly-leptos/src/root.rs',
  adminCargo: 'crates/rustok-page-builder/admin/Cargo.toml',
  adminAdapter: 'crates/rustok-page-builder/admin/src/ui/browser_adapter.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  adminHost: 'crates/rustok-page-builder/admin/src/ui/leptos.rs',
  adminTransport: 'crates/rustok-page-builder/admin/src/transport/mod.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  ssrDrop: 'crates/rustok-page-builder/admin/src/editor/ssr_drop.rs',
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

for (const marker of [
  'default = ["ssr"]',
  '"dep:axum"',
  '"dep:tokio"',
  '"dep:wasm-bindgen"',
  '"dep:web-sys"',
  'rustok-page-builder-admin/ssr',
  'rustok-pages-admin/ssr',
  'wasm-bindgen = { version = "0.2", optional = true }',
  'web-sys = { version = "0.3", optional = true',
  'console_error_panic_hook = { version = "0.1", optional = true }',
]) {
  requireMarker('appsAdmin', marker, `apps/admin SSR default is missing ${marker}`);
}
for (const forbidden of [
  'leptos = { workspace = true, features = ["csr"] }',
  'leptos_i18n = { workspace = true, features = ["csr"] }',
]) {
  forbidMarker('appsAdmin', forbidden, `apps/admin must not force ${forbidden} in the SSR profile`);
}
for (const marker of [
  '#[tokio::main]',
  'generate_route_list(App)',
  'LeptosRoutes',
  '/api/admin/pages/{page_id}/builder/intents',
  'dispatch_pages_browser_intent',
  '#[cfg(any(feature = "csr", feature = "hydrate"))]',
]) {
  requireMarker('appsAdminMain', marker, `native admin SSR host is missing ${marker}`);
}
for (const marker of [
  '<!DOCTYPE html>',
  '<App/>',
  'intentionally omits `HydrationScripts`',
]) {
  requireMarker('appsAdminShell', marker, `classic admin document shell is missing ${marker}`);
}

for (const marker of [
  "[target.'cfg(target_arch = \"wasm32\")'.dependencies]",
  'gloo-storage = { workspace = true }',
]) {
  requireMarker('authCargo', marker, `auth browser storage boundary is missing ${marker}`);
}
for (const marker of [
  'ServerAuthSnapshot',
  '#[cfg(target_arch = "wasm32")]',
  '#[cfg(not(target_arch = "wasm32"))]',
  'use_context::<ServerAuthSnapshot>()',
]) {
  requireMarker('authStorage', marker, `request-scoped SSR auth storage is missing ${marker}`);
}
forbidMarker(
  'authStorage',
  'use gloo_storage::{LocalStorage, Storage};\n\npub fn',
  'native auth storage must not unconditionally compile LocalStorage',
);
for (const marker of [
  '#[cfg(target_arch = "wasm32")]',
  'use_interval_fn',
  '#[cfg(not(target_arch = "wasm32"))]',
]) {
  requireMarker('authContext', marker, `auth refresh/runtime split is missing ${marker}`);
}

requireMarker('flyBrowserCargo', 'name = "fly-browser"', 'standalone fly-browser package is missing');
for (const marker of [
  'FLY_BROWSER_ADAPTER_JS',
  'BrowserAdapterConfig',
  'BrowserIntentEnvelope',
  'FLY_BROWSER_PROTOCOL_V1',
  'pub fn is_mutating',
  '"drop"',
]) {
  requireMarker('flyBrowserLib', marker, `fly-browser contract is missing ${marker}`);
}
for (const marker of [
  'class FlyBrowserAdapter',
  'event.source !== this.iframe.contentWindow',
  'event.origin !== this.expectedOrigin',
  'fly:canvas-message',
  'fly:browser-intent',
  'credentials: "same-origin"',
  'instance_id: adapter.instanceId',
  'project_hash:',
  'activeDrag',
  'updateDropCandidate',
  'commitDrop',
  'this.emitIntent("drop", payload)',
  'globalThis.location.reload()',
  'rustok-admin-token',
]) {
  requireMarker('flyBrowserJs', marker, `SSR browser bridge is missing ${marker}`);
}
for (const forbidden of ['wasm_bindgen', 'web_sys', 'wasm-bindgen', 'web-sys']) {
  forbidMarker('flyBrowserLib', forbidden, `fly-browser Rust contract must not depend on ${forbidden}`);
}

for (const marker of [
  'default = ["ssr"]',
  'ssr = ["leptos/ssr"]',
  'wasm-client = [',
  'optional = true',
]) {
  requireMarker('flyLeptosCargo', marker, `fly-leptos SSR feature boundary is missing ${marker}`);
}
for (const marker of [
  '#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]',
  'mod browser_runtime;',
  'mod browser_interaction;',
]) {
  requireMarker('flyLeptosRoot', marker, `fly-leptos root is missing ${marker}`);
}
forbidMarker(
  'flyLeptosRoot',
  '#[cfg(target_arch = "wasm32")]',
  'fly-leptos browser modules must require the explicit wasm-client feature',
);

for (const marker of [
  'default = ["ssr", "browser-js"]',
  'browser-js = []',
  'wasm-client = ["fly-leptos/wasm-client"]',
  'fly-browser =',
  'fly-leptos = { path = "../../fly-leptos", default-features = false }',
]) {
  requireMarker('adminCargo', marker, `Page Builder admin SSR boundary is missing ${marker}`);
}
requireMarker(
  'adminTransport',
  'Future<Output = Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>>\n            + Send',
  'native Page Builder facade futures must be Send for Axum',
);
for (const marker of [
  'PageBuilderBrowserAdapter',
  'FLY_BROWSER_ADAPTER_JS',
  'data-fly-browser-adapter="fly_browser_v1"',
  'escape_json_for_script',
  '\\u003c',
]) {
  requireMarker('adminAdapter', marker, `SSR adapter component is missing ${marker}`);
}
for (const marker of [
  'data-fly-browser-root="true"',
  'data-fly-runtime="ssr"',
  'data-fly-intent-endpoint',
  'PageBuilderBrowserAdapter',
]) {
  requireMarker('adminCanvas', marker, `Admin canvas SSR wiring is missing ${marker}`);
}
for (const marker of [
  'browser_intent_endpoint',
  'browser_csrf_token',
  'with_browser_intent_endpoint',
  'with_browser_csrf_token',
]) {
  requireMarker('adminHost', marker, `Admin host context is missing ${marker}`);
}
for (const marker of [
  'dispatch_browser_intent',
  'BrowserIntentEnvelope',
  'RevisionConflict',
  'ProjectHashConflict',
  'GrapesJsV1Codec::encode_value',
  'PageBuilderCapabilityRequest',
  'SsrDropRequest',
  '"drop"',
]) {
  requireMarker('browserIntent', marker, `classic SSR intent dispatcher is missing ${marker}`);
}
for (const marker of [
  'pub enum SsrDropSource',
  'pub struct SsrDropRequest',
  'evaluate_placement',
  'EditorCommand::Insert',
  'EditorCommand::Move',
]) {
  requireMarker('ssrDrop', marker, `stateless SSR drop policy is missing ${marker}`);
}
for (const marker of ['data-fly-block-id', 'data-fly-component-id']) {
  requireMarker('palette', marker, `SSR control hooks are missing ${marker}`);
}
for (const marker of [
  'data-fly-action=format!("intent:{ssr_intent}")',
  'ssr_intent="undo"',
  'ssr_intent="save"',
  'ssr_intent="remove_selected"',
  'feature = "wasm-client"',
]) {
  requireMarker('toolbar', marker, `SSR toolbar hooks are missing ${marker}`);
}

for (const marker of [
  'default = ["ssr"]',
  'leptos-auth/ssr',
  'rustok-page-builder-admin/ssr',
  'fly-browser =',
]) {
  requireMarker('pagesCargo', marker, `Pages SSR feature boundary is missing ${marker}`);
}
for (const marker of [
  'dispatch_pages_browser_intent',
  'PagesBuilderSaveSnapshot',
  'transport::fetch_page',
  'PagesBuilderFacade',
  'reload: envelope.is_mutating()',
]) {
  requireMarker('pagesIntent', marker, `Pages SSR intent service is missing ${marker}`);
}
for (const marker of [
  '/api/admin/pages/{page_id}/builder/intents',
  'with_browser_intent_endpoint',
  'PagesBuilderFacade',
]) {
  requireMarker('pagesComposition', marker, `Pages consumer SSR ownership is missing ${marker}`);
}

if (failures.length > 0) {
  console.error('Fly SSR-first verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly SSR-first browser boundary verified.');
