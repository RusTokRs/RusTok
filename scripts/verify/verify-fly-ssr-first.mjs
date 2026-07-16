import { readFile } from 'node:fs/promises';

const paths = {
  appsAdmin: 'apps/admin/Cargo.toml',
  flyBrowserCargo: 'crates/fly-browser/Cargo.toml',
  flyBrowserLib: 'crates/fly-browser/src/lib.rs',
  flyBrowserJs: 'crates/fly-browser/assets/fly-browser.js',
  flyLeptosCargo: 'crates/fly-leptos/Cargo.toml',
  flyLeptosRoot: 'crates/fly-leptos/src/root.rs',
  adminCargo: 'crates/rustok-page-builder/admin/Cargo.toml',
  adminAdapter: 'crates/rustok-page-builder/admin/src/ui/browser_adapter.rs',
  adminCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  adminHost: 'crates/rustok-page-builder/admin/src/ui/leptos.rs',
  browserIntent: 'crates/rustok-page-builder/admin/src/browser_intent.rs',
  palette: 'crates/rustok-page-builder/admin/src/editor/palette_layers.rs',
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
  'csr = ["leptos/csr"',
  '"dep:wasm-bindgen"',
  '"dep:web-sys"',
  'wasm-bindgen = { version = "0.2", optional = true }',
  'web-sys = { version = "0.3", optional = true',
  'leptos.workspace = true',
  'leptos_i18n.workspace = true',
]) {
  requireMarker('appsAdmin', marker, `apps/admin SSR default is missing ${marker}`);
}
for (const forbidden of [
  'leptos = { workspace = true, features = ["csr"] }',
  'leptos_i18n = { workspace = true, features = ["csr"] }',
]) {
  forbidMarker('appsAdmin', forbidden, `apps/admin must not force ${forbidden} in the SSR profile`);
}

requireMarker('flyBrowserCargo', 'name = "fly-browser"', 'standalone fly-browser package is missing');
for (const marker of [
  'FLY_BROWSER_ADAPTER_JS',
  'BrowserAdapterConfig',
  'BrowserIntentEnvelope',
  'FLY_BROWSER_PROTOCOL_V1',
  'pub fn is_mutating',
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
  'csr = ["leptos/csr", "wasm-client"]',
  'hydrate = ["leptos/hydrate", "wasm-client"]',
  'fly-browser =',
  'fly-leptos = { path = "../../fly-leptos", default-features = false }',
]) {
  requireMarker('adminCargo', marker, `Page Builder admin SSR boundary is missing ${marker}`);
}

for (const marker of [
  'PageBuilderBrowserAdapter',
  'FLY_BROWSER_ADAPTER_JS',
  'data-fly-browser-adapter="fly_browser_v1"',
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
]) {
  requireMarker('browserIntent', marker, `classic SSR intent dispatcher is missing ${marker}`);
}
for (const marker of ['data-fly-block-id', 'data-fly-component-id']) {
  requireMarker('palette', marker, `SSR control hooks are missing ${marker}`);
}

if (failures.length > 0) {
  console.error('Fly SSR-first verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly SSR-first browser boundary verified.');
