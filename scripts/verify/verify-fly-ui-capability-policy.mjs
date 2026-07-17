import { readFile } from 'node:fs/promises';

const paths = {
  flyUiLib: 'crates/fly-ui/src/lib.rs',
  policy: 'crates/fly-ui/src/capability_policy.rs',
  commandCapability: 'crates/fly-ui/src/command_capability.rs',
  machine: 'crates/fly-ui/src/machine.rs',
  flyUiTests: 'crates/fly-ui/src/tests.rs',
  host: 'crates/rustok-page-builder/admin/src/ui/leptos.rs',
  runtime: 'crates/rustok-page-builder/admin/src/editor/runtime.rs',
  canvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  controls: 'crates/rustok-page-builder/admin/src/editor/capability_controls.rs',
  pageManager: 'crates/rustok-page-builder/admin/src/editor/page_manager.rs',
  propertyFacade: 'crates/rustok-page-builder/admin/src/editor/properties_assets.rs',
  assetSection: 'crates/rustok-page-builder/admin/src/editor/asset_section.rs',
  browserAccess: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  pageBuilderLib: 'crates/rustok-page-builder/admin/src/lib.rs',
  pagesAccess: 'crates/rustok-pages/admin/src/access.rs',
  pagesComposition: 'crates/rustok-pages/admin/src/composition.rs',
  pagesBrowser: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  pagesProblem: 'crates/rustok-pages/admin/src/browser_problem.rs',
  pagesLib: 'crates/rustok-pages/admin/src/lib.rs',
  adminMain: 'apps/admin/src/main.rs',
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

requireMarkers('flyUiLib', [
  'mod capability_policy;',
  'mod command_capability;',
  'pub use capability_policy::*;',
  'pub use command_capability::*;',
], 'fly-ui capability wiring');
requireMarkers('policy', [
  'pub enum EditorCapability',
  'pub const ALL: [Self; 8]',
  'pub struct EditorCapabilityPolicy',
  'pub struct EditorCapabilityEvaluation',
  'pub fn evaluate_detailed(self)',
  '.intersection(self.tenant)',
  '.intersection(self.permissions)',
  'EditorProviderState::Degraded',
  'EditorProviderState::Unavailable',
  'effective: effective.normalized()',
  'capability_enum_is_stable_and_exhaustive',
], 'framework-neutral capability policy');
requireMarkers('commandCapability', [
  'pub struct CommandCapabilityRequirement',
  'pub fn for_command(command: &EditorCommand)',
  'EditorCommand::Asset { .. } => self.insert(EditorCapability::Assets)',
  'EditorCommand::StyleRule { .. } => self.insert(EditorCapability::Styles)',
  'PageCommand::Patch { .. } => self.insert(EditorCapability::Properties)',
  'EditorCommand::Batch { commands }',
  'fn extend_component_patch(',
  'mixed_and_batch_commands_require_every_specialized_capability',
], 'command-specific capability classifier');
for (const key of ['policy', 'commandCapability']) {
  for (const forbidden of ['leptos', 'dioxus', 'web_sys', 'wasm_bindgen', 'rustok_', 'rustok-']) {
    rejectMarker(key, forbidden, `${key} must remain framework and RusTok neutral: ${forbidden}`);
  }
}

requireMarkers('machine', [
  'SetEditableCapabilities(CapabilityState)',
  'editable_capabilities: CapabilityState',
  'CommandCapabilityRequirement::for_command(command.as_ref())',
  'requirement.first_missing(self.state.capabilities)',
  'capability.as_str().to_string()',
  'self.refresh_effective_capabilities();',
  'self.state.drag = None;',
  'self.state.overlays.insertion = None;',
], 'state-machine enforcement');
requireMarkers('flyUiTests', [
  'restricted_capabilities_survive_presentation_round_trip',
  'specialized_commands_cannot_bypass_disabled_capabilities',
  'batch_commands_require_every_specialized_capability_before_dispatch',
  'reviewer_profile_can_publish_but_cannot_mutate',
], 'state-machine regressions');

requireMarkers('host', [
  'pub editor_capabilities: Option<CapabilityState>',
  'pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>',
  'pub fn with_editor_capability_policy(',
  'policy.evaluate_detailed()',
], 'host capability boundary');
requireMarkers('runtime', [
  'pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>',
  'pub fn capability_enabled(&self, capability: EditorCapability)',
], 'runtime capability metadata');
requireMarkers('canvas', [
  'UiIntent::SetEditableCapabilities(capabilities)',
  '<CapabilityPolicyPanel runtime=capability_runtime />',
], 'canvas capability application');
requireMarkers('controls', [
  'pub(crate) fn CapabilityFieldset(',
  'disabled=move || !disabled_runtime.capability_enabled(capability)',
  'pub(crate) fn CapabilityPolicyPanel(',
  'EditorCapability::ALL.into_iter()',
], 'reusable capability controls');
requireMarkers('pageManager', [
  'capability=EditorCapability::Edit',
  'capability=EditorCapability::Properties',
  'UiIntent::ActivatePage',
], 'page lifecycle and metadata gates');
requireMarkers('propertyFacade', [
  'capability=EditorCapability::Properties',
  'capability=EditorCapability::Styles',
  'capability=EditorCapability::Assets',
], 'property style asset gates');
requireMarkers('assetSection', [
  'disabled=move || !use_runtime.capability_enabled(EditorCapability::Properties)',
], 'cross-capability asset apply gate');

requireMarkers('browserAccess', [
  'pub struct BrowserCapabilityDenial',
  'pub enum BrowserCapabilityAccessError',
  'Denied(#[from] BrowserCapabilityDenial)',
  'Dispatch(#[from] BrowserIntentDispatchError)',
  'Result<(), BrowserCapabilityAccessError>',
  'for capability in capability_requirements(envelope)?',
  '"select_asset" => vec![EditorCapability::Assets, EditorCapability::Properties]',
  '| "rename_page"',
  'malformed_shortcut_remains_a_typed_dispatch_error',
], 'typed browser capability preflight');
for (const forbidden of ['CAPABILITY_DENIAL_PREFIX', 'FLY_CAPABILITY_DENIED:']) {
  rejectMarker('browserAccess', forbidden, `browser access must not contain ${forbidden}`);
}
requireMarkers('pageBuilderLib', [
  'pub const BROWSER_CAPABILITY_DENIAL_CODE: &str = "FLY_CAPABILITY_DENIED";',
  'BrowserCapabilityAccessError, BrowserCapabilityDenial,',
], 'Page Builder capability exports');

requireMarkers('pagesAccess', [
  'pub fn pages_editor_permissions_for_role(',
  'Some("super_admin") | Some("admin")',
  'Some("manager")',
  'Some("customer") | None | Some(_)',
  'pub fn pages_editor_provider_state(',
], 'Pages auth and provider policy');
requireMarkers('pagesComposition', [
  'use_current_user',
  'pages_editor_capability_policy(',
  '.with_editor_capability_policy(editor_policy)',
], 'Pages visual host policy');
requireMarkers('pagesBrowser', [
  'pub enum PagesBrowserIntentAccessError',
  'Capability(#[from] BrowserCapabilityAccessError)',
  'Pages(#[from] PagesBrowserIntentError)',
  'validate_browser_capability_access(&envelope, capabilities)',
  'pages_preflight_preserves_typed_capability_denial',
  'pages_preflight_rejects_capability_bypass',
], 'Pages browser access boundary');
requireMarkers('pagesProblem', [
  'pub struct PagesBrowserIntentProblem',
  'pub fn from_error(error: &PagesBrowserIntentAccessError)',
  'code: Some(BROWSER_CAPABILITY_DENIAL_CODE.to_string())',
  'capability_denial_has_stable_problem_contract',
  'revision_conflict_maps_to_conflict_without_capability_fields',
], 'testable Pages problem mapping');
requireMarkers('pagesLib', [
  'mod browser_problem;',
  'PagesBrowserIntentProblem,',
  'PagesBrowserIntentAccessError,',
], 'Pages public access and problem exports');

requireMarkers('adminMain', [
  'leptos_auth::api::fetch_current_user(',
  'pages_editor_capability_policy_for_role(Some(',
  'PagesBrowserIntentProblem',
  'let problem = PagesBrowserIntentProblem::from(&error);',
  'StatusCode::from_u16(problem.status)',
  'serde_json::to_value(problem)',
], 'thin server-verified Axum adapter');
for (const forbidden of [
  'auth.user.as_ref().map(|user| user.role.as_str())',
  'BrowserCapabilityAccessError::Denied(_)',
  'PagesBrowserIntentError::PageNotFound',
  '"code": "FLY_CAPABILITY_DENIED"',
]) {
  rejectMarker('adminMain', forbidden, `admin host must not own ${forbidden}`);
}

for (const localeKey of ['localeEn', 'localeRu']) JSON.parse(source[localeKey]);
if (JSON.stringify(Object.keys(JSON.parse(source.localeEn)).sort())
  !== JSON.stringify(Object.keys(JSON.parse(source.localeRu)).sort())) {
  failures.push('Page Builder top-level en/ru locale parity failed');
}

if (failures.length > 0) {
  console.error('Fly UI capability policy verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly UI capability policy verified.');
