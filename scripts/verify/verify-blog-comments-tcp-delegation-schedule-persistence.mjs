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
const scheduleBridgePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_bridge.rs';
const scheduleGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs';
const triggerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs';
const persistencePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence.rs';
const persistedTriggerPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persisted_trigger.rs';
const triggerGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger_guard.rs';
const commentsSchedulePath = 'crates/rustok-comments/src/tcp_delegation_schedule.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-81.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-persistence.json';

const runtime = read(runtimePath);
const scheduleFacade = read(scheduleFacadePath);
const scheduleBridge = read(scheduleBridgePath);
const triggerGuard = read(triggerGuardPath);
const persistence = read(persistencePath);
const persistedTrigger = read(persistedTriggerPath);
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

requireCondition(
  gitBlobSha(scheduleBasePath) === '7d0c48df3dcea5a77bebe99dcdd4f02292f7ef47',
  'slice-79 host schedule base drift',
);
requireCondition(
  gitBlobSha(scheduleGuardPath) === 'af2d688d43286a8118823c31408dd5dc7e3f6202',
  'slice-79 schedule guard drift',
);
requireCondition(
  gitBlobSha(triggerPath) === 'f44f386c36ac81d51da89ee5568ffd9af083cda0',
  'slice-80 process-local trigger drift',
);
requireCondition(
  gitBlobSha(commentsSchedulePath) === '7701953d2892e1b68b4830135639d841ebdd6ed1',
  'Comments lifecycle schedule owner drift',
);

hasAll(
  scheduleFacade,
  [
    'include!("comments_provider_runtime_keyring_schedule_base.rs");',
    'include!("comments_provider_runtime_keyring_schedule_persistence_bridge.rs");',
    'pub struct SharedCommentsTcpDelegationScheduleHandle(',
    'pub(super) fn from_prepared_file(',
    'pub(super) fn replace_prepared_with_commit',
    'before_publish: F',
  ],
  'schedule persistence facade',
);

hasAll(
  scheduleBridge,
  [
    'pub(super) fn from_prepared_file(',
    'DelegationScheduleSource::File(file_path)',
    'pub(super) fn replace_prepared_with_commit',
    'let mut current = match self.0.current.write()',
    'candidate.generation <= current.generation',
    '.validate_replacement_from(&current.schedule, now_ms)',
    '.current_keyring_at(now_ms)',
    'let current_selection = schedule_selection_at(&candidate, now_ms)?;',
    'if let Err(error) = before_publish()',
    '*current = candidate;',
    '.successful_reloads',
  ],
  'persist-before-publish bridge',
);
const bridgeWriteLock = scheduleBridge.indexOf('let mut current = match self.0.current.write()');
const bridgeValidation = scheduleBridge.indexOf('.validate_replacement_from(&current.schedule, now_ms)');
const bridgeCommit = scheduleBridge.indexOf('if let Err(error) = before_publish()');
const bridgePublish = scheduleBridge.indexOf('*current = candidate;');
requireCondition(
  bridgeWriteLock >= 0
    && bridgeWriteLock < bridgeValidation
    && bridgeValidation < bridgeCommit
    && bridgeCommit < bridgePublish,
  'schedule validation / durable commit / publication ordering drift',
);

hasAll(
  persistence,
  [
    'pub const COMMENTS_TCP_DELEGATION_SCHEDULE_PERSISTENCE_SCHEMA_VERSION: u16 = 1;',
    'rustok-comments-tcp-delegation-schedule-state-v1\\0',
    'pub struct CommentsTcpDelegationScheduleDigest([u8; 32]);',
    'pub struct CommentsTcpDelegationSchedulePersistenceRecord',
    'pub enum CommentsTcpDelegationSchedulePersistenceStoreError',
    'pub trait CommentsTcpDelegationSchedulePersistenceStore: Send + Sync',
    'fn verify_current(',
    'fn compare_and_store(',
    'durably commit the candidate before returning `Ok(())`',
    'Any returned error must guarantee that the durable state remains exactly unchanged',
    'pub enum CommentsTcpDelegationSchedulePersistenceStartupMode',
    'BootstrapEmpty',
    'ResumeExact',
    'pub struct CommentsTcpDelegationSchedulePersistenceKey',
    'pub struct CommentsTcpDelegationSchedulePersistenceDocument',
    'programmatic precomputed digest',
    'Sha256::new()',
    'hasher.update(COMMENTS_TCP_DELEGATION_SCHEDULE_DIGEST_DOMAIN);',
    'hasher.update(propagation_budget_ms.to_be_bytes());',
    'hasher.update(max_ttl_ms.to_be_bytes());',
    'hasher.update(clock_skew_ms.to_be_bytes());',
    'update_text(&mut hasher, &key.key_id)?;',
    'update_text(&mut hasher, &key.secret)?;',
    'hasher.update(key.activates_at_unix_ms.to_be_bytes());',
    '#[serde(deny_unknown_fields)]',
    'COMMENTS_TCP_DELEGATION_SCHEDULE_FILE_SCHEMA_VERSION: u16 = 2',
    'MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES',
  ],
  'canonical persistence contract',
);
hasNone(
  persistence,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'tokio::spawn',
    'notify::',
    'signal::',
    'axum::',
    'async_graphql',
    'secret = %',
    'secret = ?',
    'key_id = %',
    'key_id = ?',
    'file_path = %',
    'file_path = ?',
  ],
  'persistence secret and transport sinks',
);

hasAll(
  persistedTrigger,
  [
    'pub struct SharedCommentsTcpDelegationPersistedScheduleTrigger',
    'pub fn from_host_document(',
    'pub fn from_file(',
    'CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty',
    '.compare_and_store(None, &initial_record)',
    'CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact',
    '.verify_current(&initial_record)',
    'pub fn current_persistence_record(',
    'pub fn reload_file(',
    'pub fn replace_host_schedule(',
    'context.principal_kind() == AuthPrincipalKind::DelegatedUser',
    'self.0.authorizer.authorize(',
    'let mut audit = self.0.audit.lock()',
    'let sequence = audit.allocate_sequence()?;',
    'let candidate = match prepare(&self.0.source)',
    'let mut persisted = self.0.persistence_record.lock()',
    'persisted.generation() != selection.generation',
    'self.0.schedule_handle.replace_prepared_with_commit(',
    '.compare_and_store(Some(&expected_record), &candidate_record)',
    '*persisted = candidate_record;',
    'CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceConflict',
    'CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceUnavailable',
    'CommentsTcpDelegationPersistedScheduleAuditOutcome::ReplacementSucceeded',
  ],
  'persisted trigger',
);
const operationIndex = persistedTrigger.indexOf('let _operation = self.0.operation.lock()');
const clockIndex = persistedTrigger.indexOf('let occurred_at_unix_ms = current_unix_ms()?;');
const authorizerIndex = persistedTrigger.indexOf('self.0.authorizer.authorize(');
const auditIndex = persistedTrigger.indexOf('let mut audit = self.0.audit.lock()');
const prepareIndex = persistedTrigger.indexOf('let candidate = match prepare(&self.0.source)');
const persistenceLockIndex = persistedTrigger.indexOf(
  'let mut persisted = self.0.persistence_record.lock()',
);
const replaceIndex = persistedTrigger.indexOf(
  'self.0.schedule_handle.replace_prepared_with_commit(',
);
const processRecordIndex = persistedTrigger.indexOf('*persisted = candidate_record;');
const successAuditIndex = persistedTrigger.indexOf(
  'outcome: CommentsTcpDelegationPersistedScheduleAuditOutcome::ReplacementSucceeded',
);
requireCondition(
  operationIndex >= 0
    && operationIndex < clockIndex
    && clockIndex < authorizerIndex
    && authorizerIndex < auditIndex
    && auditIndex < prepareIndex
    && prepareIndex < persistenceLockIndex
    && persistenceLockIndex < replaceIndex
    && replaceIndex < processRecordIndex
    && processRecordIndex < successAuditIndex,
  'persisted trigger ordering drift',
);
hasNone(
  persistedTrigger,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'tokio::spawn',
    'interval(',
    'notify::',
    'signal::',
    'axum::',
    'async_graphql',
    'bearer_token',
    'session_token',
    'secret = %',
    'secret = ?',
    'key_id = %',
    'key_id = ?',
    'file_path = %',
    'file_path = ?',
  ],
  'persisted trigger external surface and secret sinks',
);

hasAll(
  triggerGuard,
  [
    'SharedCommentsTcpDelegationPersistedScheduleTrigger',
    'Persisted and process-local Comments TCP delegation schedule triggers cannot be combined',
    'trigger and standalone schedule handle cannot be combined',
    'persisted_trigger',
    'process_local_trigger',
    'keyring_schedule_guard::register_comments_provider_runtime(extensions)',
  ],
  'persisted trigger composition guard',
);

hasAll(
  runtime,
  [
    'include!("comments_provider_runtime_keyring_schedule_persistence.rs");',
    'include!("comments_provider_runtime_keyring_schedule_persisted_trigger.rs");',
    'CommentsTcpDelegationSchedulePersistenceStore',
    'CommentsTcpDelegationSchedulePersistenceRecord',
    'CommentsTcpDelegationSchedulePersistenceDocument',
    'SharedCommentsTcpDelegationPersistedScheduleTrigger',
    'keyring_schedule_trigger_guard::register_comments_provider_runtime(extensions)',
  ],
  'runtime persistence exports',
);
hasNone(runtime, ['Sha256::new(', 'File::open(', 'serde_json::'], 'runtime facade ownership');

requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence status drift',
);
requireCondition(
  evidence.digest?.algorithm === 'sha256'
    && evidence.digest?.bytes === 32
    && evidence.digest?.canonical_binary === true
    && evidence.digest?.includes?.includes('exact_secret_bytes')
    && evidence.digest?.debug_value_exposed === false,
  'digest evidence drift',
);
requireCondition(
  evidence.store_contract?.linearizable_required === true
    && evidence.store_contract?.durable_before_success_required === true
    && evidence.store_contract?.error_leaves_state_unchanged_required === true
    && evidence.store_contract?.concrete_database_adapter === false
    && evidence.store_contract?.concrete_file_adapter === false,
  'store-contract evidence drift',
);
requireCondition(
  evidence.startup?.bootstrap_empty_explicit === true
    && evidence.startup?.resume_exact_explicit === true
    && evidence.startup?.resume_or_bootstrap_fallback === false
    && evidence.startup?.same_generation_different_digest_rejected === true
    && evidence.startup?.offline_automatic_advancement === false,
  'startup evidence drift',
);
requireCondition(
  evidence.replacement_ordering?.schedule_write_lock_before_store_cas === true
    && evidence.replacement_ordering?.store_cas_before_snapshot_assignment === true
    && evidence.replacement_ordering?.no_fallible_step_between_store_success_and_snapshot_assignment === true
    && evidence.replacement_ordering?.store_error_preserves_old_snapshot === true
    && evidence.replacement_ordering?.schedule_validation_error_calls_store === false,
  'replacement ordering evidence drift',
);
requireCondition(
  evidence.rollback_claim?.deployment_claim_conditional_on_conforming_store === true
    && evidence.rollback_claim?.concrete_backend_runtime_evidence === false
    && evidence.rollback_claim?.restart_exact_match_enforced === true,
  'conditional rollback evidence drift',
);
requireCondition(
  Array.isArray(evidence.dependency_contract?.new_direct_dependencies)
    && evidence.dependency_contract.new_direct_dependencies.length === 0
    && evidence.dependency_contract?.existing_sha2_dependency_reused === true
    && evidence.dependency_contract?.manifest_changed === false
    && evidence.dependency_contract?.cargo_lock_changed === false,
  'dependency evidence drift',
);
requireCondition(
  evidence.execution?.rust_tests_run === false
    && evidence.execution?.javascript_verifiers_run === false
    && evidence.execution?.cargo_commands_run === false
    && evidence.execution?.formatting_run === false
    && evidence.execution?.workflow_run === false
    && evidence.execution?.ci_run === false,
  'execution evidence drift',
);

hasAll(
  plan,
  [
    '## Slice 81 — durable generation and canonical schedule digest contract',
    '`BootstrapEmpty` performs:',
    '`ResumeExact` performs:',
    'same generation with different key IDs, secrets, timestamps',
    'durably commit the complete candidate before returning success',
    'any error leaves durable state exactly unchanged',
    'store performs',
    '`compare_and_store(current_record, candidate_record)`',
    'This slice deliberately does not provide a database, filesystem, broker, cloud,',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-81 plan',
);

console.log('Blog Comments TCP delegation schedule persistence source verification passed');
