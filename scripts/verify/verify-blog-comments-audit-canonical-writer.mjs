import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);
const read = (relativePath) =>
  readFileSync(path.join(root, relativePath), 'utf8');

const eventContractPath = 'crates/rustok-events/src/contract.rs';
const eventApiPath = 'crates/rustok-events/CRATE_API.md';
const outboxTransportPath = 'crates/rustok-outbox/src/transport.rs';
const outboxTransactionalPath = 'crates/rustok-outbox/src/transactional.rs';
const outboxLibPath = 'crates/rustok-outbox/src/lib.rs';
const outboxApiPath = 'crates/rustok-outbox/CRATE_API.md';
const outboxTestPath = 'crates/rustok-outbox/tests/contract_write_once.rs';
const publicationPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_publication.rs';
const writerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_canonical_writer.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-89.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-audit-canonical-writer.json';
const temporaryWorkflowPath =
  '.github/workflows/tmp-blog-comments-canonical-writer-check.yml';

const eventContract = read(eventContractPath);
const eventApi = read(eventApiPath);
const outboxTransport = read(outboxTransportPath);
const outboxTransactional = read(outboxTransactionalPath);
const outboxLib = read(outboxLibPath);
const outboxApi = read(outboxApiPath);
const outboxTest = read(outboxTestPath);
const publication = read(publicationPath);
const writer = read(writerPath);
const runtime = read(runtimePath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

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

function hasNone(content, markers, label) {
  for (const marker of markers) {
    requireCondition(
      !content.includes(marker),
      `${label} contains forbidden marker: ${marker}`,
    );
  }
}

hasAll(eventContract, [
  'pub fn new_with_envelope_id<E>(',
  'Self::new_with_identity(envelope_id, tenant_id, actor_id, None, event)',
  'fn new_with_identity<E>(',
  'correlation_id: id',
  'pub fn correlation_id(&self) -> Uuid',
  'pub fn actor_id(&self) -> Option<Uuid>',
  'explicit_contract_envelope_identity_is_exact_and_correlated',
  'explicit_contract_envelope_identity_rejects_nil_uuid',
], 'exact contract envelope identity');

hasAll(outboxTransport, [
  'pub enum ContractEventWriteOnceError',
  'Conflict,',
  'Unavailable,',
  'write_contract_envelope_once_in_tx',
  'OnConflict::column(entity::Column::Id)',
  '.do_nothing()',
  'entity::Entity::find_by_id(envelope_id)',
  'stored_envelope',
  '.validate_registered_schema()',
  'same_contract_publication(&stored_envelope, &envelope)',
  'stored.correlation_id() != expected.correlation_id()',
  'stored.causation_id() != expected.causation_id()',
  'stored.tenant_id() != expected.tenant_id()',
  'stored.actor_id() != expected.actor_id()',
  'stored.payload()? == expected.payload()?',
  'write_once_comparison_ignores_generated_timestamp_and_trace_only',
  'write_once_comparison_rejects_scope_or_payload_reuse',
], 'canonical outbox write-once transport');

hasNone(outboxTransport, [
  'OutboxRelay::new',
  'tokio::spawn(',
  'FOR UPDATE SKIP LOCKED',
], 'canonical outbox write-once transport');

hasAll(outboxTransactional, [
  'publish_contract_once_direct_in_tx_with_envelope_id',
  'ContractEventEnvelope::new_with_envelope_id',
  'OutboxTransport::write_contract_envelope_once_in_tx',
  'ContractEventWriteOnceError::Unavailable',
], 'transactional write-once API');

hasAll(outboxLib, [
  'ContractEventWriteOnceError',
  'OutboxTransport',
], 'outbox public exports');

hasAll(writer, [
  'pub struct RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter',
  'impl CommentsTcpDelegationScheduleAuditCanonicalWriter',
  'BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded',
  'publication.idempotency_key()',
  'publication.control_plane_tenant_id()',
  'Some(publication.actor_id())',
  'TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id',
  'ContractEventWriteOnceError::Conflict',
  'ContractEventWriteOnceError::Unavailable',
  'maps_the_exact_bounded_audit_fact_into_the_registered_event',
  'maps_closed_outbox_errors_without_infrastructure_details',
], 'Blog canonical writer adapter');

hasNone(writer, [
  'OutboxTransport::new',
  'OutboxRelay::new',
  'tokio::spawn(',
  'EventTransport::publish',
  'rustok_iggy',
  'reqwest',
  'SKIP LOCKED',
], 'Blog canonical writer adapter');

hasAll(publication, [
  'occurred_at_unix_ms > i64::MAX as u64',
  'previous_generation > i64::MAX as u64',
  'candidate_generation > i64::MAX as u64',
  'rejects_values_outside_the_signed_wire_range',
], 'Blog publication wire range');

hasAll(runtime, [
  'mod keyring_schedule_audit_canonical_writer',
  'comments_provider_runtime_keyring_schedule_audit_canonical_writer.rs',
  'RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter',
], 'server runtime export');

hasAll(outboxTest, [
  'exact_replay_returns_the_same_envelope_and_keeps_one_row',
  'mismatched_request_id_reuse_returns_conflict_and_preserves_the_first_row',
  'publish_contract_once_direct_in_tx_with_envelope_id',
  'ContractEventWriteOnceError::Conflict',
  'SysEvents::find().count(&db)',
], 'write-once integration source coverage');

hasAll(eventApi, [
  'ContractEventEnvelope::new_with_envelope_id',
  'caller-owned durable identity',
  'both envelope ID and correlation ID',
  'does not perform',
], 'event API documentation');

hasAll(outboxApi, [
  'publish_contract_once_direct_in_tx_with_envelope_id',
  'ContractEventWriteOnceError { Conflict, Unavailable }',
  'ON CONFLICT DO NOTHING',
  'Exact replay requires matching',
  'Trace ID and envelope timestamp',
  'OutboxRelay',
], 'outbox API documentation');

hasAll(plan, [
  '## Slice 89 — canonical typed write-once writer',
  'ContractEventEnvelope::new_with_envelope_id',
  'ON CONFLICT (id) DO NOTHING',
  'Stable replay comparison includes',
  'Replay comparison deliberately ignores',
  'RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter',
  'atomic Blog-row-to-`sys_events` transaction',
  'Focused verification',
], 'slice 89 plan');

requireCondition(
  evidence.status ===
    'canonical_writer_source_ready_partial_compile_blocked_by_unrelated_main_error',
  'slice 89 evidence status drift',
);
requireCondition(
  evidence.source_contract.request_id_is_canonical_envelope_id === true
    && evidence.source_contract.request_id_is_correlation_id === true
    && evidence.source_contract.tenant_in_envelope_metadata === true
    && evidence.source_contract.actor_in_envelope_metadata === true,
  'source identity ownership evidence drift',
);
requireCondition(
  evidence.event_envelope.exact_identity_constructor_added === true
    && evidence.event_envelope.serialized_shape_changed === false
    && evidence.event_envelope.event_contract_digests_changed === false,
  'event envelope evidence drift',
);
requireCondition(
  evidence.canonical_write_once.owner === 'rustok-outbox'
    && evidence.canonical_write_once.on_conflict_do_nothing === true
    && evidence.canonical_write_once.winning_row_read_in_same_transaction === true
    && evidence.canonical_write_once.exact_replay_returns_existing_envelope_id === true
    && evidence.canonical_write_once.mismatched_reuse_returns_conflict === true
    && evidence.canonical_write_once.direct_transport_publication === false,
  'canonical write-once evidence drift',
);
requireCondition(
  Object.values(evidence.stable_replay_comparison).every(Boolean)
    && Object.values(evidence.ignored_replay_metadata).every(Boolean),
  'replay comparison evidence drift',
);
requireCondition(
  evidence.server_adapter.implements_slice_87_port === true
    && evidence.server_adapter.maps_registered_typed_event === true
    && evidence.server_adapter.uses_request_id_as_envelope_id === true
    && evidence.server_adapter.constructs_outbox_transport === false
    && evidence.server_adapter.starts_relay === false
    && evidence.server_adapter.starts_worker === false,
  'server adapter evidence drift',
);
requireCondition(
  Object.values(evidence.preserved_boundaries).every(
    (value) => value === false,
  ),
  'preserved boundary evidence drift',
);
// The evidence records the initial review attempt. It is intentionally distinct
// from current focused test results, which this verifier must never fabricate.
requireCondition(
  evidence.validation.rust_toolchain === '1.96.0'
    && evidence.validation.rustok_events_cargo_check_run === true
    && evidence.validation.rustok_events_cargo_check_conclusion === 'success'
    && evidence.validation.rustok_outbox_tests_cargo_check_run === true
    && evidence.validation.rustok_outbox_tests_cargo_check_conclusion === 'success'
    && evidence.validation.rustok_server_mod_blog_cargo_check_run === true
    && evidence.validation.rustok_server_mod_blog_cargo_check_conclusion ===
      'blocked_by_pre_existing_unrelated_error'
    && evidence.validation.blocker_intersects_slice_files === false
    && evidence.validation.rust_unit_tests_run === false
    && evidence.validation.rust_integration_tests_run === false
    && evidence.validation.javascript_verifier_run === false
    && evidence.validation.postgresql_run === false
    && evidence.validation.runtime_run === false
    && evidence.validation.production_run === false,
  'compile-only validation evidence drift',
);
requireCondition(
  Object.values(evidence.next_cursor).every(Boolean),
  'next implementation cursor drift',
);

requireCondition(
  !existsSync(path.join(root, temporaryWorkflowPath)),
  'temporary compile workflow must not remain in the final slice',
);

console.log('Blog Comments audit canonical writer verified');
