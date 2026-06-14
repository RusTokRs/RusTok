import { readFileSync } from 'node:fs';

const modules = ['payment', 'fulfillment', 'order', 'pricing', 'inventory'];
const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`ecommerce FBA registry verification failed: ${message}`);
  process.exit(1);
};

const central = read('docs/modules/registry.md');

for (const module of modules) {
  const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
  const registry = JSON.parse(read(registryPath));
  const plan = read(`crates/rustok-${module}/docs/implementation-plan.md`);
  const manifest = read(`crates/rustok-${module}/rustok-module.toml`);
  const cargo = read(`crates/rustok-${module}/Cargo.toml`);
  const portSource = read(`crates/rustok-${module}/src/ports.rs`);
  const libSource = read(`crates/rustok-${module}/src/lib.rs`);

  if (registry.module !== module) fail(`${registryPath} has module=${registry.module}`);
  if (registry.status !== 'in_progress') fail(`${module} registry status must be in_progress`);
  if (!Array.isArray(registry.ports) || registry.ports.length === 0) fail(`${module} has no ports`);
  if (!Array.isArray(registry.consumers) || registry.consumers.length === 0) fail(`${module} has no consumers`);

  for (const port of registry.ports) {
    if (port.owner !== module) fail(`${module}.${port.name} owner must be ${module}`);
    if (port.context !== 'rustok_api::ports::PortContext') fail(`${module}.${port.name} must use PortContext`);
    if (port.error !== 'rustok_api::ports::PortError') fail(`${module}.${port.name} must use PortError`);
    if (!Array.isArray(port.operations) || port.operations.length === 0) fail(`${module}.${port.name} has no operations`);
    if (port.deadline_required !== true) fail(`${module}.${port.name} must declare deadline_required=true`);
    if (!portSource.includes(`trait ${port.name}`)) fail(`${module} src/ports.rs lacks trait ${port.name}`);
    for (const operation of port.operations) {
      if (!portSource.includes(`${operation}(`)) fail(`${module}.${port.name} lacks operation ${operation}`);
    }
  }

  if (!manifest.includes('[fba.provider]')) fail(`${module} manifest lacks [fba.provider]`);
  if (!manifest.includes(`registry = "contracts/${module}-fba-registry.json"`)) fail(`${module} manifest registry path drift`);
  if (!manifest.includes(`contract_version = "${registry.contract_version}"`)) fail(`${module} manifest contract version drift`);
  if (!manifest.includes('context = "rustok_api::ports::PortContext"')) fail(`${module} manifest context drift`);
  if (!manifest.includes('error = "rustok_api::ports::PortError"')) fail(`${module} manifest error drift`);
  if (!cargo.includes('rustok-api.workspace = true')) fail(`${module} Cargo.toml lacks rustok-api dependency`);
  if (!libSource.includes('pub mod ports;') || !libSource.includes('pub use ports::*;')) fail(`${module} lib.rs must export ports`);
  if (!portSource.includes('rustok_api::{PortContext, PortError}')) fail(`${module} src/ports.rs must import neutral port primitives`);

  if (registry.in_process_provider_impl) {
    const implDeclaration = `impl ${registry.ports[0].name} for crate::${registry.in_process_provider_impl.service}`;
    if (!portSource.includes(implDeclaration)) fail(`${module} lacks in-process provider impl ${implDeclaration}`);
    if (registry.ports.some((port) => port.idempotency_required === true) && !portSource.includes('require_write_semantics()?')) {
      fail(`${module} in-process provider impl must enforce write semantics`);
    }
  }

  if (!plan.includes('- FBA status: `in_progress`')) fail(`${module} local plan FBA status drift`);
  if (!plan.includes(`${module}-fba-registry.json`)) fail(`${module} local plan lacks registry evidence`);
  if (!central.includes(`| \`${module}\` |`) || !central.includes(`crates/rustok-${module}/contracts/${module}-fba-registry.json`)) {
    fail(`${module} central readiness board lacks registry evidence`);
  }
}

console.log('ecommerce FBA registries verified: payment, fulfillment, order, pricing, inventory');
