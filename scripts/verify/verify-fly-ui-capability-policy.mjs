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
  propertiesSection: 'crates/rustok-page-builder/admin/src/editor/properties_section.rs',
  styleSection: 'crates/rustok-page-builder/admin/src/editor/style_section.rs',
  assetSection: 'crates/rustok-page-builder/admin/src/editor/asset_section.rs',
  diagnosticsSection: 'crates/rustok-page-builder/admin/src/editor/diagnostics_section.rs',
  toolbar: 'crates/rustok-page-builder/admin/src/editor/toolbar.rs',
  palette: 'crates/rustok-page-builder/admin/src/editor/palette_layers.rs',
  browserCapabilities: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  pageBuilderLib: 'crates/rustok-page-builder/admin/src/lib.rs',
  pagesAccess: 'crates/rustok-pages/admin/src/access.rs',
  pagesComposition: 'crates/rustok-pages/admin/src/composition.rs',
  pagesBrowser: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  pagesLib: 'crates/rustok-pages/admin/src/lib.rs',
  adminMain: 'apps/admin/src/main.rs',
  localeEn: 'crates/rustok-page-builder/admin/locales/en.json',
  localeRu: 'crates/rustok-page-builder/admin/locales/ru.json',
};

const source = Object.fromEntries(await Promise.all(
  Object.entries(paths).map(async ([key, path]) => [key, await readFile(path, 'utf8')]),
));
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
], 'fly-ui capability module wiring');
requireMarkers('policy', [
  'pub enum EditorCapability',
  'pub const ALL: [Self; 8]',
  'pub enum EditorProviderState',
  'pub struct EditorCapabilityPolicy',
  'pub struct EditorCapabilityEvaluation',
  'pub fn evaluate_detailed(self)',
  'pub requested: CapabilityState',
  'pub tenant: CapabilityState',
  'pub permissions: CapabilityState',
  'pub provider_state: EditorProviderState',
  '.intersection(self.tenant)',
  '.intersection(self.permissions)',
  'EditorProviderState::Degraded',
  'EditorProviderState::Unavailable',
  'pub const fn allows(self, capability: EditorCapability)',
  'policy_intersects_tenant_and_permission_capabilities',
  'degraded_provider_disables_publish_without_destroying_draft_editing',
  'unavailable_provider_forces_read_only',
  'capability_enum_is_stable_and_exhaustive',
], 'framework-neutral editor capability policy');
requireMarkers('commandCapability', [
  'pub struct CommandCapabilityRequirement',
  'pub fn for_command(command: &EditorCommand)',
  'EditorCommand::Asset { .. } => self.insert(EditorCapability::Assets)',
  'EditorCommand::StyleRule { .. } => self.insert(EditorCapability::Styles)',
  'PageCommand::Patch { .. } => self.insert(EditorCapability::Properties)',
  'EditorCommand::Batch { commands }',
  'fn extend_component_patch(',
  'property_and_style_patches_require_independent_capabilities',
  'mixed_and_batch_commands_require_every_specialized_capability',
], 'command-specific capability classifier');
for (const key of ['policy', 'commandCapability']) {
  for (const forbidden of ['leptos', 'dioxus', 'web_sys', 'wasm_bindgen', 'rustok_', 'rustok-']) {
    rejectMarker(key, forbidden, `${key} must remain framework/RusTok neutral: ${forbidden}`);
  }
}
requireMarkers('machine', [
  'SetEditableCapabilities(CapabilityState)',
  'editable_capabilities: CapabilityState',
  '#[serde(default = "CapabilityState::full")]',
  'self.editable_capabilities = capabilities.normalized();',
  'self.refresh_effective_capabilities();',
  'CommandCapabilityRequirement::for_command(command.as_ref())',
  'requirement.first_missing(self.state.capabilities)',
  'capability.as_str().to_string()',
  'self.state.drag = None;',
  'self.state.overlays.insertion = None;',
  'self.state.overlays.resize_handles_visible = false;',
], 'state-machine capability enforcement');
rejectMarker(
  'machine',
  'self.state.capabilities = if presentation.is_editable() {\n                    CapabilityState::full()',
  'presentation switching must not restore unrestricted capabilities',
);
requireMarkers('flyUiTests', [
  'restricted_capabilities_survive_presentation_round_trip',
  'withdrawing_drag_capability_cancels_active_drag_and_overlay',
  'reviewer_profile_can_publish_but_cannot_mutate',
], 'capability state-machine regressions');
requireMarkers('host', [
  'pub editor_capabilities: Option<CapabilityState>',
  'pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>',
  'pub fn with_editor_capability_policy(',
  'policy.evaluate_detailed()',
  'editor_capability_evaluation=context.editor_capability_evaluation',
], 'Page Builder host capability boundary');
requireMarkers('runtime', [
  'pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>',
  'pub fn with_editor_capability_evaluation(',
  'pub fn capability_enabled(&self, capability: EditorCapability)',
], 'Page Builder runtime capability metadata');
requireMarkers('canvas', [
  'editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>',
  'UiIntent::SetEditableCapabilities(capabilities)',
  '<CapabilityPolicyPanel runtime=capability_runtime />',
], 'Page Builder runtime capability application');
requireMarkers('controls', [
  'pub(crate) fn CapabilityFieldset(',
  'disabled=move || !disabled_runtime.capability_enabled(capability)',
  'pub(crate) fn CapabilityPolicyPanel(',
  'data-fly-capability-policy="true"',
  'data-fly-provider-state=provider.as_str()',
  'EditorCapability::ALL.into_iter()',
], 'reusable capability controls');
requireMarkers('pageManager', [
  '<CapabilityFieldset runtime=edit_gate_runtime capability=EditorCapability::Edit>',
  'runtime=properties_gate_runtime',
  'capability=EditorCapability::Properties',
  'UiIntent::ActivatePage',
], 'Page Manager lifecycle/metadata gates');
requireMarkers('propertyFacade', [
  'capability=EditorCapability::Properties',
  'capability=EditorCapability::Styles',
  'capability=EditorCapability::Assets',
  '<DiagnosticsSection runtime=diagnostics_runtime />',
], 'property/style/asset capability facade');
requireMarkers('propertiesSection', ['pub(crate) fn PropertiesSection'], 'properties section split');
requireMarkers('styleSection', ['pub(crate) fn StyleSection'], 'style section split');
requireMarkers('assetSection', ['pub(crate) fn AssetSection'], 'asset section split');
requireMarkers('diagnosticsSection', ['pub(crate) fn DiagnosticsSection'], 'diagnostics section split');
requireMarkers('toolbar', [
  '!controller.ui().state.capabilities.history',
  '!controller.ui().state.capabilities.publish',
  '!controller.ui().state.capabilities.clipboard',
  'capabilities.drag_drop',
  'capabilities.edit',
], 'toolbar capability controls');
requireMarkers('palette', [
  'let can_insert = capabilities.edit;',
  'let can_drag = capabilities.drag_drop;',
  'draggable=can_drag',
  'disabled=!can_insert',
  'disabled=!can_drag',
], 'palette capability controls');
requireMarkers('browserCapabilities', [
  'pub fn validate_browser_capability_access(',
  'let capabilities = capabilities.normalized();',
  'capability_requirement(envelope, capabilities)',
  '"undo" | "redo" => Some(("history", capabilities.history))',
  'Some(("clipboard", capabilities.clipboard))',
  '"save" => Some(("publish", capabilities.publish))',
  'Some(("drag_drop", capabilities.drag_drop))',
  'Some(("styles", capabilities.styles))',
  'Some(("properties", capabilities.properties))',
  'Some(("assets", capabilities.assets))',
  '_ if envelope.is_mutating() => Some(("edit", capabilities.edit))',
  'supplied_profile_is_authoritative',
], 'browser capability preflight');
requireMarkers('pageBuilderLib', [
  'mod capability_access;',
  'pub use capability_access::validate_browser_capability_access;',
], 'Page Builder browser capability export');
requireMarkers('pagesAccess', [
  'pub fn pages_editor_capability_policy(',
  'pub fn pages_editor_permissions_for_role(',
  'Some("super_admin") | Some("admin")',
  'Some("manager")',
  'Some("customer") | None | Some(_)',
  'pub fn pages_editor_provider_state(',
  'ContributionAssemblySeverity::Error',
  'ContributionAssemblySeverity::Warning',
], 'Pages auth/provider capability adapter');
requireMarkers('pagesComposition', [
  'use_current_user',
  'pages_editor_capability_policy(',
  '.with_editor_capability_policy(editor_policy)',
], 'Pages visual editor capability policy');
requireMarkers('pagesBrowser', [
  'dispatch_pages_browser_intent_with_capabilities(',
  'dispatch_pages_browser_intent_with_store_and_capabilities(',
  'validate_browser_capability_access(&envelope, capabilities)',
  'pages_preflight_rejects_capability_bypass',
], 'Pages capability-aware browser dispatch');
requireMarkers('pagesLib', [
  'pages_editor_capability_policy_for_role,',
  'dispatch_pages_browser_intent_with_capabilities,',
], 'Pages capability exports');
requireMarkers('adminMain', [
  'leptos_auth::api::fetch_current_user(',
  'pages_editor_capability_policy_for_role(Some(',
  'dispatch_pages_browser_intent_with_capabilities(',
  'StatusCode::FORBIDDEN',
  'Page Builder access token is missing',
], 'server-verified Page Builder endpoint policy');
rejectMarker(
  'adminMain',
  'auth.user.as_ref().map(|user| user.role.as_str())',
  'Page Builder endpoint must not trust the client-mirrored user cookie for authoritative role checks',
);
for (const localeKey of ['localeEn', 'localeRu']) {
  requireMarkers(localeKey, [
    '"capabilityPolicy"',
    '"provider"',
    '"requested"',
    '"tenant"',
    '"permission"',
    '"effective"',
  ], `${localeKey} capability messages`);
  JSON.parse(source[localeKey]);
}

if (failures.length > 0) {
  console.error('Fly UI capability policy verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly UI capability policy verified.');
