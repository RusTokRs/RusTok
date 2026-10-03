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
const nativePath = 'crates/modules/rustok-outbox/admin/src/transport/native_server_adapter.rs';
const controller = read(controllerPath);
const native = read(nativePath);

requireMarkers(controller, [
  'CurrentTenant(tenant): CurrentTenant',
  '_user: RequireLogsRead',
  'Permission::new(Resource::Logs, Action::Manage)',
  'Permission denied: logs:manage required',
  '.filter(entity::Column::Id.eq(id))',
  'sys_event_tenant_condition(',
  'tenant.id',
  "payload->>'tenant_id' = $1 OR payload->'event'->>'tenant_id' = $1",
  "json_extract(payload, '$.tenant_id') = ?1 OR json_extract(payload, '$.event.tenant_id') = ?1",
  'dlq_replay_permission_is_manage_not_read',
  'tenant_condition_covers_current_and_legacy_envelope_shapes',
], controllerPath);

// Bound both checks to their own regions: an unbounded `[\s\S]*?` scan would
// match `tenant_id:`/`RequireLogsRead` from unrelated code later in the file
// (e.g. the tenant-scoping helper this gate itself requires).
const dlqQueryStruct = controller.match(/pub struct DlqQuery\s*\{[^}]*\}/u)?.[0] ?? '';
if (/\btenant_id\s*:/u.test(dlqQueryStruct)) {
  fail('DLQ query accepts a client-selected tenant_id');
}
const replayStart = controller.indexOf('async fn replay_dlq_event');
const replayEnd = controller.indexOf('async fn', replayStart + 1);
const replayRegion = replayStart >= 0
  ? controller.slice(replayStart, replayEnd === -1 ? controller.length : replayEnd)
  : '';
if (replayRegion.includes('RequireLogsRead')) {
  fail('DLQ replay is still authorized by logs:read');
}
if (/find_by_id\(id\)/u.test(controller)) {
  fail('DLQ replay performs an unqualified event lookup');
}
if (!controller.includes('.ok_or(Error::NotFound)?;')) {
  fail('cross-tenant or missing replay targets must fail closed as not found');
}

requireMarkers(native, [
  'tenant context is required for outbox inspection',
  'tenant_slug: Some(tenant.slug)',
  'query_status_count(&db, backend, tenant.id, "pending")',
  'query_status_count(&db, backend, tenant.id, "dispatched")',
  'query_status_count(&db, backend, tenant.id, "failed")',
  'query_max_retry_count(&db, backend, tenant.id)',
  'fn tenant_scoped_status_sql(',
  'fn tenant_scoped_max_retry_sql(',
  "payload->>'tenant_id' = $2 OR payload->'event'->>'tenant_id' = $2",
  "json_extract(payload, '$.tenant_id') = ?2 OR json_extract(payload, '$.event.tenant_id') = ?2",
  'operational_queries_are_tenant_scoped_for_supported_backends',
], nativePath);

if (native.includes('tenant.map(|tenant| tenant.slug)')) {
  fail('native bootstrap still treats tenant context as optional');
}
if (native.includes('query_scalar_i64(')) {
  fail('native bootstrap retains an unscoped arbitrary SQL counter helper');
}
if (native.includes('SELECT COUNT(*) AS value FROM sys_events WHERE status = $1"')) {
  fail('native status counters remain unscoped');
}
if (native.includes('SELECT COALESCE(MAX(retry_count), 0) AS value FROM sys_events"')) {
  fail('native retry counter remains unscoped');
}

console.log('[verify-outbox-dlq-tenant-rbac] OK');
