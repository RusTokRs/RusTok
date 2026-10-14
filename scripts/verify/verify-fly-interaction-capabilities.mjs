import { readFile } from 'node:fs/promises';

const paths = {
  flyLib: 'crates/ui/fly/src/lib.rs',
  flyError: 'crates/ui/fly/src/error.rs',
  capability: 'crates/ui/fly/src/interaction_capability.rs',
  capabilityGate: 'crates/modules/rustok-page-builder/src/domain/interaction_capability_gate.rs',
  actionModel: 'crates/ui/fly/src/action/model.rs',
  actionMaterialize: 'crates/ui/fly/src/action/materialize.rs',
  runtimeRender: 'crates/ui/fly/src/runtime_render.rs',
  runtimePipeline: 'crates/ui/fly/src/runtime_pipeline.rs',
  componentVisit: 'crates/ui/fly/src/component_visit.rs',
  hostCatalog: 'crates/modules/rustok-page-builder/src/interaction_providers.rs',
  hostLib: 'crates/modules/rustok-page-builder/src/lib.rs',
  hostRuntimeContext: 'crates/modules/rustok-page-builder/src/runtime_context.rs',
  hostRender: 'crates/modules/rustok-page-builder/src/render.rs',
  hostStorefrontRoute: 'crates/modules/rustok-page-builder-storefront/src/localized_route.rs',
  adminSession: 'crates/modules/rustok-page-builder/admin/src/core/session.rs',
  adminRuntime: 'crates/modules/rustok-page-builder/admin/src/editor/runtime.rs',
  formsControllers: 'crates/modules/rustok-forms/src/controllers.rs',
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
], 'interaction capability module wiring');

// Assert the re-exported symbols rather than literal `pub use` lines: the fly root uses
// curated explicit re-exports, so pin the exported capability surface, not its spelling.
// The capability-aware publish gate itself is Page Builder domain since the extraction
// (DECISIONS/2026-10-14-landing-scenario-domain-leaves-fly-core.md), so its symbol is
// pinned on the `rustok_page_builder` root instead.
{
  const exportedFrom = (text, module) => {
    const braced = text.match(new RegExp(`pub use ${module}::\\{([^}]*)\\}`));
    if (braced) return braced[1].split(',').map((entry) => entry.trim()).filter(Boolean);
    const single = text.match(new RegExp(`pub use ${module}::([A-Za-z0-9_]+);`));
    return single ? [single[1]] : null;
  };
  const expected = [
    [source.flyLib, 'Fly lib', 'interaction_capability', [
      'InteractionCapabilityDefinition',
      'InteractionCapabilityKind',
      'InteractionCapabilityPolicy',
      'InteractionCapabilityRegistry',
      'InteractionRuntimeTarget',
      'MissingInteractionCapabilityPolicy',
      'validate_component_actions_with_capabilities',
      'validate_interaction_capabilities',
    ]],
    [source.hostLib, 'Page-builder lib', 'domain', [
      'evaluate_runtime_publish_gate_with_capabilities',
    ]],
  ];
  for (const [text, label, module, symbols] of expected) {
    const exported = exportedFrom(text, module);
    if (!exported) {
      failures.push(`${label} does not re-export anything from ${module}`);
      continue;
    }
    for (const symbol of symbols) {
      if (!exported.includes(symbol)) {
        failures.push(`${label} does not re-export ${module}::${symbol}`);
      }
    }
  }
}
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
requireMarkers('actionModel', [
  'pub trait FormProviderEndpointResolver',
  'fn form_endpoint(&self, provider: &str, operation: &str, form_id: &str) -> Option<String>;',
], 'host provider endpoint resolver contract');
requireMarkers('actionMaterialize', [
  'pub fn materialize_component_actions_with_providers(',
  'providers?.form_endpoint(provider, operation, form.id.trim())',
], 'provider-aware action materialization');
requireMarkers('runtimeRender', [
  'pub fn render_page_with_runtime_context_with_providers(',
  'materialize_project_with_runtime_context_with_providers(',
], 'provider-aware runtime render');
requireMarkers('runtimePipeline', [
  'pub fn materialize_project_with_runtime_context_with_providers(',
], 'provider-aware runtime materialization');
requireMarkers('hostCatalog', [
  'pub const FORMS_PROVIDER_ID: &str = "forms";',
  'pub const FORMS_SUBMIT_OPERATION: &str = "submit";',
  'pub const FORMS_SUBMIT_ENDPOINT_TEMPLATE: &str = "/api/forms/{form_id}/submit/form";',
  'impl FormProviderEndpointResolver for HostFormProviderEndpoints',
  'pub fn host_interaction_capability_registry()',
  'InteractionCapabilityKind::Form',
  'MissingInteractionCapabilityPolicy::Error',
  'MissingInteractionCapabilityPolicy::Warn',
  'evaluate_runtime_publish_gate_with_capabilities(',
], 'host provider capability catalog');
requireMarkers('hostLib', [
  'pub mod interaction_providers;',
], 'host provider capability module');
requireMarkers('hostRuntimeContext', [
  'evaluate_host_runtime_publish_gate(',
  'evaluate_host_runtime_publish_gate_with_policy(',
  'pub interaction_policy: Option<InteractionCapabilityPolicy>,',
], 'host publish gate capability wiring');
requireMarkers('hostRender', [
  'render_page_with_runtime_context_with_providers(',
  '&HostFormProviderEndpoints',
], 'host preview render provider wiring');
requireMarkers('hostStorefrontRoute', [
  'render_page_with_runtime_context_with_providers(',
  '&HostFormProviderEndpoints',
], 'storefront provider form endpoint binding');
requireMarkers('adminSession', [
  'evaluate_host_runtime_publish_gate(',
], 'admin session publish gate capability wiring');
requireMarkers('adminRuntime', [
  'rustok_page_builder::evaluate_host_runtime_publish_gate(',
], 'admin editor publish gate capability wiring');

// The endpoint template in the page builder must stay byte-identical to the route the forms
// module serves; the two are only coupled through this check, not through Cargo.
const routeLiteral = /pub const SUBMIT_URLENCODED_ROUTE: &str = "([^"]+)";/;
const formsRoute = source.formsControllers.match(routeLiteral);
if (!formsRoute) {
  failures.push('rustok-forms does not declare SUBMIT_URLENCODED_ROUTE');
} else if (!source.hostCatalog.includes(`"${formsRoute[1]}"`)) {
  failures.push(
    `host provider endpoint template drifted from the rustok-forms route ${formsRoute[1]}`,
  );
}

if (failures.length > 0) {
  console.error('Fly interaction capability verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly interaction capabilities verified.');