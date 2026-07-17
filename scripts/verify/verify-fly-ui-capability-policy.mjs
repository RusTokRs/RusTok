import { readFile } from 'node:fs/promises';

const paths = {
  flyUiLib: 'crates/fly-ui/src/lib.rs',
  policy: 'crates/fly-ui/src/capability_policy.rs',
  machine: 'crates/fly-ui/src/machine.rs',
  flyUiTests: 'crates/fly-ui/src/tests.rs',
  host: 'crates/rustok-page-builder/admin/src/ui/leptos.rs',
  canvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  toolbar: 'crates/rustok-page-builder/admin/src/editor/toolbar.rs',
  palette: 'crates/rustok-page-builder/admin/src/editor/palette_layers.rs',
  browserCapabilities: 'crates/rustok-page-builder/admin/src/capability_access.rs',
  pageBuilderLib: 'crates/rustok-page-builder/admin/src/lib.rs',
  pagesBrowser: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  pagesLib: 'crates/rustok-pages/admin/src/lib.rs',
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
  'pub use capability_policy::*;',
], 'fly-ui capability module wiring');
requireMarkers('policy', [
  'pub enum EditorProviderState',
  'pub struct EditorCapabilityPolicy',
  'pub requested: CapabilityState',
  'pub tenant: CapabilityState',
  'pub permissions: CapabilityState',
  'pub provider_state: EditorProviderState',
  'pub fn evaluate(self) -> CapabilityState',
  '.intersection(self.tenant)',
  '.intersection(self.permissions)',
  'EditorProviderState::Degraded',
  'EditorProviderState::Unavailable',
  'pub const fn intersection(self, other: Self)',
  'pub const fn normalized(mut self)',
  'policy_intersects_tenant_and_permission_capabilities',
  'degraded_provider_disables_publish_without_destroying_draft_editing',
  'unavailable_provider_forces_read_only',
  'edit_denial_removes_mutating_sub_capabilities_but_can_preserve_publish',
], 'framework-neutral editor capability policy');
for (const forbidden of ['leptos', 'dioxus', 'web_sys', 'wasm_bindgen', 'rustok_', 'rustok-']) {
  rejectMarker('policy', forbidden, `capability policy must remain framework/RusTok neutral: ${forbidden}`);
}
requireMarkers('machine', [
  'SetEditableCapabilities(CapabilityState)',
  'editable_capabilities: CapabilityState',
  '#[serde(default = "CapabilityState::full")]',
  'pub fn with_editable_capabilities(',
  'pub const fn editable_capabilities(&self)',
  'pub fn set_editable_capabilities(',
  'self.editable_capabilities = capabilities.normalized();',
  'self.refresh_effective_capabilities();',
  'fn refresh_effective_capabilities(&mut self)',
  'self.state.drag = None;',
  'self.state.overlays.insertion = None;',
  'self.state.overlays.resize_handles_visible = false;',
], 'state-machine capability ceiling');
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
  'pub fn with_editor_capabilities(',
  'self.editor_capabilities = Some(capabilities.normalized());',
  'editor_capabilities=context.editor_capabilities',
  'editor_capabilities: Option<CapabilityState>',
], 'Page Builder host capability boundary');
requireMarkers('canvas', [
  'editor_capabilities: Option<CapabilityState>',
  'UiIntent::SetEditableCapabilities(capabilities)',
], 'Page Builder runtime capability application');
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
rejectMarker(
  'browserCapabilities',
  'fn capability_profile(',
  'browser preflight must use the supplied host capability profile',
);
requireMarkers('pageBuilderLib', [
  'mod capability_access;',
  'pub use capability_access::validate_browser_capability_access;',
], 'Page Builder browser capability export');
requireMarkers('pagesBrowser', [
  'dispatch_pages_browser_intent_with_capabilities(',
  'dispatch_pages_browser_intent_with_store_and_capabilities(',
  'validate_browser_capability_access(&envelope, capabilities)',
  'pages_preflight_rejects_capability_bypass',
], 'Pages capability-aware browser dispatch');
requireMarkers('pagesLib', [
  'dispatch_pages_browser_intent_with_capabilities,',
  'dispatch_pages_browser_intent_with_store_and_capabilities,',
], 'Pages capability dispatch exports');

if (failures.length > 0) {
  console.error('Fly UI capability policy verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly UI capability policy verified.');
