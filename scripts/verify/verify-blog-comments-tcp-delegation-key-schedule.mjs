import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const commentsLibPath = 'crates/rustok-comments/src/lib.rs';
const staticDelegationPath = 'crates/rustok-comments/src/tcp_delegation.rs';
const reloadDelegationPath = 'crates/rustok-comments/src/tcp_delegation_reload.rs';
const schedulePath = 'crates/rustok-comments/src/tcp_delegation_schedule.rs';
const reloadTransportPath = 'crates/rustok-comments/src/tcp_transport_reload.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const staticHostPath = 'apps/server/src/services/comments_provider_runtime_keyring.rs';
const reloadHostPath =
  'apps/server/src/services/comments_provider_runtime_keyring_reload.rs';
const reloadGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_reload_guard.rs';
const scheduleHostPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule.rs';
const scheduleGuardPath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-79.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-key-schedule.json';

const commentsLib = read(commentsLibPath);
const schedule = read(schedulePath);
const runtime = read(runtimePath);
const scheduleHost = read(scheduleHostPath);
const scheduleGuard = read(scheduleGuardPath);
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
  gitBlobSha(staticDelegationPath) === '3814155056a7670782a95723befac499625ccc67',
  'static Comments delegation owner drift',
);
requireCondition(
  gitBlobSha(reloadDelegationPath) === 'dedcbb5d5099f595371b082848399b700fdf2622',
  'slice-78 reloadable delegation owner drift',
);
requireCondition(
  gitBlobSha(reloadTransportPath) === '1f8a79efcdca40ddefacec30d86f3a3c908a37c6',
  'slice-78 reloadable transport drift',
);
requireCondition(
  gitBlobSha(staticHostPath) === '394c2511a3daf29f1f8bac3424fb096f859535f1',
  'static host keyring owner drift',
);
requireCondition(
  gitBlobSha(reloadHostPath) === 'e971783cc7b746b3fbde6e47281e31cac264258f',
  'ordinary reload host owner drift',
);
requireCondition(
  gitBlobSha(reloadGuardPath) === '20c7af731847ac14a2fca1631d940308d7ea4a5f',
  'ordinary reload authority guard drift',
);

hasAll(
  commentsLib,
  [
    'pub mod tcp_delegation_schedule;',
    'CommentsTcpDelegationSchedule',
    'CommentsTcpDelegationScheduleConfigError',
    'CommentsTcpDelegationScheduledKey',
    'MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS',
    'MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_CLOCK_SKEW_MS',
  ],
  'Comments schedule exports',
);

hasAll(
  schedule,
  [
    'pub struct CommentsTcpDelegationScheduledKey',
    'activates_at_unix_ms: u64',
    'retires_at_unix_ms: Option<u64>',
    'pub struct CommentsTcpDelegationSchedule',
    'propagation_budget_ms: u64',
    'max_ttl_ms: u64',
    'clock_skew_ms: u64',
    'keys.sort_by_key(|key| key.activates_at_unix_ms);',
    'DuplicateActivation',
    'MissingRetirement',
    'TerminalKeyMustRemain',
    '.checked_add(propagation_budget_ms)',
    '.and_then(|value| value.checked_add(max_ttl_ms))',
    '.and_then(|value| value.checked_add(clock_skew_ms))',
    'InsufficientOverlap',
    '.saturating_sub(self.propagation_budget_ms)',
    'now_ms <= retirement',
    'pub fn validate_replacement_from(',
    'self.propagation_budget_ms < previous.propagation_budget_ms',
    'previous_active.key_id != candidate_active.key_id',
    'candidate.secret != retained.secret',
    'candidate.activates_at_unix_ms != retained.activates_at_unix_ms',
    'candidate_retirement < previous_retirement',
    '.checked_add(self.propagation_budget_ms)',
    'NewKeyActivatesTooEarly',
    'LegacyPolicyChangedEarly',
    'impl CommentsTcpDelegationKeyringProvider for CommentsTcpDelegationSchedule',
    'comments.tcp_delegation_schedule_unavailable',
  ],
  'Comments lifecycle schedule',
);
hasNone(
  schedule,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'tokio::spawn',
    'interval(',
    'sleep(',
    'notify::',
    'signal::',
    'secret = %',
    'secret = ?',
    'key_id = %',
    'key_id = ?',
  ],
  'schedule background work and secret diagnostics',
);

hasAll(
  runtime,
  [
    'include!("comments_provider_runtime_keyring_schedule.rs");',
    'include!("comments_provider_runtime_keyring_schedule_guard.rs");',
    'COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV',
    'SharedCommentsTcpDelegationScheduleHandle',
    'keyring_schedule_guard::register_comments_provider_runtime(extensions)',
    'keyring_schedule_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'server schedule facade',
);
hasNone(runtime, ['env::var(', 'File::open(', 'serde_json::'], 'facade ownership');

hasAll(
  scheduleGuard,
  [
    'validate_optional_bool(keyring_schedule::COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV)?;',
    'validate_optional_bool(keyring_reload::COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV)?;',
    'keyring_schedule::register_comments_provider_runtime(extensions)',
    'must be one of: true, false, 1, 0, yes, no, on, off',
  ],
  'schedule environment guard',
);

hasAll(
  scheduleHost,
  [
    '"RUSTOK_COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED"',
    'COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION: u16 = 2',
    'pub struct CommentsTcpDelegationScheduleRuntimeSelection',
    'pub struct CommentsTcpDelegationScheduleReloadStatus',
    'pub struct CommentsTcpDelegationScheduleReloadOutcome',
    'pub struct SharedCommentsTcpDelegationScheduleHandle',
    'current: RwLock<DelegationScheduleSnapshot>',
    'pub fn replace_host_schedule(',
    'pub fn reload_file(',
    'candidate.generation <= current.generation',
    '.validate_replacement_from(&current.schedule, now_ms)',
    '*current = candidate;',
    'impl CommentsTcpDelegationKeyringProvider',
    'schedule.current_keyring()',
    'ReloadableCommentsTcpDelegationSigner::with_ttl(',
    'ReloadableTcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(',
    'ReloadableCommentsTcpDelegatingAuthorityResolver::new(',
    'keyring_reload::register_comments_provider_runtime(extensions)',
    'keyring_reload_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
    'base::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'host schedule state and composition',
);

hasAll(
  scheduleHost,
  [
    '#[serde(deny_unknown_fields)]',
    'schema_version: u16',
    'generation: u64',
    'propagation_budget_ms: u64',
    'activates_at_unix_ms: u64',
    'retires_at_unix_ms: Option<u64>',
    'schema_version must equal {COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION} in schedule mode',
    'metadata.len() > keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES as u64',
    '.take((keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES + 1) as u64)',
    'CommentsTcpDelegationScheduledKey::new(',
    'CommentsTcpDelegationSchedule::new(',
    'DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS',
  ],
  'schedule file contract',
);

hasAll(
  scheduleHost,
  [
    'Scheduled, static, and ordinary reloadable Comments TCP delegation keyrings cannot be combined',
    'Host-provided Comments TCP delegation schedule cannot be combined with file or legacy-secret environment configuration',
    'Comments TCP delegation schedule requires a built-in TCP client or an enabled TCP listener',
    'Comments TCP delegation schedule is unused because an external listener authority override is configured',
    '.field("file_path", &"[REDACTED]")',
    '.field("key_ids", &"[REDACTED]")',
    '.field("secrets", &"[REDACTED]")',
  ],
  'host schedule ambiguity and redaction',
);
hasNone(
  scheduleHost,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'notify::',
    'tokio::fs',
    'signal::',
    'file_path = %',
    'file_path = ?',
    '%file_path',
    '?file_path',
    'secret = %',
    'secret = ?',
    'format!("{document:?}")',
    'format!("{entry:?}")',
    'dbg!(document',
    'dbg!(entry',
  ],
  'host schedule background work and secret sinks',
);

requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence status drift',
);
requireCondition(
  evidence.schedule_contract?.minimum_keys === 1
    && evidence.schedule_contract?.maximum_keys === 8
    && evidence.schedule_contract?.unique_activation_timestamps === true
    && evidence.schedule_contract?.non_terminal_retirement_required === true
    && evidence.schedule_contract?.terminal_retirement_forbidden === true,
  'schedule evidence drift',
);
requireCondition(
  evidence.overlap?.verification_starts_before_activation === true
    && evidence.overlap?.required_retirement_formula
      === 'successor_activation_plus_propagation_plus_max_ttl_plus_clock_skew'
    && evidence.overlap?.insufficient_overlap_rejected === true,
  'overlap evidence drift',
);
requireCondition(
  evidence.replacement?.strictly_increasing_generation === true
    && evidence.replacement?.propagation_budget_cannot_decrease === true
    && evidence.replacement?.active_key_cannot_change_at_replacement === true
    && evidence.replacement?.retained_secret_immutable === true
    && evidence.replacement?.retirement_cannot_move_earlier === true
    && evidence.replacement?.new_key_requires_full_propagation_lead === true
    && evidence.replacement?.persisted_generation === false,
  'replacement evidence drift',
);
requireCondition(
  evidence.operation_semantics?.one_effective_keyring_per_operation === true
    && evidence.operation_semantics?.mixed_schedule_operation === false
    && evidence.operation_semantics?.background_task_required === false
    && evidence.operation_semantics?.stable_unavailable_code
      === 'comments.tcp_delegation_schedule_unavailable',
  'operation evidence drift',
);
requireCondition(
  evidence.replay?.slice_78_reloadable_resolver_reused === true
    && evidence.replay?.gate_survives_activation === true
    && evidence.replay?.gate_survives_retirement === true
    && evidence.replay?.process_local === true
    && evidence.replay?.shared === false,
  'replay evidence drift',
);
requireCondition(
  evidence.metadata?.file_path === false
    && evidence.metadata?.active_key_id === false
    && evidence.metadata?.secret_values === false
    && evidence.metadata?.delegation_nonce === false,
  'metadata evidence drift',
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
    '## Slice 79 — scheduled delegation activation and retirement',
    '`RUSTOK_COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED=true`',
    '`schema_version: 2`',
    '`retirement >= successor activation + propagation budget + max TTL + clock skew`',
    '`activation - propagation budget`',
    '`now + propagation budget`',
    '`comments.tcp_delegation_schedule_unavailable`',
    'No background task is required.',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-79 plan',
);

console.log('Blog Comments TCP delegation key schedule source verification passed');
