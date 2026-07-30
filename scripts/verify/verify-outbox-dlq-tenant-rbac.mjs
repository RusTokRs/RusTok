#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-outbox-dlq-tenant-rbac] ${message}`);
  process.exit(1);
};
const requireMarkers = (source, markers, label) => {
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
  }
};

const controllerPath = 'apps/server/src/controllers/admin_events.rs';
const source = read(controllerPath);

requireMarkers(source, [
  'CurrentTenant(tenant): CurrentTenant',
  '_user: RequireLogsRead',
  'Permission::new(Resource::Logs, Action::Manage)',
  'Permission denied: logs:manage required',
  '.filter(entity::Column::Id.eq(id))',
  'sys_event_tenant_condition(',
  'tenant.id',
  'payload->>\'tenant_id\' = $1 OR payload->\'event\'->>\'tenant_id\' = $1',
  'json_extract(payload, \'$.tenant_id\') = ?1 OR json_extract(payload, \'$.event.tenant_id\') = ?1',
  'dlq_replay_permission_is_manage_not_read',
  'tenant_condition_covers_current_and_legacy_envelope_shapes',
], controllerPath);

if (/pub struct DlqQuery[\s\S]*?tenant_id\s*:/u.test(source)) {
  fail('DLQ query accepts a client-selected tenant_id');
}
if (/replay_dlq_event[\s\S]*?RequireLogsRead/u.test(source)) {
  fail('DLQ replay is still authorized by logs:read');
}
if (/find_by_id\(id\)/u.test(source)) {
  fail('DLQ replay performs an unqualified event lookup');
}
if (!source.includes('.ok_or(Error::NotFound)?;')) {
  fail('cross-tenant or missing replay targets must fail closed as not found');
}

console.log('[verify-outbox-dlq-tenant-rbac] OK');
