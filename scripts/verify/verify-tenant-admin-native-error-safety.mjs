#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-tenant-admin-native-error-safety] ${message}`);
  process.exit(1);
};

const cargo = read('crates/rustok-tenant/admin/Cargo.toml');
const adapter = read('crates/rustok-tenant/admin/src/transport/native_server_adapter.rs');

for (const marker of [
  'tracing.workspace = true',
  'uuid.workspace = true',
]) {
  if (!cargo.includes(marker)) fail(`tenant admin diagnostics dependency missing ${marker}`);
}

for (const marker of [
  'const TENANT_ADMIN_OWNER: &str = "rustok_tenant.admin_transport";',
  'const TENANT_ADMIN_BOUNDARY: &str = "tenant_admin_native_transport";',
  'fn tenant_admin_correlation_id()',
  'fn tenant_admin_context_error<',
  'fn tenant_admin_owner_error(',
  'fn tenant_admin_internal_error<',
  'error = ?error',
  'correlation_id',
  'tenant_id = %tenant_id',
  'code = "tenant.admin_access_denied"',
  '"Tenant admin access is denied"',
  '"Tenant authentication context is temporarily unavailable"',
  '"Tenant context is temporarily unavailable"',
  '"Tenant data is temporarily unavailable"',
  '"Tenant module state is temporarily unavailable"',
  '"Module composition is temporarily unavailable"',
  '"Module configuration is temporarily unavailable"',
  '"Effective module policy is temporarily unavailable"',
]) {
  if (!adapter.includes(marker)) fail(`tenant admin safe error boundary missing ${marker}`);
}

for (const forbidden of [
  '.map_err(ServerFnError::new)',
  'ServerFnError::new(format!(',
  'invalid active module manifest: {error}',
  'tenant admin bootstrap requires tenants:(read|list|manage)',
]) {
  if (adapter.includes(forbidden)) fail(`tenant admin exposes technical or policy detail through ${forbidden}`);
}

const mappedOperations = [
  'get_tenant',
  'list_tenant_modules',
  'active_snapshot',
  'decode_active_manifest',
  'resolve_enabled',
];
for (const operation of mappedOperations) {
  if (!adapter.includes(`"${operation}"`)) fail(`tenant admin operation lacks safe mapping: ${operation}`);
}

console.log('[verify-tenant-admin-native-error-safety] tenant admin native errors are static publicly and diagnostic privately');
