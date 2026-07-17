import { readFile } from 'node:fs/promises';

const paths = {
  flyLib: 'crates/fly/src/lib.rs',
  flyError: 'crates/fly/src/error.rs',
  capability: 'crates/fly/src/interaction_capability.rs',
  capabilityGate: 'crates/fly/src/interaction_capability_gate.rs',
  actionModel: 'crates/fly/src/action/model.rs',
  componentVisit: 'crates/fly/src/component_visit.rs',
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

requireMarkers('flyLib', [
  'mod interaction_capability;',
  'mod interaction_capability_gate;',
  'pub use interaction_capability::*;',
  'pub use interaction_capability_gate::*;',
], 'interaction capability module wiring');
requireMarker(
  'flyError',
  'InvalidInteractionCapability(String)',
  'interaction capabilities need a domain-specific contract error',
);
requireMarkers('componentVisit', [
  'pub fn visit_project_components(',
  'pub(crate) fn visit_project_components_mut(',
], 'capability traversal boundary');
requireMarkers('actionModel', [
  'ProviderAction',
  'pub struct ComponentForm',
], 'provider-neutral interaction models');
requireMarkers('capability', [
  'pub enum InteractionCapabilityKind',
  'pub enum InteractionRuntimeTarget',
  'pub struct InteractionCapabilityDefinition',
  'pub struct InteractionCapabilityRegistry',
  'pub enum MissingInteractionCapabilityPolicy',
  'pub struct InteractionCapabilityPolicy',
  'pub fn validate_interaction_capabilities(',
  'pub fn validate_component_actions_with_capabilities(',
  'visit_project_components(&document.project',
  'capability.input_kind.accepts(interaction.input)',
  'interaction_capability_missing',
  'interaction_capability_input_kind_mismatch',
  'strict_policy_rejects_unregistered_provider_forms',
  'capability_input_kind_is_validated',
  'permissive_policy_preserves_unknown_provider_compatibility',
  'invalid_capability_identifier_has_domain_error',
], 'provider interaction capability registry');
for (const forbidden of [
  '#[allow(clippy::too_many_arguments)]',
  'FlyError::InvalidRegistryId',
  'fn value_matches_kind(',
]) {
  rejectMarker(
    'capability',
    forbidden,
    `capability validation must not regress to ${forbidden}`,
  );
}
requireMarkers('capabilityGate', [
  'pub fn evaluate_runtime_publish_gate_with_capabilities(',
  'evaluate_runtime_publish_gate(',
  'validate_interaction_capabilities(',
  'Draft editing and',
  'strict_capability_policy_blocks_publish_without_blocking_base_gate',
  'registered_capability_allows_publish',
], 'capability-aware publish gate');

if (failures.length > 0) {
  console.error('Fly interaction capability verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly interaction capabilities verified.');