import { readFile } from 'node:fs/promises';

const paths = {
  flyUiCargo: 'crates/fly-ui/Cargo.toml',
  flyUiLib: 'crates/fly-ui/src/lib.rs',
  error: 'crates/fly-ui/src/error.rs',
  contribution: 'crates/fly-ui/src/contribution.rs',
  tests: 'crates/fly-ui/src/tests.rs',
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
  'mod contribution;',
  'pub use contribution::*;',
], 'fly-ui contribution module wiring');
requireMarkers('error', [
  'InvalidContribution',
  'DuplicateRenderer(String)',
  'DuplicatePropertyEditor(String)',
], 'contribution contract errors');
requireMarkers('contribution', [
  'pub struct AccessibilityMetadata',
  'pub struct RendererDescriptor',
  'pub struct PropertyEditorDescriptor',
  'pub struct ContributionDescriptor',
  'pub struct ResolvedRenderer',
  'pub struct ResolvedPropertyEditor',
  'pub struct ContributionRegistry',
  'pub fn register(',
  'pub fn resolve_renderer',
  'pub fn resolve_property_editor',
  'fn normalize_contribution(',
  'fn validate_renderer_conflicts',
  'fn validate_property_editor_conflicts',
  'renderer.presentations.is_empty()',
  'renderer.provider != contribution.provider',
  'editor.provider != contribution.provider',
  'accessibility.label_message_id',
  'duplicate_renderer_contract_is_rejected_atomically',
  'duplicate_property_editor_contract_is_rejected_atomically',
  'provider_ownership_and_accessibility_labels_are_required',
  'registration_normalizes_identity_and_optional_accessibility_ids',
], 'deterministic contribution registry');
for (const forbidden of [
  'leptos',
  'dioxus',
  'web_sys',
  'wasm_bindgen',
  'rustok_',
  'rustok-',
]) {
  rejectMarker(
    'contribution',
    forbidden,
    `fly-ui contribution contracts must remain framework/RusTok neutral: ${forbidden}`,
  );
  rejectMarker(
    'flyUiCargo',
    forbidden,
    `fly-ui dependencies must remain framework/RusTok neutral: ${forbidden}`,
  );
}
requireMarker(
  'tests',
  'contribution_filtering_is_capability_driven',
  'legacy capability filtering regression coverage is missing',
);

if (failures.length > 0) {
  console.error('Fly UI contribution verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly UI contributions verified.');