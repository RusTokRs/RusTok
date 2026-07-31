#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-commerce-tenant-locale-boundary] ${message}`);
  process.exit(1);
};

const rootManifest = read('modules.toml');
const exampleManifest = read('modules.toml.example');
const cargo = read('crates/rustok-commerce/Cargo.toml');
const manifest = read('crates/rustok-commerce/rustok-module.toml');
const lib = read('crates/rustok-commerce/src/lib.rs');
const context = read('crates/rustok-commerce/src/services/context.rs');
const tests = read('crates/rustok-commerce/tests/context_service_test.rs');
const support = read('crates/rustok-commerce/tests/support/mod.rs');
const evidence = read('crates/rustok-commerce/docs/tenant-locale-owner-cutover.md');

if (!cargo.includes('rustok-tenant.workspace = true')) {
  fail('commerce must depend on the tenant owner module in production');
}
if (!manifest.includes('tenant = { version_req = ">=0.1.0" }')) {
  fail('commerce module manifest must declare the tenant owner dependency');
}
if (!lib.includes('"tenant",')) {
  fail('CommerceModule runtime dependencies must include tenant');
}
const rootDependency = 'commerce = { crate = "rustok-commerce", source = "path", path = "crates/rustok-commerce", depends_on = ["tenant",';
if (!rootManifest.includes(rootDependency)) {
  fail('root modules.toml must order commerce after tenant');
}
if (!exampleManifest.includes(rootDependency)) {
  fail('modules.toml.example must mirror the commerce tenant dependency');
}

for (const marker of [
  'tenant_read_port: Arc<dyn TenantReadPort>',
  'tenant_locale_policy_port: Arc<dyn TenantLocalePolicyPort>',
  'TenantService::new(db)',
  'TenantReadSelector::Id(tenant_id)',
  'include_inactive: false',
  '.read_locale_policy(',
  'policy.default_locale.into_inner()',
  '.filter(|locale| locale.is_enabled)',
  'TenantLocale::new(value)',
  'tenant.locale_policy_default_mismatch',
  'StoreContextError::TenantBoundary',
]) {
  if (!context.includes(marker)) fail(`commerce tenant locale owner boundary missing ${marker}`);
}

for (const forbidden of [
  'SELECT default_locale FROM tenants',
  'SELECT locale FROM tenant_locales',
  'fn load_default_locale(',
  'fn load_enabled_locales(',
  'ConnectionTrait',
  'Statement',
  `.replace('_', "-").to_ascii_lowercase()`,
]) {
  if (context.includes(forbidden)) fail(`commerce context retains tenant storage or locale authority through ${forbidden}`);
}

for (const marker of [
  'resolve_context_uses_owner_canonical_locale_tags',
  'locale: Some("pt_br".to_string())',
  'assert_eq!(resolved.locale, "pt-BR")',
  'policy_revision',
]) {
  if (!tests.includes(marker)) fail(`commerce tenant locale runtime regression missing ${marker}`);
}

for (const marker of [
  'policy_revision INTEGER NOT NULL DEFAULT 0',
  'updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP',
]) {
  if (!support.includes(marker)) fail(`commerce tenant fixture schema missing ${marker}`);
}

for (const marker of [
  'Status: `source_ready_unvalidated`',
  'Severity: `P1`',
  '`TenantReadPort`',
  '`TenantLocalePolicyPort`',
  '`pt_br -> pt-BR`',
  'does not promote Commerce FBA/FFA status',
]) {
  if (!evidence.includes(marker)) fail(`commerce tenant locale evidence missing ${marker}`);
}

console.log('[verify-commerce-tenant-locale-boundary] commerce declares and consumes tenant owner ports with canonical locale semantics');
