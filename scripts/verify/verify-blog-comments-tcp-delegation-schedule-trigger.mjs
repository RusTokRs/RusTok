import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const scheduleFacadePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule.rs';
const scheduleBasePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_base.rs';
const scheduleGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs';
const triggerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs';
const triggerGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger_guard.rs';
const commentsSchedulePath = 'crates/rustok-comments/src/tcp_delegation_schedule.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-80.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-trigger.json';

const runtime = read(runtimePath);
const scheduleFacade = read(scheduleFacadePath);
const scheduleBase = read(scheduleBasePath);
const scheduleGuard = read(scheduleGuardPath);
const trigger = read(triggerPath);
const triggerGuard = read(triggerGuardPath);
const commentsSchedule = read(commentsSchedulePath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

function requireCondition(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function hasAll(source, markers, label) {
  for (const marker of markers) {
    requireCondition(source.includes(marker), `${label} missing marker: ${marker}`);
  }
}

function hasNone(source, markers, label) {
  for (const marker of markers) {
    requireCondition(!source.includes(marker), `${label} forbidden marker: ${marker}`);
  }
}

function gitBlobSha(relativePath) {
  const content = readBuffer(relativePath);
  return createHash('sha1')
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest('hex');
}

function stripRustLineComments(source) {
  return source.replace(/\/\/.*$/gm, '');
}

requireCondition(
  gitBlobSha(scheduleBasePath) === '7d0c48df3dcea5a77bebe99dcdd4f02292f7ef47',
  'slice-79 host schedule base drift',
);
requireCondition(
  gitBlobSha(scheduleGuardPath) === 'af2d688d43286a8118823c31408dd5dc7e3f6202',
  'slice-79 schedule guard drift',
);
requireCondition(
  gitBlobSha(commentsSchedulePath) === '7701953d2892e1b68b4830135639d841ebdd6ed1',
  'Comments schedule owner drift',
);

hasAll(
  scheduleFacade,
  [
    'mod historical {',
    'include!("comments_provider_runtime_keyring_schedule_base.rs");',
    'pub struct SharedCommentsTcpDelegationScheduleHandle(',
    'pub fn from_host_schedule(',
    'pub fn from_file(',
    'pub fn current_status(',
    'pub fn current_selection(',
    'pub(super) fn replace_host_schedule(',
    'pub(super) fn reload_file(',
    'impl CommentsTcpDelegationKeyringProvider for SharedCommentsTcpDelegationScheduleHandle',
    'historical::register_comments_provider_runtime(extensions)?;',
    'historical::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'read-only schedule facade',
);
const uncommentedScheduleFacade = stripRustLineComments(scheduleFacade);
requireCondition(
  !uncommentedScheduleFacade.includes('pub fn replace_host_schedule('),
  'raw host replacement remains publicly exported',
);
requireCondition(
  !uncommentedScheduleFacade.includes('pub fn reload_file('),
  'raw file reload remains publicly exported',
);

hasAll(
  trigger,
  [
    'pub const DEFAULT_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY: usize = 256;',
    'pub const MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY: usize = 1024;',
    'pub enum CommentsTcpDelegationScheduleTriggerOperation',
    'pub enum CommentsTcpDelegationScheduleTriggerAuthorizationError',
    'pub enum CommentsTcpDelegationScheduleTriggerAuditOutcome',
    'pub struct CommentsTcpDelegationScheduleTriggerContext',
    'request_id.is_nil()',
    'actor_id.is_nil()',
    'pub struct CommentsTcpDelegationScheduleTriggerAuthorizationRequest',
    'pub struct CommentsTcpDelegationScheduleTriggerAuditRecord',
    'pub trait CommentsTcpDelegationScheduleTriggerAuthorizer: Send + Sync',
    'pub struct SharedCommentsTcpDelegationScheduleTrigger',
    'operation: Mutex<()>',
    'audit: Mutex<DelegationScheduleTriggerAuditState>',
    'VecDeque::with_capacity(audit_capacity)',
    'pub fn reload_file(',
    'pub fn replace_host_schedule(',
    'handle.reload_file()',
    'handle.replace_host_schedule(schedule, generation)',
  ],
  'authorized schedule trigger',
);

hasAll(
  trigger,
  [
    'context.principal_kind == AuthPrincipalKind::DelegatedUser',
    'self.0.authorizer.authorize(',
    'CommentsTcpDelegationScheduleTriggerAuthorizationError::Denied',
    'CommentsTcpDelegationScheduleTriggerAuthorizationError::Unavailable',
    'CommentsTcpDelegationScheduleTriggerAuditOutcome::PrincipalIneligible',
    'CommentsTcpDelegationScheduleTriggerAuditOutcome::AuthorizationDenied',
    'CommentsTcpDelegationScheduleTriggerAuditOutcome::AuthorizationUnavailable',
    'CommentsTcpDelegationScheduleTriggerAuditOutcome::ReplacementRejected',
    'CommentsTcpDelegationScheduleTriggerAuditOutcome::ReplacementSucceeded',
  ],
  'trigger authorization outcomes',
);

const operationLockIndex = trigger.indexOf('let _operation = self.0.operation.lock()');
const clockIndex = trigger.indexOf('let occurred_at_unix_ms = current_unix_ms()?;');
const authorizerIndex = trigger.indexOf('self.0.authorizer.authorize(');
const auditLockIndex = trigger.indexOf('let mut audit = self.0.audit.lock()');
const sequenceIndex = trigger.indexOf('let sequence = audit.allocate_sequence()?;');
const mutationIndex = trigger.indexOf('let result = mutation(&self.0.schedule_handle);');
const successfulAuditIndex = trigger.indexOf(
  'outcome: CommentsTcpDelegationScheduleTriggerAuditOutcome::ReplacementSucceeded',
);
const finalReturnIndex = trigger.indexOf('\n        result\n', mutationIndex);
requireCondition(operationLockIndex >= 0, 'operation serialization marker missing');
requireCondition(
  operationLockIndex < clockIndex
    && clockIndex < authorizerIndex
    && authorizerIndex < auditLockIndex
    && auditLockIndex < sequenceIndex
    && sequenceIndex < mutationIndex
    && mutationIndex < successfulAuditIndex
    && successfulAuditIndex < finalReturnIndex,
  'authorization/audit/mutation ordering drift',
);

hasAll(
  trigger,
  [
    'if self.records.len() == self.capacity',
    'self.records.pop_front();',
    'self.records.push_back(record);',
    'self.next_sequence.checked_add(1)',
    'Comments TCP delegation schedule audit sequence is exhausted',
    '.field("authorizer", &"[CONFIGURED]")',
    '.field("audit_actor_ids", &"[REDACTED]")',
    '.field("audit_request_ids", &"[REDACTED]")',
  ],
  'bounded audit owner',
);

hasNone(
  trigger,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'File::open(',
    'serde_json::',
    'tokio::spawn',
    'interval(',
    'notify::',
    'signal::',
    'axum::',
    'async_graphql',
    'Mcp',
    'bearer_token',
    'session_token',
    'secret = %',
    'secret = ?',
    'key_id = %',
    'key_id = ?',
    'file_path = %',
    'file_path = ?',
  ],
  'trigger external surface and secret sinks',
);

hasAll(
  triggerGuard,
  [
    'SharedCommentsTcpDelegationScheduleTrigger>()',
    'standalone schedule handle cannot be combined',
    'extensions.insert(trigger.schedule_handle());',
    'keyring_schedule_guard::register_comments_provider_runtime(extensions)',
    'keyring_schedule_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'trigger composition guard',
);

hasAll(
  runtime,
  [
    'include!("comments_provider_runtime_keyring_schedule_trigger.rs");',
    'include!("comments_provider_runtime_keyring_schedule_trigger_guard.rs");',
    'CommentsTcpDelegationScheduleTriggerAuthorizer',
    'SharedCommentsTcpDelegationScheduleTrigger',
    'keyring_schedule_trigger_guard::register_comments_provider_runtime(extensions)',
    'keyring_schedule_trigger_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'runtime trigger exports and routing',
);
hasNone(runtime, ['File::open(', 'serde_json::', 'AuthPrincipalKind::'], 'runtime facade ownership');

requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence status drift',
);
requireCondition(
  evidence.mutation_boundary?.exported_schedule_handle_read_only === true
    && evidence.mutation_boundary?.public_raw_replace_method === false
    && evidence.mutation_boundary?.public_raw_file_reload_method === false
    && evidence.mutation_boundary?.trigger_required_for_exported_mutation === true
    && evidence.mutation_boundary?.historical_owner_blob_preserved === true,
  'mutation-boundary evidence drift',
);
requireCondition(
  evidence.authorization?.delegated_user_always_ineligible === true
    && evidence.authorization?.delegated_user_rejected_before_authorizer === true
    && evidence.authorization?.direct_user_requires_authorizer === true
    && evidence.authorization?.service_requires_authorizer === true
    && evidence.authorization?.free_form_reason_retained === false,
  'authorization evidence drift',
);
requireCondition(
  evidence.serialization?.one_process_local_operation_mutex === true
    && evidence.serialization?.audit_mutex_checked_before_mutation === true
    && evidence.serialization?.audit_sequence_checked_before_mutation === true
    && evidence.serialization?.audit_guard_held_during_mutation === true
    && evidence.serialization?.final_outcome_appended_before_return === true
    && evidence.serialization?.successful_return_without_local_record === false,
  'serialization evidence drift',
);
requireCondition(
  evidence.audit?.minimum_capacity === 1
    && evidence.audit?.default_capacity === 256
    && evidence.audit?.maximum_capacity === 1024
    && evidence.audit?.oldest_evicted_at_capacity === true
    && evidence.audit?.sequence_reused === false
    && evidence.audit?.file_path === false
    && evidence.audit?.key_ids === false
    && evidence.audit?.secret_values === false
    && evidence.audit?.raw_mutation_error === false
    && evidence.audit?.external_sink === false
    && evidence.audit?.durable === false,
  'audit evidence drift',
);
requireCondition(
  Array.isArray(evidence.dependency_contract?.new_direct_dependencies)
    && evidence.dependency_contract.new_direct_dependencies.length === 0
    && evidence.dependency_contract?.manifest_changed === false
    && evidence.dependency_contract?.cargo_lock_changed === false
    && evidence.dependency_contract?.feature_changed === false,
  'dependency evidence drift',
);
requireCondition(
  evidence.execution?.rust_tests_run === false
    && evidence.execution?.javascript_verifiers_run === false
    && evidence.execution?.cargo_commands_run === false
    && evidence.execution?.formatting_run === false
    && evidence.execution?.tcp_runtime_run === false
    && evidence.execution?.workflow_run === false
    && evidence.execution?.ci_run === false,
  'execution evidence drift',
);

hasAll(
  plan,
  [
    '## Slice 80 — authorized schedule trigger and bounded audit',
    'The exported `SharedCommentsTcpDelegationScheduleHandle` is now a read-only',
    '`CommentsTcpDelegationScheduleTriggerAuthorizer`',
    '`DelegatedUser` is always ineligible',
    'audit capacity within `1..=1024` records',
    'successful trigger return always has a corresponding local',
    '`ReplacementSucceeded` record',
    'There is no external sink in this slice.',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-80 plan',
);

console.log('Blog Comments TCP delegation schedule trigger source verification passed');
