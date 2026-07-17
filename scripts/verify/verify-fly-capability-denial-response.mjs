import { readFile } from 'node:fs/promises';

const paths = {
  access: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  exports: 'crates/rustok-page-builder/admin/src/lib.rs',
  pages: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  problem: 'crates/rustok-pages/admin/src/browser_problem.rs',
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
  'pub const BROWSER_CAPABILITY_DENIAL_CODE: &str = "FLY_CAPABILITY_DENIED";',
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
for (const marker of [
  'pub struct PagesBrowserIntentProblem',
  'pub fn from_error(error: &PagesBrowserIntentAccessError)',
  'code: Some(BROWSER_CAPABILITY_DENIAL_CODE.to_string())',
  'fn status_for_error(',
  'fn dispatch_status(',
  'capability_denial_has_stable_problem_contract',
  'revision_conflict_maps_to_conflict_without_capability_fields',
  'page_not_found_maps_to_not_found',
  'malformed_capability_preflight_payload_stays_unprocessable',
]) {
  requireMarker('problem', marker, `Pages problem mapping is missing ${marker}`);
}
for (const marker of [
  'mod browser_problem;',
  'PagesBrowserIntentProblem,',
  'PagesBrowserIntentAccessError,',
]) {
  requireMarker('pagesExports', marker, `Pages admin export is missing ${marker}`);
}
for (const marker of [
  'leptos_auth::api::fetch_current_user(',
  'PagesBrowserIntentAccessError',
  'PagesBrowserIntentProblem',
  'let problem = PagesBrowserIntentProblem::from(&error);',
  'StatusCode::from_u16(problem.status)',
  'serde_json::to_value(problem)',
]) {
  requireMarker('server', marker, `admin Page Builder adapter is missing ${marker}`);
}
for (const forbidden of [
  'message.contains("requires editor capability")',
  'rustok_page_builder_admin::browser_capability_denial(error)',
  '"code": "FLY_CAPABILITY_DENIED"',
  'BrowserCapabilityAccessError::Denied(_)',
  'PagesBrowserIntentError::PageNotFound',
  'PagesBrowserIntentError::RevisionConflict',
]) {
  rejectMarker('server', forbidden, `admin host must not own domain problem mapping through ${forbidden}`);
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
