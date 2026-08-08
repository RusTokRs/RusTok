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
const recoveryMigration = read(
  'crates/rustok-migrations/src/m20260808_000099_create_module_operation_override_states.rs',
);
const platformMigrator = read('crates/rustok-migrations/src/lib.rs');
const runbook = read('apps/server/docs/module-lifecycle-retry-compensation-runbook.md');
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
const compensationBlock = between(
  lifecycleWriter,
  'pub async fn compensate_failed_operation(',
  '\n    /// Persists a static module settings value',
  'owner compensation block',
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
  [pricingBootstrap, 'Pricing-owned transaction-aware operations required by Product lifecycle writes.', 'Pricing owner contract'],

  [manifestCorequisites, 'pub(crate) fn module_policy_corequisites', 'owner co-requisite input entrypoint'],
  [manifestCorequisites, 'pub(super) fn validate_default_corequisite_selection', 'deployment selection preflight'],
  [manifestCorequisites, 'module.ordinary_dependencies.contains(co_requisite)', 'dependency/co-requisite separation'],
  [manifestManager, 'pub fn validate_deployment_selection(', 'manifest deployment selection API'],
  [startupValidationBlock, '.and_then(|_| ManifestManager::validate_deployment_selection(&manifest))', 'startup co-requisite preflight'],
  [registryValidationBlock, 'dependencies: resolved_spec.depends_on.iter().cloned().collect()', 'ordinary registry dependency source'],
  [installBuiltinBlock, 'Self::validate(manifest)?;', 'ordinary install validation'],

  [ownerPolicy, 'pub struct ModuleEffectivePolicyCoRequisite', 'owner co-requisite input'],
  [ownerPolicy, 'CoRequisiteUnavailable { module_slug: String }', 'owner unavailable denial'],
  [ownerPolicy, 'CoRequisiteVersionMismatch { module_slug: String }', 'owner version denial'],
  [ownerPolicy, 'contract: "rustok.module_effective_policy.v2"', 'effective policy v2 identity'],
  [ownerPolicy, 'normalize_corequisites(self.catalog, self.co_requisites)', 'owner co-requisite normalization'],
  [lifecycleWriter, 'ordering_policy_from_overrides', 'ordinary ordering projection'],
  [lifecycleExecutor, '&request.ordering_enabled_modules', 'ordinary toggle validation input'],
  [effectivePolicy, '.with_corequisites(co_requisites)', 'effective-policy owner binding'],
  [effectivePolicy, '.cache_identity(tenant_id)', 'canonical cache identity'],

  [recovery, 'pub override_state_recorded: bool', 'recorded selected-intent marker'],
  [recovery, 'pub previous_override_enabled: Option<bool>', 'nullable override predecessor'],
  [recovery, 'pub requested_override_enabled: Option<bool>', 'nullable override target'],
  [recovery, 'FROM module_operation_override_states', 'selected-intent recovery read'],
  [recovery, 'INSERT INTO module_operation_override_states', 'selected-intent recovery write'],
  [recovery, 'selected_intent_state_unavailable', 'legacy recovery fail-closed reason'],
  [recovery, 'request.current_override_enabled != plan.requested_override_enabled', 'retry exact override state match'],
  [recovery, 'plan.previous_override_enabled,\n                plan.requested_override_enabled', 'retry predecessor retention'],
  [recovery, 'DELETE FROM tenant_modules', 'inherited override restoration'],
  [lifecycleExecutor, 'pub previous_override_enabled: Option<bool>', 'executor exact override predecessor'],
  [lifecycleExecutor, 'pub requested_override_enabled: Option<bool>', 'executor exact override target'],
  [lifecycleExecutor, 'record_operation_override_state(', 'executor recovery-state retention'],
  [lifecycleExecutor, 'apply_tenant_override_enabled(', 'executor tri-state persistence'],
  [lifecycleWriter, 'Some(enabled),', 'normal toggle explicit override target'],
  [lifecycleWriter, 'None => next_overrides.retain(|value| value.module_slug != module_slug)', 'policy inherited override projection'],
  [compensationBlock, 'let reverse_enabled = !plan.requested_enabled;', 'inverse compensation lifecycle direction'],
  [compensationBlock, 'plan.previous_override_enabled', 'exact compensation target'],
  [compensationBlock, 'current_override_enabled != plan.requested_override_enabled', 'compensation exact current-state match'],
  [tenantLifecycle, 'explicit module toggle did not persist a tenant override row', 'normal explicit-row server assertion'],
  [tenantLifecycle, '(state.enabled, state.settings)', 'explicit compensation response state'],
  [tenantLifecycle, 'None => (policy.contains(&plan.module_slug), serde_json::json!({}))', 'inherited compensation availability fallback'],

  [recoveryMigration, 'module_operation_override_states', 'recovery side-table migration'],
  [recoveryMigration, 'PreviousOverrideEnabled', 'nullable predecessor migration column'],
  [recoveryMigration, 'RequestedOverrideEnabled', 'nullable target migration column'],
  [recoveryMigration, 'ForeignKeyAction::Cascade', 'recovery side-table operation ownership'],
  [platformMigrator, 'mod m20260808_000099_create_module_operation_override_states;', 'migration module registration'],
  [platformMigrator, '"m20260808_000099_create_module_operation_override_states",', 'append-only migration tail registration'],
  [platformMigrator, 'Box::new(m20260808_000099_create_module_operation_override_states::Migration)', 'migration execution registration'],

  [runbook, '`inherit`', 'operational inherited-state contract'],
  [runbook, 'selected_intent_state_unavailable', 'operational legacy fail-closed contract'],
  [runbook, 'Do **not** use `previous_effective_enabled` as the compensation target.', 'operational availability/predecessor separation'],
  [note, 'staged lifecycle recovery source-complete', 'focused note status'],
  [note, '`module_operation_override_states`', 'focused recovery evidence'],
  [note, '`previous_effective_enabled` remains in the journal as historical availability evidence', 'focused legacy fact role'],
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
  [compensationBlock, 'plan.previous_effective_enabled,', 'effective availability reused as compensation target'],
  [catalog, 'rustok_commerce_foundation::entities', 'foreign foundation entity import'],
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

if (evidence.status !== 'product_lifecycle_staged_recovery_source_complete_unvalidated') {
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
  ['staged_intent_recovery_semantics_source_complete', true],
  ['inherited_override_recovery_source_complete', true],
  ['legacy_recovery_without_selected_state_fails_closed', true],
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
  'staged_intent_recovery_semantics_source_complete',
]) {
  if (evidence.decision?.[key] !== true) failures.push(`evidence decision.${key} must be true`);
}
if (evidence.decision?.containment_complete !== true ||
    evidence.decision?.corequisite_contract_declared !== true ||
    evidence.decision?.dependency_contract_resolved !== true) {
  failures.push('evidence decision must mark the Product dependency/collaboration source contract resolved');
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
  'migration_execution_proven',
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
  '✔ Product co-requisite policy identity, ordinary ordering separation, and exact staged tenant-intent recovery are source-locked; maintainer execution evidence remains open',
);
