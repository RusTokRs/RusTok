#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-rbac/admin/src/transport/native_server_adapter.rs', root),
  'utf8',
);
const owner = readFileSync(
  new URL('crates/rustok-rbac/src/control_plane.rs', root),
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
  'RbacControlPlanePrincipal',
  'require_direct_control_plane_user(principal, tenant.id)',
  'tenant_id: auth.tenant_id',
  'session_id: auth.session_id',
  'client_id: auth.client_id',
  'grant_type: &auth.grant_type',
  'code = "rbac.admin_control_plane_denied"',
  'boundary = RBAC_ADMIN_BOUNDARY',
  'ServerFnError::new("RBAC admin access is denied")',
  'RBAC authentication context is temporarily unavailable',
  'RBAC tenant context is temporarily unavailable',
  'fn rbac_admin_context_error<E: std::fmt::Debug>(',
]) {
  if (!source.includes(marker)) fail(`RBAC Admin principal/error guard missing ${marker}`);
}

const principalIndex = source.indexOf('require_direct_control_plane_user(principal, tenant.id)');
const permissionIndex = source.indexOf('Permission::SETTINGS_READ');
if (principalIndex < 0 || permissionIndex < 0 || principalIndex >= permissionIndex) {
  fail('RBAC Admin must admit a direct matching principal before SETTINGS_READ');
}

for (const forbidden of [
  'fn require_rbac_admin_tenant_scope<T>(',
  'fn rbac_admin_scope_matches<T: PartialEq>(',
  '.map_err(ServerFnError::new)?',
  '.map_err(|error| ServerFnError::new(error))?',
]) {
  if (source.includes(forbidden)) fail(`RBAC Admin retains obsolete or unsafe path ${forbidden}`);
}

for (const marker of [
  'pub struct RbacControlPlanePrincipal',
  'principal.client_id.is_some()',
  'principal.grant_type != "direct"',
  'principal.session_id.is_nil()',
  'principal.tenant_id != tenant_id',
]) {
  if (!owner.includes(marker)) fail(`RBAC owner control-plane policy missing ${marker}`);
}

if (!cargo.includes('tracing.workspace = true')) {
  fail('RBAC Admin structured control-plane diagnostics require direct tracing dependency');
}
if (!cargo.includes('"dep:rustok-rbac"')) {
  fail('RBAC Admin SSR feature must activate the RBAC owner policy dependency');
}

console.log(
  '[verify-rbac-admin-tenant-scope] RBAC Admin requires a direct matching principal before permission admission and keeps context failures static publicly',
);
