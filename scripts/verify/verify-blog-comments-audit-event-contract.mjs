import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);
const read = (relativePath) =>
  readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) =>
  readFileSync(path.join(root, relativePath));

const eventPath = 'crates/rustok-events/src/blog_comments_schedule_audit.rs';
const libPath = 'crates/rustok-events/src/lib.rs';
const contractPath = 'crates/rustok-events/src/contract.rs';
const testPath = 'crates/rustok-events/tests/blog_comments_schedule_audit.rs';
const digestPath = 'crates/rustok-events/contracts/event-contract-digests.json';
const apiPath = 'crates/rustok-events/CRATE_API.md';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-88.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-audit-event-contract.json';

const event = read(eventPath);
const lib = read(libPath);
const contract = read(contractPath);
const test = read(testPath);
const digest = JSON.parse(read(digestPath));
const api = read(apiPath);
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

function gitBlobSha(relativePath) {
  const content = readBuffer(relativePath);
  return createHash('sha1')
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest('hex');
}

const preserved = {
  'crates/rustok-events/src/schema.rs':
    'df973b3386b9229a8876b88e7bff5b639b8a1ff3',
  'crates/rustok-outbox/src/transport.rs':
    '29620cca59eb67fb0fe3ba21646d9c8d63bce428',
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_audit_publication.rs':
    'd82b35eb2d0c2b5c84ac4c00611ff98b96c0eb3b',
  'crates/rustok-blog/docs/implementation-plan-slice-87.md':
    '1a84f4a2776519cab98aefd0b7058a4abe8384d1',
};

for (const [relativePath, expectedSha] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expectedSha,
    `preserved owner drift: ${relativePath}`,
  );
}

hasAll(event, [
  'pub enum BlogCommentsDelegationScheduleAuditEvent',
  'ReplacementSucceeded',
  'BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE',
  'blog.comments_delegation_schedule.replacement_succeeded',
  'BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION: u16 = 1',
  'BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY',
  'comments_tcp_delegation_schedule',
  'audit_schema_version: u16',
  'request_id: Uuid',
  'occurred_at_unix_ms: i64',
  'principal_kind: String',
  'operation: String',
  'source: String',
  'previous_generation: i64',
  'candidate_generation: i64',
  'validators::validate_not_nil_uuid("request_id", request_id)',
  '"direct_user" | "service"',
  '"reload_file" | "replace_host_schedule"',
  '"host_provided" | "file"',
  'candidate_generation <= previous_generation',
  'impl sealed::Sealed for BlogCommentsDelegationScheduleAuditEvent',
  'impl EventContract for BlogCommentsDelegationScheduleAuditEvent',
  'ContractEventPayload::BlogCommentsDelegationScheduleAudit(self)',
], 'Blog Comments audit event');

const forbiddenEventMarkers = [
  'key_id',
  'secret',
  'schedule_digest',
  'schedule_document',
  'file_path',
  'database_url',
  'credential',
  'token',
  'nonce',
  'claims',
  'roles',
  'permissions',
  'raw_error',
  'operator_text',
];
for (const marker of forbiddenEventMarkers) {
  requireCondition(
    !event.toLowerCase().includes(marker),
    `event payload leaks forbidden marker: ${marker}`,
  );
}

hasAll(lib, [
  'mod blog_comments_schedule_audit;',
  'BlogCommentsDelegationScheduleAuditEvent',
  'BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS',
  'blog_comments_schedule_audit_event_schema(event_type)',
  '.chain(BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS.iter())',
], 'rustok-events registry');

hasAll(contract, [
  'BlogCommentsDelegationScheduleAuditEvent',
  '#[serde(rename = "blog_comments_delegation_schedule_audit")]',
  'BlogCommentsDelegationScheduleAudit(BlogCommentsDelegationScheduleAuditEvent)',
  'Self::BlogCommentsDelegationScheduleAudit(event) => event.event_type()',
  'Self::BlogCommentsDelegationScheduleAudit(event) => event.schema_version()',
  'Self::BlogCommentsDelegationScheduleAudit(event) => event.validate()',
], 'typed contract payload');

hasAll(test, [
  'registry_exposes_the_blog_comments_schedule_audit_contract',
  'registered_contract_envelope_round_trips_without_payload_drift',
  'ContractEventEnvelope::new',
  'serde_json::to_vec',
  'serde_json::from_slice',
  'validate_registered_schema',
  'ContractEventPayload::BlogCommentsDelegationScheduleAudit(event)',
  'assert_eq!(event.request_id(), request_id)',
], 'wire source coverage');

const expectedDigests = {
  format_version: 1,
  registry:
    'sha256:add56c12537c74f1c0a41cb7aa36847065eb9747f3443eacc4a8da08f34f4ce7',
  root_event:
    'sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87',
  root_envelope:
    'sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d',
  contract_payload:
    'sha256:4d3f53da292abe8777ff6463941072e3098e22a5d61b44e21c32f40432f590ea',
  contract_envelope:
    'sha256:59a4348d04ce4aa140a974929dbfc28888d0c5784dd0c057e5b6e17b2106d540',
};
requireCondition(
  JSON.stringify(digest) === JSON.stringify(expectedDigests),
  'generated event-contract digest artifact drift',
);

hasAll(api, [
  'BlogCommentsDelegationScheduleAuditEvent',
  'blog.comments_delegation_schedule.replacement_succeeded',
  'request_id',
  'canonical handoff idempotency',
  'Control-plane tenant and actor remain envelope metadata',
  'does not implement the Blog source-row handoff',
], 'rustok-events public API documentation');

hasAll(plan, [
  '## Slice 88 — sealed Blog Comments schedule-audit event contract',
  'Request identity exception',
  'Control-plane tenant and actor identity remain `ContractEventEnvelope` metadata',
  'Release digest generation',
  'cargo run -p rustok-events --example event_contract_digests',
  'generated_contract_source_ready_maintainer_tests_pending',
  'canonical writer implementation',
  'intentionally not run',
], 'slice 88 plan');

requireCondition(
  evidence.status ===
    'generated_contract_source_ready_maintainer_tests_pending',
  'slice 88 evidence status drift',
);
requireCondition(
  evidence.event.event_type ===
    'blog.comments_delegation_schedule.replacement_succeeded'
    && evidence.event.schema_version === 1,
  'event identity evidence drift',
);
requireCondition(
  evidence.payload.request_id_non_nil === true
    && evidence.payload.request_id_is_source_audit_identity === true
    && evidence.payload.tenant_in_envelope_metadata === true
    && evidence.payload.actor_in_envelope_metadata === true,
  'identity ownership evidence drift',
);
requireCondition(
  Object.values(evidence.privacy_exclusions).every(Boolean),
  'privacy exclusion evidence drift',
);
requireCondition(
  evidence.registration.sealed_event_contract === true
    && evidence.registration.schema_registered === true
    && evidence.registration.contract_payload_variant_added === true
    && evidence.registration.canonical_outbox_transport_changed === false
    && evidence.registration.canonical_relay_changed === false,
  'registration ownership evidence drift',
);
requireCondition(
  evidence.generated_digests.registry === expectedDigests.registry
    && evidence.generated_digests.root_event === expectedDigests.root_event
    && evidence.generated_digests.root_envelope === expectedDigests.root_envelope
    && evidence.generated_digests.contract_payload ===
      expectedDigests.contract_payload
    && evidence.generated_digests.contract_envelope ===
      expectedDigests.contract_envelope
    && evidence.generated_digests.root_event_unchanged === true
    && evidence.generated_digests.root_envelope_unchanged === true
    && evidence.generated_digests.values_guessed === false,
  'generated digest evidence drift',
);
requireCondition(
  evidence.generator_execution.completed === true
    && evidence.generator_execution.conclusion === 'success'
    && evidence.generator_execution.rust_version === '1.96.0'
    && evidence.generator_execution.tests_run === false
    && evidence.generator_execution.javascript_verifiers_run === false
    && evidence.generator_execution.postgresql_run === false,
  'generator execution evidence drift',
);
requireCondition(
  evidence.preserved_boundaries.outbox_transport_changed === false
    && evidence.preserved_boundaries.outbox_relay_changed === false
    && evidence.preserved_boundaries.worker_added === false
    && evidence.preserved_boundaries.environment_switch_added === false,
  'preserved runtime boundary drift',
);
requireCondition(
  Object.values(evidence.next_cursor).every(Boolean),
  'next implementation cursor drift',
);

requireCondition(
  !event.includes('rustok_outbox')
    && !event.includes('OutboxTransport')
    && !event.includes('OutboxRelay'),
  'event contract must not implement canonical persistence or relay',
);
requireCondition(
  !contract.includes('SKIP LOCKED')
    && !contract.includes('tokio::spawn'),
  'event contract must not add source-row claiming or workers',
);

console.log('Blog Comments schedule audit event contract verified');
