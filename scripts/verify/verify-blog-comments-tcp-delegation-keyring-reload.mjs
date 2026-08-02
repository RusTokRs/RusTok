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
const staticTransportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const reloadTransportPath = 'crates/rustok-comments/src/tcp_transport_reload.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const staticHostPath = 'apps/server/src/services/comments_provider_runtime_keyring.rs';
const reloadHostPath =
  'apps/server/src/services/comments_provider_runtime_keyring_reload.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-78.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-keyring-reload.json';

const commentsLib = read(commentsLibPath);
const staticDelegation = read(staticDelegationPath);
const reloadDelegation = read(reloadDelegationPath);
const staticTransport = read(staticTransportPath);
const reloadTransport = read(reloadTransportPath);
const runtime = read(runtimePath);
const staticHost = read(staticHostPath);
const reloadHost = read(reloadHostPath);
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
  gitBlobSha(staticTransportPath) === '1b23a02eebe2f420901ed4290e34b436e0a51333',
  'static Comments transport drift',
);
requireCondition(
  gitBlobSha(staticHostPath) === '394c2511a3daf29f1f8bac3424fb096f859535f1',
  'static host keyring owner drift',
);

hasAll(
  commentsLib,
  [
    'pub mod tcp_delegation_reload;',
    'pub mod tcp_transport_reload;',
    'CommentsTcpDelegationKeyringProvider',
    'ReloadableCommentsTcpDelegatingAuthorityResolver',
    'ReloadableCommentsTcpDelegationSigner',
    'ReloadableTcpJsonCommentsTransport',
  ],
  'Comments exports',
);

hasAll(
  reloadDelegation,
  [
    'pub trait CommentsTcpDelegationKeyringProvider: Send + Sync',
    'fn current_keyring(&self) -> Result<CommentsTcpDelegationKeyring, PortError>;',
    'pub struct ReloadableCommentsTcpDelegationSigner',
    'let keyring = self.keyring_provider.current_keyring()?;',
    'CommentsTcpDelegationSigner::with_keyring_and_ttl(keyring, self.ttl)',
    'pub struct ReloadableCommentsTcpDelegatingAuthorityResolver',
    'replay: Arc<Mutex<ReloadableDelegationReplayState>>',
    'let keyring = self.keyring_provider.current_keyring()?;',
    'CommentsTcpDelegatingAuthorityResolver::with_keyring(',
    'let authority = resolver',
    'self.accept_verified_nonce_once(credential)?;',
    'credential.token()',
    'ReloadableSignedDelegation',
    'ReloadableDelegationClaims',
    'let nonce_digest = sha256_digest(&[claims.nonce.as_bytes()]);',
    '.saturating_add(self.clock_skew_ms)',
    'replay.entries.contains_key(&nonce_digest)',
    'comments.tcp_delegation_replayed',
    'comments.tcp_delegation_replay_unavailable',
    'DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS',
  ],
  'reloadable Comments delegation',
);
hasNone(
  reloadDelegation,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'credential = %',
    'credential = ?',
    'nonce = %',
    'nonce = ?',
    '%nonce_digest',
    '?nonce_digest',
  ],
  'delegation secret and nonce diagnostics',
);

hasAll(
  reloadTransport,
  [
    'pub struct ReloadableTcpJsonCommentsTransport',
    'delegation_signer: ReloadableCommentsTcpDelegationSigner',
    'self.delegation_signer.credential_for(&request)?',
    'CommentsTcpRequestEnvelope::with_credential(request, credential)',
    'CommentsTcpRequestEnvelope::with_bearer(request, &self.bearer_token)',
    'request.context().require_deadline_semantics()?;',
    'comments.tcp_timeout',
    'ensure_frame_size(request_payload.len(), self.max_frame_bytes)?;',
  ],
  'reloadable Comments transport',
);

hasAll(
  runtime,
  [
    'include!("comments_provider_runtime_keyring.rs");',
    'include!("comments_provider_runtime_keyring_reload.rs");',
    'COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV',
    'SharedCommentsTcpDelegationKeyringReloadHandle',
    'keyring_reload::register_comments_provider_runtime(extensions)',
    'keyring_reload::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'server runtime facade',
);
hasNone(runtime, ['env::var(', 'File::open(', 'serde_json::'], 'facade ownership');

hasAll(
  reloadHost,
  [
    '"RUSTOK_COMMENTS_TCP_DELEGATION_RELOAD_ENABLED"',
    'pub struct CommentsTcpDelegationKeyringReloadStatus',
    'pub struct CommentsTcpDelegationKeyringReloadOutcome',
    'pub struct SharedCommentsTcpDelegationKeyringReloadHandle',
    'current: RwLock<DelegationReloadSnapshot>',
    'successful_reloads: AtomicU64',
    'rejected_reloads: AtomicU64',
    'pub fn replace_host_keyring(',
    'pub fn reload_file(',
    'let candidate = match load_reload_snapshot_from_file(&file_path)',
    'let mut current = match self.0.current.write()',
    'candidate.selection.generation <= current.selection.generation',
    '*current = candidate;',
    'self.0.successful_reloads.fetch_add(1, Ordering::Relaxed);',
    'self.0.rejected_reloads.fetch_add(1, Ordering::Relaxed);',
    'impl CommentsTcpDelegationKeyringProvider',
    'current.keyring.clone()',
    'comments.tcp_delegation_keyring_unavailable',
  ],
  'host reload state',
);

hasAll(
  reloadHost,
  [
    'if host_handle.is_none() && !reload_enabled',
    'return keyring::register_comments_provider_runtime(extensions);',
    'Static and reloadable Comments TCP delegation keyring snapshots cannot be combined',
    'Host-provided Comments TCP delegation reload handle cannot be combined with file or legacy-secret environment configuration',
    'Comments TCP delegation reload handle requires a built-in TCP client or an enabled TCP listener',
    'ReloadableCommentsTcpDelegationSigner::with_ttl(',
    'ReloadableTcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(',
    'ReloadableCommentsTcpDelegatingAuthorityResolver::new(',
    'SharedCommentsTcpAuthorityResolver>()',
    'base::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'host reload composition',
);

hasAll(
  reloadHost,
  [
    '#[serde(deny_unknown_fields)]',
    'schema_version: u16',
    'generation: u64',
    'active_key_id: String',
    'legacy_unkeyed_key_id: Option<String>',
    'revoked_key_ids: Vec<String>',
    'metadata.len() > keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES as u64',
    '.take((keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES + 1) as u64)',
    'retained_ids.contains(raw_key_id)',
    'CommentsTcpDelegationKeyring::new(active_key_id, keys)',
    '.with_legacy_unkeyed_key_id(legacy_key_id)',
  ],
  'reload file validation',
);

hasAll(
  reloadHost,
  [
    '.field("file_path", &"[REDACTED]")',
    '.field("key_ids", &"[REDACTED]")',
    '.field("secrets", &"[REDACTED]")',
  ],
  'reload metadata redaction',
);
hasNone(
  reloadHost,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'notify::',
    'watch(',
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
  'reload non-claims and secret sinks',
);

requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence status drift',
);
requireCondition(
  evidence.atomicity?.candidate_validated_before_write_lock === true
    && evidence.atomicity?.whole_snapshot_write === true
    && evidence.atomicity?.generation_rechecked_under_lock === true
    && evidence.atomicity?.strictly_increasing_generation === true
    && evidence.atomicity?.equal_generation_rejected === true
    && evidence.atomicity?.lower_generation_rejected === true
    && evidence.atomicity?.failed_candidate_preserves_active_snapshot === true
    && evidence.atomicity?.persisted_generation === false,
  'atomicity evidence drift',
);
requireCondition(
  evidence.operation_semantics?.client_provider_reads_once_per_operation === true
    && evidence.operation_semantics?.listener_provider_reads_once_per_authorize === true
    && evidence.operation_semantics?.mixed_generation_operation === false
    && evidence.operation_semantics?.static_signer_changed === false
    && evidence.operation_semantics?.static_resolver_changed === false
    && evidence.operation_semantics?.static_transport_changed === false,
  'operation evidence drift',
);
requireCondition(
  evidence.replay?.one_gate_for_resolver_lifetime === true
    && evidence.replay?.gate_survives_generation_replacement === true
    && evidence.replay?.identity === 'sha256_of_verified_delegation_nonce'
    && evidence.replay?.signature_and_claim_validation_before_nonce_decode === true
    && evidence.replay?.cross_key_nonce_replay_rejected === true
    && evidence.replay?.process_local === true
    && evidence.replay?.shared === false
    && evidence.replay?.durable === false,
  'replay evidence drift',
);
requireCondition(
  evidence.sources?.automatic_watch === false
    && evidence.sources?.automatic_poll === false
    && evidence.sources?.signal_handler === false
    && evidence.sources?.admin_endpoint === false
    && evidence.sources?.file_path_fixed_for_handle_lifetime === true,
  'source evidence drift',
);
requireCondition(
  evidence.metadata?.file_path === false
    && evidence.metadata?.active_key_id === false
    && evidence.metadata?.secret_values === false
    && evidence.metadata?.delegation_nonce === false
    && evidence.metadata?.serialized_credential === false
    && evidence.metadata?.os_error_detail === false,
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
    '## Slice 78 — atomic process-local delegation reload',
    '`RUSTOK_COMMENTS_TCP_DELEGATION_RELOAD_ENABLED=true`',
    '`replace_host_keyring(...)`',
    '`reload_file()`',
    'candidate generation is strictly greater',
    'one process-local replay gate',
    'hashes the nonce',
    'cross-key replay semantics',
    'automatic file watching or polling',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-78 plan',
);

console.log('Blog Comments TCP delegation keyring reload source verification passed');
