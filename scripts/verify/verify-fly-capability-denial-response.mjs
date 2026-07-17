import { readFile } from 'node:fs/promises';

const paths = {
  access: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  exports: 'crates/rustok-page-builder/admin/src/lib.rs',
  pages: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
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
  'const CAPABILITY_DENIAL_PREFIX: &str = "FLY_CAPABILITY_DENIED:"',
  'pub fn browser_capability_denial(',
  '"select_asset" => vec![EditorCapability::Assets, EditorCapability::Properties]',
  '| "rename_page"',
  'page_rename_uses_properties_capability',
  'selecting_an_asset_requires_asset_and_property_capabilities',
]) {
  requireMarker('access', marker, `capability access contract is missing ${marker}`);
}
requireMarker(
  'exports',
  'browser_capability_denial, validate_browser_capability_access, BrowserCapabilityDenial,',
  'Page Builder admin must export the capability denial contract',
);
for (const marker of [
  'browser_capability_denial(&error)',
  'Some(EditorCapability::Publish)',
]) {
  requireMarker('pages', marker, `Pages preflight is missing ${marker}`);
}
for (const marker of [
  'leptos_auth::api::fetch_current_user(',
  'rustok_page_builder_admin::browser_capability_denial(error)',
  '"code": "FLY_CAPABILITY_DENIED"',
  '"intent": denial.intent',
  '"capability": denial.capability.as_str()',
  'StatusCode::FORBIDDEN',
]) {
  requireMarker('server', marker, `admin capability denial response is missing ${marker}`);
}
rejectMarker(
  'server',
  'message.contains("requires editor capability")',
  'admin must not classify capability denials by parsing error prose',
);
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
