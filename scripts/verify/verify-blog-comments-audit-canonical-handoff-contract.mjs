import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readComposedRustSource } from './lib/rust-facade-source.mjs';

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);
const read = (relativePath) =>
  readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) =>
  readFileSync(path.join(root, relativePath));

const sourcePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_publication.rs';
const runtimePath =
  'apps/server/src/services/comments_provider_runtime.rs';
const planPath =
  'crates/modules/rustok-blog/docs/implementation-plan-slice-87.md';
const evidencePath =
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-audit-canonical-handoff-contract.json';
const relayPath = 'crates/modules/rustok-outbox/src/relay.rs';

const source = readComposedRustSource(read, sourcePath);
const runtime = readComposedRustSource(read, runtimePath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));
const relay = read(relayPath);

function requireCondition(condition, message) {
  if (!condition) throw new Error(message);
}

function hasAll(content, markers, label) {
  for (const marker of markers) {
    requireCondition(
      content.includes(marker),
      `${label} missing marker: ${marker}`,
    );
  }
}

function gitBlobSha(relativePath) {
  const content = readBuffer(relativePath);
  return createHash('sha1')
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest('hex');
}

const preserved = {
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs':
    '4094539df6158d0a899e641322ae84bdbb6a129f',
  'crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs':
    '2f6473d7889a242ef86621b8615834e8bffe1013',
  'crates/modules/rustok-blog/docs/implementation-plan-slice-86.md':
    'c969aed84350f2ec7af4c60846f429debc47dfbe',
  [relayPath]: '47f98da024744f5d61c9faeca7492a4a6456c75d',
  'crates/modules/rustok-outbox/docs/implementation-plan.md':
    'b198630b31bff569503c45428c639ec4507259f7',
  'crates/libs/rustok-events/src/lib.rs':
    'd2593f110102800bd1e0e77f2d4aeaaf7791ae7c',
  'crates/libs/rustok-events/src/contract.rs':
    'caee66898db718514c6695b12cba1df3e6b491b5',
  'crates/libs/rustok-events/contracts/event-contract-digests.json':
    'aee557e02c5c34c08b64fd85454036a247567378',
  'apps/server/Cargo.toml':
    'e80ef5a046f33d493848e695164fa5954ec27d4d',
};

for (const [relativePath, expectedSha] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expectedSha,
    `preserved owner drift: ${relativePath}`,
  );
}

hasAll(source, [
  'COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_EVENT_TYPE',
  'blog.comments_delegation_schedule.replacement_succeeded',
  'COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_SCHEMA_VERSION: u16 = 1',
  'COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_STATE_KEY',
  'CommentsTcpDelegationScheduleAuditCanonicalPublication',
  'control_plane_tenant_id: Uuid',
  'request_id: Uuid',
  'actor_id: Uuid',
  'principal_kind: AuthPrincipalKind',
  'operation: trigger::CommentsTcpDelegationScheduleTriggerOperation',
  'source: keyring::CommentsTcpDelegationKeyringSource',
  'occurred_at_unix_ms: u64',
  'previous_generation: u64',
  'candidate_generation: u64',
  'control_plane_tenant_id.is_nil()',
  'request_id.is_nil()',
  'actor_id.is_nil()',
  'AuthPrincipalKind::DelegatedUser',
  'occurred_at_unix_ms == 0',
  'candidate_generation <= previous_generation',
  'pub fn idempotency_key(&self) -> Uuid',
  'pub trait CommentsTcpDelegationScheduleAuditCanonicalWriter: Send + Sync',
  'async fn write_once_in_transaction(',
  'transaction: &DatabaseTransaction',
  'CommentsTcpDelegationScheduleAuditCanonicalWriteError',
  'Conflict',
  'Unavailable',
  'SharedCommentsTcpDelegationScheduleAuditCanonicalWriter',
], 'canonical audit handoff source');

hasAll(runtime, [
  'mod keyring_schedule_audit_publication',
  '#[path = "comments_provider_runtime_keyring_schedule_audit_publication.rs"]',
  'CommentsTcpDelegationScheduleAuditCanonicalPublication',
  'CommentsTcpDelegationScheduleAuditCanonicalWriter',
  'SharedCommentsTcpDelegationScheduleAuditCanonicalWriter',
], 'Comments runtime facade');

hasAll(relay, [
  'pub struct RelayConfig',
  'pub claim_ttl: Duration',
  'pub backoff_base: Duration',
  'pub backoff_max: Duration',
  'LockBehavior::SkipLocked',
  'SysEventStatus::Failed',
  'Outbox event moved to DLQ (failed)',
], 'canonical outbox relay');

requireCondition(
  !source.includes('rustok_events')
    && !source.includes('ContractEventEnvelope'),
  'slice 87 must not publish an unregistered rustok-events wire contract',
);
requireCondition(
  !source.includes('rustok_outbox')
    && !source.includes('OutboxRelay')
    && !source.includes('OutboxTransport'),
  'slice 87 must not implement or compose canonical outbox delivery',
);
requireCondition(
  !source.includes('std::env')
    && !source.includes('RUSTOK_'),
  'control-plane tenant ownership must not be replaced by an environment switch',
);
requireCondition(
  !source.includes('SKIP LOCKED')
    && !source.includes('FOR UPDATE')
    && !source.includes('tokio::spawn'),
  'slice 87 must not add a claim query or background worker',
);

hasAll(plan, [
  '## 2026-08-03 continuation audit',
  'Slice 87 — canonical audit handoff admission contract',
  '`rustok-outbox` already owns the canonical `sys_events` table',
  'request UUID as `idempotency_key()`',
  'Transaction writer port',
  'Release-contract boundary',
  'does not add a new `rustok-events` family',
  'must not add',
  'Status: `source_verified_no_compile`',
  'intentionally not run',
], 'slice 87 plan');

requireCondition(
  evidence.status === 'source_verified_no_compile',
  'evidence status drift',
);
requireCondition(
  evidence.ownership_audit.canonical_relay_owner === 'rustok-outbox',
  'canonical relay ownership drift',
);
requireCondition(
  evidence.ownership_audit.blog_relay_added === false
    && evidence.ownership_audit.server_relay_added === false,
  'duplicate relay overclaim',
);
requireCondition(
  evidence.publication_document.host_control_plane_tenant_required === true,
  'control-plane tenant admission evidence missing',
);
requireCondition(
  evidence.publication_document.request_id_is_idempotency_key === true,
  'stable request identity evidence missing',
);
requireCondition(
  evidence.publication_document.delegated_principal_rejected === true,
  'delegated-principal rejection evidence missing',
);
requireCondition(
  evidence.writer_port.caller_owned_database_transaction === true,
  'caller-owned transaction evidence missing',
);
requireCondition(
  evidence.writer_port.implementation_added === false,
  'writer implementation overclaim',
);
requireCondition(
  evidence.release_contract_boundary.rustok_events_family_added === false
    && evidence.release_contract_boundary.event_contract_digest_changed === false
    && evidence.release_contract_boundary.digest_values_guessed === false,
  'event release-contract boundary drift',
);
requireCondition(
  evidence.runtime_boundary.background_worker_added === false
    && evidence.runtime_boundary.skip_locked_query_added === false
    && evidence.runtime_boundary.canonical_outbox_write_added === false,
  'runtime handoff overclaim',
);
requireCondition(
  evidence.plan_status.canonical_handoff_admission_contract_open === false
    && evidence.plan_status.sealed_platform_event_open === true
    && evidence.plan_status.canonical_writer_implementation_open === true,
  'slice cursor drift',
);
requireCondition(
  evidence.execution.rust_tests_run === false
    && evidence.execution.javascript_verifiers_run === false
    && evidence.execution.cargo_commands_run === false
    && evidence.execution.postgresql_run === false,
  'execution overclaim',
);

console.log(
  'Blog Comments canonical audit handoff admission contract verified',
);
