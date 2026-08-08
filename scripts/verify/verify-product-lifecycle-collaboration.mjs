#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const modules = read('modules.toml');
const productManifest = read('crates/rustok-product/rustok-module.toml');
const inventoryManifest = read('crates/rustok-inventory/rustok-module.toml');
const pricingManifest = read('crates/rustok-pricing/rustok-module.toml');
const productCargo = read('crates/rustok-product/Cargo.toml');
const productRuntime = read('crates/rustok-product/src/lib.rs');
const catalog = read('crates/rustok-product/src/services/catalog.rs');
const commands = read('crates/rustok-product/src/services/catalog/commands.rs');
const inventoryBootstrap = read('crates/rustok-inventory/src/services/bootstrap.rs');
const pricingBootstrap = read('crates/rustok-pricing-persistence/src/lib.rs');
const manifestCorequisites = read('apps/server/src/modules/manifest/corequisites.rs');
const manifestManager = read('apps/server/src/modules/manifest/mod.rs');
const tenantLifecycle = read('apps/server/src/services/module_lifecycle.rs');
const effectivePolicy = read('apps/server/src/services/effective_module_policy.rs');
const ownerPolicy = read('crates/rustok-modules/src/policy.rs');
const lifecycleWriter = read('crates/rustok-modules/src/lifecycle_writer.rs');
const lifecycleExecutor = read('crates/rustok-modules/src/executor.rs');
const recovery = read('crates/rustok-modules/src/recovery.rs');
const note = read('crates/rustok-product/docs/product-lifecycle-collaboration.md');
const evidence = JSON.parse(
  read('crates/rustok-product/contracts/evidence/product-lifecycle-collaboration-source.json'),
);

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const productModuleEntry = between(modules, 'product = {', '\nprofiles = {', 'root Product module entry');
const pricingModuleEntry = between(modules, 'pricing = {', '\ninventory = {', 'root Pricing module entry');
const inventoryModuleEntry = between(modules, 'inventory = {', '\norder = {', 'root Inventory module entry');
const defaultEnabled = modules.slice(modules.indexOf('[settings]'));
const productDependencySection = between(
  productManifest,
  '[dependencies]',
  '\n[co_requisites]',
  'Product package dependency section',
);
const productCoRequisiteSection = between(
  productManifest,
  '[co_requisites]',
  '\n[provides.admin_ui]',
  'Product package co-requisite section',
);
const inventoryDependencySection = between(
  inventoryManifest,
  '[dependencies]',
  '\n[crate]',
  'Inventory package dependency section',
);
const pricingDependencySection = between(
  pricingManifest,
  '[dependencies]',
  '\n[crate]',
  'Pricing package dependency section',
);
const installBuiltinBlock = between(
  manifestManager,
  'pub fn install_builtin_module(',
  '\n    pub fn uninstall_module(',
  'builtin install block',
);
const registryValidationBlock = between(
  manifestManager,
  'pub fn validate_with_registry(',
  '\n}\n\npub fn validate_registry_vs_manifest',
  'registry validation block',
);
const startupValidationBlock = manifestManager.slice(
  manifestManager.indexOf('pub fn validate_registry_vs_manifest'),
);
const lifecycleToggleBlock = between(
  tenantLifecycle,
  'pub async fn toggle_module_with_actor(',
  '\n    pub async fn module_operation_recovery_plan(',
  'tenant lifecycle toggle block',
);

for (const [source, value, label] of [
  [productModuleEntry, 'depends_on = ["taxonomy"]', 'root Product dependency'],
  [pricingModuleEntry, 'depends_on = ["product"]', 'root Pricing dependency'],
  [inventoryModuleEntry, 'depends_on = ["product"]', 'root Inventory dependency'],
  [defaultEnabled, '"product"', 'default Product activation'],
  [defaultEnabled, '"pricing"', 'default Pricing activation'],
  [defaultEnabled, '"inventory"', 'default Inventory activation'],
  [productDependencySection, 'taxonomy = { version_req = ">=0.1.0" }', 'Product Taxonomy dependency'],
  [productCoRequisiteSection, 'inventory = { version_req = ">=0.1.0" }', 'Product Inventory co-requisite'],
  [productCoRequisiteSection, 'pricing = { version_req = ">=0.1.0" }', 'Product Pricing co-requisite'],
  [inventoryDependencySection, 'product = { version_req = ">=0.1.0" }', 'Inventory Product dependency'],
  [pricingDependencySection, 'product = { version_req = ">=0.1.0" }', 'Pricing Product dependency'],
  [productRuntime, '&["taxonomy"]', 'Product runtime dependency'],
  [productCargo, 'rustok-inventory.workspace = true', 'Inventory owner crate dependency'],
  [productCargo, 'rustok-pricing-persistence.workspace = true', 'Pricing persistence dependency'],
  [catalog, 'use rustok_inventory::{BootstrapService, InitialInventory};', 'Inventory bootstrap import'],
  [catalog, 'use rustok_pricing_persistence::{BootstrapService as PricingBootstrapService, InitialPrice};', 'Pricing bootstrap import'],
  [commands, 'BootstrapService::ensure_default_location_in_tx(&txn, tenant_id)', 'default inventory location collaboration'],
  [commands, 'BootstrapService::create_initial_records_in_tx(', 'initial inventory collaboration'],
  [commands, 'PricingBootstrapService::create_initial_prices_in_tx(&txn, initial_prices)', 'initial price collaboration'],
  [commands, 'BootstrapService::delete_records_for_variants_in_tx(&txn, &variant_ids)', 'inventory cleanup collaboration'],
  [commands, 'PricingBootstrapService::delete_prices_for_variants_in_tx(&txn, &variant_ids)', 'price cleanup collaboration'],
  [inventoryBootstrap, 'Owner-owned, transaction-aware inventory bootstrap operations.', 'Inventory owner contract'],
  [inventoryBootstrap, 'C: ConnectionTrait', 'Inventory transaction connection'],
  [pricingBootstrap, 'Pricing-owned transaction-aware operations required by Product lifecycle writes.', 'Pricing owner contract'],
  [pricingBootstrap, 'C: ConnectionTrait', 'Pricing transaction connection'],

  [manifestCorequisites, 'co_requisites: BTreeMap<String, PackageCoRequisiteSpec>', 'typed package co-requisite parser'],
  [manifestCorequisites, 'pub(crate) fn module_policy_corequisites', 'owner co-requisite input entrypoint'],
  [manifestCorequisites, 'pub(super) fn validate_default_corequisite_selection', 'deployment selection preflight'],
  [manifestCorequisites, 'module.ordinary_dependencies.contains(co_requisite)', 'dependency/co-requisite separation'],
  [manifestCorequisites, '!selected.contains(co_requisite)', 'missing selected co-requisite rejection'],
  [manifestCorequisites, 'VersionReq::parse(requirement)', 'deployment co-requisite version validation'],
  [manifestCorequisites, 'requirement.matches(&installed_version)', 'deployment selected version validation'],
  [manifestManager, 'pub fn validate_deployment_selection(', 'manifest deployment selection API'],
  [startupValidationBlock, '.and_then(|_| ManifestManager::validate_deployment_selection(&manifest))', 'startup co-requisite preflight'],
  [registryValidationBlock, 'dependencies: resolved_spec.depends_on.iter().cloned().collect()', 'ordinary registry dependency source'],
  [installBuiltinBlock, 'Self::validate(manifest)?;', 'ordinary install validation'],

  [ownerPolicy, 'pub struct ModuleEffectivePolicyCoRequisite', 'owner co-requisite input'],
  [ownerPolicy, 'CoRequisite {', 'owner co-requisite decision fact'],
  [ownerPolicy, 'CoRequisiteUnavailable { module_slug: String }', 'owner unavailable denial'],
  [ownerPolicy, 'CoRequisiteVersionMismatch { module_slug: String }', 'owner version denial'],
  [ownerPolicy, 'co_requisites: Vec<ModuleEffectivePolicyCoRequisite>', 'owner query co-requisite state'],
  [ownerPolicy, 'co_requisites: &\'a [ModuleEffectivePolicyCoRequisite]', 'revisioned co-requisite input'],
  [ownerPolicy, 'contract: "rustok.module_effective_policy.v2"', 'effective policy v2 identity'],
  [ownerPolicy, 'normalize_corequisites(self.catalog, self.co_requisites)', 'owner co-requisite normalization'],
  [ownerPolicy, 'corequisite_version_compatible(self.catalog, co_requisite)', 'owner version decision'],
  [ownerPolicy, 'corequisites_are_revisioned_explainable_availability_not_dependency_edges', 'owner policy source test'],

  [lifecycleWriter, 'pub fn with_corequisites(', 'lifecycle writer co-requisite input'],
  [lifecycleWriter, '.with_corequisites(self.co_requisites.iter().cloned())', 'canonical lifecycle policy co-requisites'],
  [lifecycleWriter, 'ordering_policy_from_overrides', 'ordinary ordering projection'],
  [lifecycleWriter, 'ordering_policy.into_enabled_modules()', 'ordinary ordering selection output'],
  [lifecycleExecutor, 'pub ordering_enabled_modules: HashSet<String>', 'lifecycle ordering input'],
  [lifecycleExecutor, '&request.ordering_enabled_modules', 'ordinary toggle validation input'],
  [recovery, 'previous_effective_enabled', 'legacy recovery predecessor debt'],

  [lifecycleToggleBlock, 'ManifestManager::module_policy_corequisites(&manifest)', 'lifecycle package contract load'],
  [lifecycleToggleBlock, '.with_corequisites(co_requisites)', 'lifecycle owner policy binding'],
  [effectivePolicy, 'ManifestManager::module_policy_corequisites(&manifest)', 'effective-policy package contract load'],
  [effectivePolicy, '.with_corequisites(co_requisites)', 'effective-policy owner binding'],
  [effectivePolicy, '.cache_identity(tenant_id)', 'canonical cache identity'],

  [note, 'canonical owner effective-policy co-requisite identity source-complete', 'focused note status'],
  [note, 'rustok.module_effective_policy.v2', 'focused note revision identity'],
  [note, '**ordinary ordering selection**', 'focused note ordering split'],
  [note, 'previous_effective_enabled', 'focused note staged recovery debt'],
]) requireText(source, value, label);

for (const [source, value, label] of [
  [productModuleEntry, '"inventory"', 'cyclic Product-to-Inventory root dependency'],
  [productModuleEntry, '"pricing"', 'cyclic Product-to-Pricing root dependency'],
  [productDependencySection, 'inventory =', 'cyclic Product package Inventory dependency'],
  [productDependencySection, 'pricing =', 'cyclic Product package Pricing dependency'],
  [productRuntime, '"inventory"', 'cyclic Product runtime Inventory dependency'],
  [productRuntime, '"pricing"', 'cyclic Product runtime Pricing dependency'],
  [installBuiltinBlock, 'validate_deployment_selection', 'cyclic install-time co-requisite preflight'],
  [registryValidationBlock, 'co_requisite', 'co-requisite leaked into registry dependency comparison'],
  [manifestCorequisites, 'validate_effective_policy_corequisites', 'duplicate host effective-policy guard'],
  [manifestCorequisites, 'validate_corequisite_toggle', 'duplicate host lifecycle guard'],
  [tenantLifecycle, 'validate_corequisite_toggle', 'server lifecycle duplicate guard'],
  [effectivePolicy, 'validate_effective_policy_corequisites', 'server effective-policy duplicate guard'],
  [catalog, 'rustok_commerce_foundation::entities', 'foreign foundation entity import'],
  [commands, 'rustok_commerce_foundation::entities', 'foreign foundation entity import in commands'],
  [commands, 'inventory_item::Entity', 'direct Inventory entity access'],
  [commands, 'inventory_level::Entity', 'direct Inventory level access'],
  [commands, 'stock_location::Entity', 'direct stock location access'],
  [commands, 'reservation_item::Entity', 'direct reservation access'],
  [commands, 'price::Entity', 'direct Pricing entity access'],
  [commands, 'FROM inventory_', 'direct Inventory SQL'],
  [commands, 'INTO inventory_', 'direct Inventory SQL write'],
  [commands, 'FROM prices', 'direct Pricing SQL'],
  [commands, 'INTO prices', 'direct Pricing SQL write'],
]) forbidText(source, value, label);

if (evidence.status !== 'product_lifecycle_owner_corequisite_policy_identity_source_complete_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
const coRequisites = [...(evidence.module_topology?.product_co_requisites ?? [])].sort();
if (JSON.stringify(coRequisites) !== JSON.stringify(['inventory', 'pricing'])) {
  failures.push(`evidence Product co-requisites mismatch: ${JSON.stringify(coRequisites)}`);
}
for (const [key, expected] of [
  ['ordinary_reverse_dependencies_forbidden', true],
  ['co_requisites_are_ordering_dependencies', false],
  ['static_default_selection_enforcement_source_complete', true],
  ['tenant_lifecycle_corequisite_guard_source_complete', true],
  ['effective_policy_corequisite_guard_source_complete', true],
  ['canonical_policy_revision_corequisite_identity_source_complete', true],
  ['owner_corequisite_decision_explanation_source_complete', true],
  ['lifecycle_ordering_separated_from_corequisite_availability_source_complete', true],
  ['staged_intent_recovery_semantics_source_complete', false],
  ['co_requisite_control_plane_execution_proven', false],
  ['tenant_lifecycle_corequisite_execution_proven', false],
  ['standalone_product_write_activation_proven', false],
]) {
  if (evidence.module_topology?.[key] !== expected) {
    failures.push(`evidence module_topology.${key} must be ${expected}`);
  }
}
if (evidence.product_boundary?.foreign_owner_entities_imported !== false ||
    evidence.product_boundary?.foreign_owner_tables_queried_directly !== false ||
    evidence.product_boundary?.owner_bootstrap_services_only !== true) {
  failures.push('evidence Product owner boundary mismatch');
}
for (const key of [
  'static_default_selection_enforcement_source_complete',
  'tenant_lifecycle_corequisite_guard_source_complete',
  'effective_policy_corequisite_guard_source_complete',
  'canonical_policy_revision_corequisite_identity_source_complete',
  'owner_corequisite_decision_explanation_source_complete',
  'lifecycle_ordering_separated_from_corequisite_availability_source_complete',
]) {
  if (evidence.decision?.[key] !== true) failures.push(`evidence decision.${key} must be true`);
}
if (evidence.decision?.containment_complete !== true ||
    evidence.decision?.corequisite_contract_declared !== true ||
    evidence.decision?.dependency_contract_resolved !== false) {
  failures.push('evidence decision must close owner policy identity while keeping staged recovery open');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'static_default_selection_execution_proven',
  'tenant_lifecycle_execution_proven',
  'effective_policy_guard_execution_proven',
  'policy_cache_identity_execution_proven',
  'staged_intent_recovery_execution_proven',
  'standalone_activation_proven',
  'non_default_deployment_selection_proven',
  'transaction_rollback_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error('Product lifecycle collaboration verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Product co-requisites are canonical owner effective-policy identity/explanation inputs and remain separate from ordinary lifecycle ordering; staged recovery and execution evidence remain open',
);
