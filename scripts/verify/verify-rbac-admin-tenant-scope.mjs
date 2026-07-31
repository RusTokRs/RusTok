#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-rbac/admin/src/transport/native_server_adapter.rs', root),
  'utf8',
);
const cargo = readFileSync(
  new URL('crates/rustok-rbac/admin/Cargo.toml', root),
  'utf8',
);
const fail = (message) => {
  console.error(`[verify-rbac-admin-tenant-scope] ${message}`);
  process.exit(1);
};

for (const marker of [
  'fn require_rbac_admin_tenant_scope<T>(',
  'rbac_admin_scope_matches(auth_tenant_id, resolved_tenant_id)',
  'code = "rbac.admin_tenant_scope_mismatch"',
  'boundary = RBAC_ADMIN_BOUNDARY',
  'Err(ServerFnError::new("RBAC admin access is denied"))',
  'require_rbac_admin_tenant_scope(&auth.tenant_id, &tenant.id)?;',
  'fn rbac_admin_scope_requires_matching_tenant()',
  'RBAC authentication context is temporarily unavailable',
  'RBAC tenant context is temporarily unavailable',
  'fn rbac_admin_context_error<E: std::fmt::Debug>(',
]) {
  if (!source.includes(marker)) fail(`RBAC Admin tenant/error guard missing ${marker}`);
}

const scopeIndex = source.indexOf('require_rbac_admin_tenant_scope(&auth.tenant_id, &tenant.id)?;');
const permissionIndex = source.indexOf('Permission::SETTINGS_READ');
if (scopeIndex < 0 || permissionIndex < 0 || scopeIndex >= permissionIndex) {
  fail('RBAC Admin must bind authenticated and resolved tenants before SETTINGS_READ admission');
}

for (const forbidden of [
  '.map_err(ServerFnError::new)?',
  '.map_err(|error| ServerFnError::new(error))?',
]) {
  if (source.includes(forbidden)) fail(`RBAC Admin exposes raw context errors through ${forbidden}`);
}

if (!cargo.includes('tracing.workspace = true')) {
  fail('RBAC Admin structured tenant-scope diagnostics require direct tracing dependency');
}

console.log(
  '[verify-rbac-admin-tenant-scope] RBAC Admin binds authenticated permissions to the resolved tenant and keeps context failures static publicly',
);
