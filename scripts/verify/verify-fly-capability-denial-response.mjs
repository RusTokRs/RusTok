import { readFile } from 'node:fs/promises';

const paths = {
  access: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  exports: 'crates/rustok-page-builder/admin/src/lib.rs',
  pages: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  pagesExports: 'crates/rustok-pages/admin/src/lib.rs',
  server: 'apps/admin/src/main.rs',
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

for (const marker of [
  'pub struct BrowserCapabilityDenial',
  'impl std::error::Error for BrowserCapabilityDenial {}',
  'pub enum BrowserCapabilityAccessError',
  'Denied(#[from] BrowserCapabilityDenial)',
  'Dispatch(#[from] BrowserIntentDispatchError)',
  'pub fn browser_capability_denial(',
  'Result<(), BrowserCapabilityAccessError>',
  '"select_asset" => vec![EditorCapability::Assets, EditorCapability::Properties]',
  '| "rename_page"',
  'page_rename_uses_properties_capability',
  'selecting_an_asset_requires_asset_and_property_capabilities',
  'malformed_shortcut_remains_a_typed_dispatch_error',
]) {
  requireMarker('access', marker, `capability access contract is missing ${marker}`);
}
for (const forbidden of [
  'CAPABILITY_DENIAL_PREFIX',
  'FLY_CAPABILITY_DENIED:',
  'serde_json::from_str(payload)',
  'BrowserIntentDispatchError::Authoring(format!',
]) {
  rejectMarker('access', forbidden, `capability denial must not use string envelope ${forbidden}`);
}
for (const marker of [
  'BrowserCapabilityAccessError, BrowserCapabilityDenial,',
  'browser_capability_denial, validate_browser_capability_access,',
]) {
  requireMarker('exports', marker, `Page Builder admin export is missing ${marker}`);
}
for (const marker of [
  'pub enum PagesBrowserIntentAccessError',
  'Capability(#[from] BrowserCapabilityAccessError)',
  'Pages(#[from] PagesBrowserIntentError)',
  'pub fn capability_denial(&self)',
  'pages_preflight_preserves_typed_capability_denial',
  'PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Denied(_))',
]) {
  requireMarker('pages', marker, `Pages typed access boundary is missing ${marker}`);
}
requireMarker(
  'pagesExports',
  'PagesBrowserIntentAccessError,',
  'Pages admin must export the typed browser access error',
);
for (const marker of [
  'leptos_auth::api::fetch_current_user(',
  'PagesBrowserIntentAccessError',
  'let capability_denial = error.capability_denial();',
  'BrowserCapabilityAccessError::Denied(_)',
  '"code": "FLY_CAPABILITY_DENIED"',
  '"intent": denial.intent.as_str()',
  '"capability": denial.capability.as_str()',
  'StatusCode::FORBIDDEN',
]) {
  requireMarker('server', marker, `admin capability denial response is missing ${marker}`);
}
for (const forbidden of [
  'message.contains("requires editor capability")',
  'rustok_page_builder_admin::browser_capability_denial(error)',
]) {
  rejectMarker('server', forbidden, `admin must not recover capability type through ${forbidden}`);
}
rejectMarker(
  'server',
  'auth.user.as_ref().map(|user| user.role.as_str())',
  'admin endpoint must not trust the client-mirrored user cookie for authoritative role checks',
);

if (failures.length > 0) {
  console.error('Fly capability denial response verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly capability denial response verified.');
