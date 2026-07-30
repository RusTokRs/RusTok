#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-auth/admin/src/transport/native_server_adapter.rs', root),
  'utf8',
);
const fail = (message) => {
  console.error(`[verify-auth-admin-tenant-scope] ${message}`);
  process.exit(1);
};

for (const marker of [
  'fn require_auth_admin_tenant_scope(',
  'if auth_tenant_id == resolved_tenant_id',
  'code = "auth.admin_tenant_scope_mismatch"',
  'boundary = "auth_admin_native_transport"',
  'Err(ServerFnError::new("Auth admin access is denied"))',
  'fn auth_admin_scope_requires_matching_tenant()',
]) {
  if (!source.includes(marker)) fail(`auth admin tenant-scope guard missing ${marker}`);
}

const scopeCall = 'require_auth_admin_tenant_scope(auth.tenant_id, tenant.id)?;';
const scopeCallCount = source.split(scopeCall).length - 1;
if (scopeCallCount !== 2) {
  fail(`auth admin tenant-scope guard must be called exactly twice, found ${scopeCallCount}`);
}

for (const [endpoint, permission] of [
  ['pub async fn list_users_native(', 'Permission::USERS_LIST'],
  ['pub async fn user_details_native(', 'Permission::USERS_READ'],
]) {
  const start = source.indexOf(endpoint);
  if (start < 0) fail(`auth admin endpoint missing ${endpoint}`);
  const nextEndpoint = source.indexOf('\n#[server(', start + endpoint.length);
  const section = source.slice(start, nextEndpoint < 0 ? source.length : nextEndpoint);
  const scopeIndex = section.indexOf(scopeCall);
  const permissionIndex = section.indexOf(permission);
  if (scopeIndex < 0 || permissionIndex < 0 || scopeIndex >= permissionIndex) {
    fail(`${endpoint} must bind authenticated and resolved tenants before RBAC admission`);
  }
}

console.log(
  '[verify-auth-admin-tenant-scope] Auth Admin read endpoints bind authenticated permissions to the resolved tenant before RBAC admission',
);
