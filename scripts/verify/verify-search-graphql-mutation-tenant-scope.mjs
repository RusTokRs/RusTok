#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const moduleSource = read('crates/rustok-search/src/graphql/mod.rs');
const mutationSource = read('crates/rustok-search/src/graphql/mutation.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};

for (const [value, label] of [
  ['async fn ensure_search_admin_permission(', 'shared admin permission helper'],
  ['ctx.data::<rustok_api::AuthContext>()', 'authenticated context'],
  ['ctx.data::<rustok_api::TenantContext>()?', 'resolved tenant context'],
  ['if auth.tenant_id != tenant.id', 'tenant equality'],
  ['Search administration access is denied', 'static public denial'],
  ['search.graphql_tenant_scope_mismatch', 'stable diagnostic code'],
  ['auth_tenant_id = %auth.tenant_id', 'authenticated tenant diagnostic'],
  ['resolved_tenant_id = %tenant.id', 'resolved tenant diagnostic'],
  ['boundary = "search_graphql_admin"', 'GraphQL boundary diagnostic'],
]) {
  requireText(moduleSource, value, label);
}

const tenantCheck = moduleSource.indexOf('if auth.tenant_id != tenant.id');
const permissionCheck = moduleSource.indexOf(
  'has_effective_permission(&auth.permissions, permission)',
);
if (tenantCheck < 0 || permissionCheck < 0 || tenantCheck >= permissionCheck) {
  failures.push('tenant equality must precede Search permission admission');
}

requireText(
  mutationSource,
  'super::ensure_search_admin_permission(ctx, &Permission::SETTINGS_MANAGE).await',
  'scoped manage helper delegation',
);
const manageCalls = mutationSource.match(/ensure_settings_manage_permission\(ctx\)\.await\?;/g) ?? [];
if (manageCalls.length !== 8) {
  failures.push(`expected eight authenticated Search manage mutations, found ${manageCalls.length}`);
}

for (const [value, label] of [
  ['async fn track_search_click(', 'public click analytics endpoint'],
  ['SearchAnalyticsService::record_click(', 'click analytics write'],
  ['tenant_id: tenant.id', 'click tenant scope'],
]) {
  requireText(mutationSource, value, label);
}
const trackStart = mutationSource.indexOf('async fn track_search_click(');
const firstManage = mutationSource.indexOf('async fn upsert_search_synonym(');
const trackBlock = mutationSource.slice(trackStart, firstManage);
if (trackBlock.includes('ensure_settings_manage_permission(')) {
  failures.push('track_search_click must not be disguised as an admin manage endpoint');
}

if (mutationSource.includes('has_effective_permission(&auth.permissions')) {
  failures.push('mutation-local permission-only admission must not bypass the shared scope helper');
}

if (failures.length > 0) {
  console.error('Search GraphQL mutation tenant-scope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ All eight Search GraphQL manage mutations bind authenticated authority to the resolved tenant',
);
