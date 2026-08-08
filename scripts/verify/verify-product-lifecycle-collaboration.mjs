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

const productModuleEntry = between(
  modules,
  'product = {',
  '\nprofiles = {',
  'root Product module entry',
);
const pricingModuleEntry = between(
  modules,
  'pricing = {',
  '\ninventory = {',
  'root Pricing module entry',
);
const inventoryModuleEntry = between(
  modules,
  'inventory = {',
  '\norder = {',
  'root Inventory module entry',
);
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
  [note, 'Status: deployment co-requisite contract declared, control-plane enforcement pending, unvalidated.', 'focused note status'],
  [note, 'deployment-selection constraint, not an ordinary dependency edge', 'co-requisite semantics'],
  [note, 'current module control plane does not yet consume `[co_requisites]`', 'explicit enforcement debt'],
]) requireText(source, value, label);

for (const [source, value, label] of [
  [productModuleEntry, '"inventory"', 'cyclic Product-to-Inventory root dependency'],
  [productModuleEntry, '"pricing"', 'cyclic Product-to-Pricing root dependency'],
  [productDependencySection, 'inventory =', 'cyclic Product package Inventory dependency'],
  [productDependencySection, 'pricing =', 'cyclic Product package Pricing dependency'],
  [productRuntime, '"inventory"', 'cyclic Product runtime Inventory dependency'],
  [productRuntime, '"pricing"', 'cyclic Product runtime Pricing dependency'],
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

if (evidence.status !== 'product_lifecycle_corequisites_declared_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
const coRequisites = [...(evidence.module_topology?.product_co_requisites ?? [])].sort();
if (JSON.stringify(coRequisites) !== JSON.stringify(['inventory', 'pricing'])) {
  failures.push(`evidence Product co-requisites mismatch: ${JSON.stringify(coRequisites)}`);
}
if (evidence.module_topology?.ordinary_reverse_dependencies_forbidden !== true) {
  failures.push('evidence must forbid ordinary reverse dependencies');
}
if (evidence.module_topology?.co_requisites_are_ordering_dependencies !== false) {
  failures.push('co-requisites must not become ordering dependencies');
}
if (evidence.module_topology?.co_requisite_control_plane_enforcement_proven !== false) {
  failures.push('co-requisite control-plane enforcement must remain unproven');
}
if (evidence.module_topology?.standalone_product_write_activation_proven !== false) {
  failures.push('standalone Product write activation must remain unproven');
}
if (evidence.product_boundary?.foreign_owner_entities_imported !== false ||
    evidence.product_boundary?.foreign_owner_tables_queried_directly !== false ||
    evidence.product_boundary?.owner_bootstrap_services_only !== true) {
  failures.push('evidence Product owner boundary mismatch');
}
if (evidence.decision?.containment_complete !== true ||
    evidence.decision?.corequisite_contract_declared !== true ||
    evidence.decision?.dependency_contract_resolved !== false) {
  failures.push('evidence decision must declare co-requisites while keeping enforcement open');
}
for (const key of [
  'tests_run',
  'cargo_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
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
  '✔ Product declares Inventory/Pricing deployment co-requisites while preserving owner bootstrap atomicity and acyclic ordinary dependencies; control-plane enforcement and runtime evidence remain open',
);
