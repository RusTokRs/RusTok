#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('apps/admin/src/features/email/transport/native_server_adapter.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};

for (const [value, label] of [
  ['fn require_email_admin_tenant_scope(', 'tenant scope helper'],
  ['if auth_tenant_id == resolved_tenant_id', 'tenant equality'],
  ['require_email_admin_tenant_scope(auth.tenant_id, tenant.id)?;', 'scoped endpoint call'],
  ['has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)', 'permission admission'],
  ['Email admin access is denied', 'static public denial'],
  ['email.admin_tenant_scope_mismatch', 'stable diagnostic code'],
  ['auth_tenant_id = %auth_tenant_id', 'authenticated tenant diagnostic'],
  ['resolved_tenant_id = %resolved_tenant_id', 'resolved tenant diagnostic'],
  ['boundary = "email_admin_native_transport"', 'transport boundary diagnostic'],
]) {
  requireText(value, label);
}

const guardCall = source.indexOf(
  'require_email_admin_tenant_scope(auth.tenant_id, tenant.id)?;',
);
const permissionCheck = source.indexOf(
  'has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)',
);
if (guardCall < 0 || permissionCheck < 0 || guardCall >= permissionCheck) {
  failures.push('tenant equality must precede SETTINGS_READ admission');
}

const authExtracts = source.match(/leptos_axum::extract::<AuthContext>\(\)/g) ?? [];
const tenantExtracts = source.match(/leptos_axum::extract::<TenantContext>\(\)/g) ?? [];
const guardCalls = source.match(
  /require_email_admin_tenant_scope\(auth\.tenant_id, tenant\.id\)\?;/g,
) ?? [];
if (authExtracts.length !== 1 || tenantExtracts.length !== 1 || guardCalls.length !== 1) {
  failures.push(
    `expected one AuthContext, one TenantContext and one scoped guard call; found ${authExtracts.length}/${tenantExtracts.length}/${guardCalls.length}`,
  );
}

if (failures.length > 0) {
  console.error('Email Admin tenant-scope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Email Admin binds SETTINGS_READ authority to the resolved tenant');
