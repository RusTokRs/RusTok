import { readFile } from 'node:fs/promises';

const paths = {
  flyUiCargo: 'crates/fly-ui/Cargo.toml',
  flyUiLib: 'crates/fly-ui/src/lib.rs',
  error: 'crates/fly-ui/src/error.rs',
  contribution: 'crates/fly-ui/src/contribution.rs',
  adapter: 'crates/fly-ui/src/contribution_adapter.rs',
  factory: 'crates/fly-ui/src/contribution_factory.rs',
  manifestFacade: 'crates/fly-ui/src/contribution_manifest.rs',
  manifestModel: 'crates/fly-ui/src/contribution_manifest/model.rs',
  manifestAssembly: 'crates/fly-ui/src/contribution_manifest/assemble.rs',
  manifestTests: 'crates/fly-ui/src/contribution_manifest/tests.rs',
  paletteAccess: 'crates/fly-ui/src/palette_access.rs',
  pageBuilderLib: 'crates/rustok-page-builder/admin/src/lib.rs',
  pageBuilderHost: 'crates/rustok-page-builder/admin/src/ui/leptos.rs',
  pageBuilderCanvas: 'crates/rustok-page-builder/admin/src/editor/modular_canvas.rs',
  pageBuilderPalette: 'crates/rustok-page-builder/admin/src/editor/palette_layers.rs',
  pageBuilderPaletteAccess: 'crates/rustok-page-builder/admin/src/palette_access.rs',
  pageBuilderContextContract: 'crates/rustok-page-builder/admin/src/context_contract.rs',
  pagesLib: 'crates/rustok-pages/admin/src/lib.rs',
  pagesContributions: 'crates/rustok-pages/admin/src/contributions.rs',
  pagesContributionBrowser: 'crates/rustok-pages/admin/src/contribution_browser_intent.rs',
  pagesComposition: 'crates/rustok-pages/admin/src/composition.rs',
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
  'mod contribution_adapter;',
  'mod contribution_factory;',
  'mod contribution_manifest;',
  'mod palette_access;',
  'pub use contribution::*;',
  'pub use contribution_adapter::*;',
  'pub use contribution_factory::*;',
  'pub use contribution_manifest::*;',
  'pub use palette_access::*;',
], 'fly-ui contribution module wiring');
requireMarkers('error', [
  'InvalidContribution',
  'DuplicateRenderer(String)',
  'DuplicatePropertyEditor(String)',
  'RendererUnavailable(String)',
  'PropertyEditorUnavailable(String)',
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
requireMarkers('adapter', [
  'pub trait ContributionAdapter',
  'pub struct RendererRequest',
  'pub struct PropertyEditorRequest',
  'pub fn render_contribution',
  'pub fn edit_contribution_properties',
  'one_mock_adapter_renders_all_presentations',
  'property_editor_is_available_only_in_editable_presentations',
  'missing_capability_returns_typed_lookup_error',
], 'framework-neutral contribution adapter');
rejectMarker(
  'adapter',
  'let request = |presentation|',
  'mock adapter proof must not return borrowed requests from a capturing closure',
);
requireMarkers('factory', [
  'pub enum ContributionSurface',
  'pub enum ContributionProviderHealth',
  'pub struct ModuleContributionMetadata',
  'pub struct ContributionAssemblyPolicy',
  'pub struct ContributionAssemblyDiagnostic',
  'pub struct ContributionAssemblyResult',
  'pub fn build_admin_contribution_registry(',
  'pub fn build_storefront_contribution_registry(',
  'pub fn assemble_contribution_registry(',
  'fn remove_missing_dependencies(',
  'fn dependency_order(',
  'contribution_dependency_missing',
  'contribution_dependency_cycle',
  'contribution_provider_unavailable',
  'contribution_permission_missing',
  'contribution_capability_missing',
  'admin_and_storefront_factories_are_separate',
  'assembly_filters_tenant_permissions_capabilities_and_health',
  'missing_dependencies_and_cycles_are_diagnosed',
  'duplicate_nested_contracts_are_reported_without_partial_registration',
], 'legacy owner-equals-target contribution factories');
requireMarker(
  'factory',
  'let code = match &error',
  'factory diagnostics must not move UiError before formatting it',
);
requireMarkers('manifestFacade', [
  'mod assemble;',
  'mod model;',
  'pub use assemble::*;',
  'pub use model::*;',
  '#[cfg(test)]\nmod tests;',
], 'contribution manifest facade');
requireMarkers('manifestModel', [
  'pub struct ModuleContributionManifest',
  'pub owner_provider: String',
  'pub owner_version: String',
  'pub target_providers: BTreeMap<String, String>',
  'pub fn allows_target_provider',
  'self.owner_provider.trim()',
  'self.owner_version.trim()',
], 'owner-safe manifest model');
requireMarkers('manifestAssembly', [
  'pub fn build_admin_contribution_registry_from_manifests(',
  'pub fn build_storefront_contribution_registry_from_manifests(',
  'pub fn assemble_contribution_manifests(',
  'fn discover_manifests(',
  'fn filter_manifests(',
  'fn register_surface_contributions(',
  'fn remove_missing_dependencies(',
  'fn dependency_order(',
  'fn provider_is_enabled(',
  'contribution_target_provider_forbidden',
  'contribution_target_provider_disabled',
  'contribution_target_provider_unavailable',
  'contribution_dependency_missing',
  'contribution_dependency_cycle',
], 'manifest assembly engine');
requireMarkers('manifestTests', [
  'owner_provider_is_the_only_implicit_target',
  'explicit_versioned_target_provider_is_allowed',
  'target_provider_version_mismatch_is_rejected',
  'target_provider_must_be_tenant_enabled',
  'owner_and_target_provider_allowlist_enables_cross_provider_extension',
  'admin_and_storefront_surfaces_remain_separate',
  'target_provider_health_can_block_cross_provider_extensions',
  'direct_target_lookup_trims_owner_and_versions',
], 'manifest assembly regression coverage');
requireMarkers('paletteAccess', [
  'pub struct PaletteBlockAccess',
  'pub fn unrestricted()',
  'pub fn from_assembly(',
  'pub fn from_optional_assembly(',
  'pub fn allows(',
  '!is_namespaced_block(block_id)',
  'legacy_surface_is_unrestricted',
  'assembled_surface_keeps_primitives_and_filters_namespaced_blocks',
  'block_provenance_is_deterministic',
], 'contribution-aware palette access');
requireMarkers('pageBuilderHost', [
  'pub contribution_assembly: Option<Arc<ContributionAssemblyResult>>',
  'pub fn with_contribution_assembly(',
  'contribution_assembly=context.contribution_assembly',
  'contribution_assembly: Option<Arc<ContributionAssemblyResult>>',
], 'Page Builder host contribution boundary');
requireMarkers('pageBuilderCanvas', [
  'contribution_assembly: Option<Arc<ContributionAssemblyResult>>',
  '<PaletteLayersPanel',
  'contribution_assembly',
], 'Page Builder canvas contribution routing');
requireMarkers('pageBuilderPalette', [
  'PaletteBlockAccess::from_optional_assembly(',
  'data-fly-contribution-registry="true"',
  'data-fly-contribution-ids=contribution_attr',
  'controller.palette_blocks_with_access(&access)',
  'begin_palette_drag_intent_with_access(',
  'insert_palette_block_intent_with_access(',
  'contribution_access_filters_templates_without_copying_block_definitions',
], 'Page Builder palette contribution enforcement');
requireMarkers('pageBuilderPaletteAccess', [
  'pub fn palette_blocks_with_access(',
  'pub fn palette_block_with_access(',
  'pub fn begin_palette_drag_intent_with_access(',
  'pub fn insert_palette_block_intent_with_access(',
  'pub fn dispatch_browser_intent_with_palette_access(',
  'pub fn validate_browser_palette_access(',
  '"insert_block" | "begin_palette_drag"',
  '"drop" => block_drop_id',
  'browser_insert_and_drop_cannot_bypass_contribution_filtering',
], 'Page Builder palette intent preflight');
requireMarkers('pageBuilderLib', [
  'mod palette_access;',
  'dispatch_browser_intent_with_palette_access',
  'validate_browser_palette_access',
], 'Page Builder palette access exports');
requireMarker(
  'pageBuilderContextContract',
  'assert_send_sync::<Arc<ContributionAssemblyResult>>()',
  'contribution assembly must remain safe for Leptos owner context',
);
requireMarkers('pagesContributions', [
  'pub const PAGES_BUILDER_CAPABILITIES',
  'pub const PAGES_LANDING_BLOCK_CAPABILITIES',
  'pub fn pages_contribution_manifest()',
  'pub fn pages_landing_blocks_contribution()',
  'pub fn pages_admin_contribution_policy()',
  'pub fn build_pages_admin_contribution_registry(',
  'FLY_BUILTIN_PROVIDER',
  'FLY_BUILTIN_PROVIDER_VERSION',
  'PAGES_LANDING_BLOCK_IDS',
  'PAGES_OWNER_PROVIDER.to_string(),',
  'FLY_BUILTIN_PROVIDER.to_string(),',
  '"preview"',
  '"tree"',
  '"properties"',
  '"publish"',
  '"fly.hero"',
  '"fly.two_columns"',
  '"fly.feature_grid"',
  '"fly.cta"',
  '"fly.contact_form"',
  'renderers: Vec::new()',
  'property_editors: Vec::new()',
  'contributed_block_ids_exist_in_the_fly_registry',
  'contribution_policy_enables_owner_and_target_providers',
  'capability_constants_match_the_module_manifest',
  'storefront_surface_stays_empty_until_a_real_adapter_exists',
], 'Pages Fly contribution manifest and policy');
requireMarkers('pagesContributionBrowser', [
  'pub fn pages_palette_block_access()',
  'pub async fn dispatch_pages_browser_intent(',
  'pub async fn dispatch_pages_browser_intent_with_capabilities(',
  'pub async fn dispatch_pages_browser_intent_with_store(',
  'pub async fn dispatch_pages_browser_intent_with_store_and_capabilities(',
  'fn preflight_pages_intent(',
  'validate_browser_palette_access(&envelope, &pages_palette_block_access())',
  'validate_browser_capability_access(&envelope, capabilities)',
  'pages_preflight_allows_primitives_and_declared_templates',
  'pages_preflight_rejects_uncontributed_namespaced_templates',
  'pages_preflight_rejects_block_drop_bypass',
  'pages_preflight_rejects_capability_bypass',
], 'Pages contribution-aware browser dispatcher');
requireMarkers('pagesLib', [
  'mod contribution_browser_intent;',
  'pub use contribution_browser_intent::{',
  'dispatch_pages_browser_intent_with_capabilities,',
  'dispatch_pages_browser_intent_with_store_and_capabilities,',
  'pages_palette_block_access,',
], 'Pages crate-root browser preflight routing');
rejectMarker(
  'pagesLib',
  'pub use browser_intent::{\n    dispatch_pages_browser_intent',
  'Pages must not export the private browser dispatcher without contribution preflight',
);
requireMarkers('pagesComposition', [
  'build_pages_admin_contribution_registry(',
  'pages_admin_contribution_policy()',
  '.with_contribution_assembly(contribution_assembly)',
], 'Pages generated contribution composition');
rejectMarker(
  'pagesComposition',
  'fn pages_contribution_policy()',
  'Pages composition must consume the centralized contribution policy',
);
for (const forbidden of [
  'leptos',
  'dioxus',
  'web_sys',
  'wasm_bindgen',
  'rustok_',
  'rustok-',
]) {
  for (const key of [
    'contribution',
    'adapter',
    'factory',
    'manifestFacade',
    'manifestModel',
    'manifestAssembly',
    'manifestTests',
    'paletteAccess',
    'flyUiCargo',
  ]) {
    rejectMarker(
      key,
      forbidden,
      `fly-ui contribution infrastructure must remain framework/RusTok neutral: ${forbidden}`,
    );
  }
}
requireMarker(
  'tests',
  'contribution_filtering_is_capability_driven',
  'legacy capability filtering regression coverage is missing',
);

if (failures.length > 0) {
  console.error('Fly UI contributions failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Fly UI contributions verified.');
