import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) => readFileSync(path.join(root, relativePath));

const wrapperPath = 'apps/server/src/services/comments_provider_runtime.rs';
const basePath = 'apps/server/src/services/comments_provider_runtime_base.rs';
const keyringPath = 'apps/server/src/services/comments_provider_runtime_keyring.rs';
const delegationPath = 'crates/rustok-comments/src/tcp_delegation.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-77.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-keyring-host.json';

const wrapper = read(wrapperPath);
const base = read(basePath);
const keyring = read(keyringPath);
const delegation = read(delegationPath);
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
  gitBlobSha(basePath) === '3b204ceaed5996aa1dd2db2eaf695820bf4ba55b',
  'historical Comments runtime blob drift',
);

hasAll(
  wrapper,
  [
    'include!("comments_provider_runtime_base.rs");',
    'include!("comments_provider_runtime_keyring.rs");',
    'pub fn register_comments_provider_runtime(',
    'keyring::register_comments_provider_runtime(extensions)',
    'pub async fn start_comments_tcp_listener_if_enabled(',
    'keyring::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
    'CommentsProviderRuntimeSelection',
    'SharedCommentsTcpAuthorityResolver',
    'SharedCommentsTcpDelegationKeyringSnapshot',
  ],
  'runtime facade',
);
hasNone(wrapper, ['env::var(', 'File::open(', 'serde_json::'], 'facade ownership');

hasAll(
  base,
  [
    'pub fn register_comments_provider_runtime(',
    'pub async fn start_comments_tcp_listener_if_enabled(',
    'CommentsTcpDelegationSigner::with_ttl(',
    'CommentsTcpDelegatingAuthorityResolver::new(',
    'endpoint.ip().is_loopback()',
    'bind_addr.ip().is_loopback()',
    'handle_connection_with_acceptor_and_pre_request_timeout(',
  ],
  'historical runtime',
);

hasAll(
  keyring,
  [
    'pub const COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV:',
    '"RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE"',
    'pub const MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES: usize = 64 * 1024;',
    'pub enum CommentsTcpDelegationKeyringSource',
    'HostProvided',
    'File,',
    'pub struct CommentsTcpDelegationKeyringRuntimeSelection',
    'pub struct SharedCommentsTcpDelegationKeyringSnapshot',
    'pub fn from_host_keyring(',
    'source: CommentsTcpDelegationKeyringSource::HostProvided',
    '#[serde(deny_unknown_fields)]',
    'schema_version: u16',
    'generation: u64',
    'active_key_id: String',
    'legacy_unkeyed_key_id: Option<String>',
    'revoked_key_ids: Vec<String>',
    'keys: Vec<DelegationKeyFileEntry>',
    'resolve_keyring_snapshot(extensions)?',
    'host_snapshot.is_some() && (file_path.is_some() || legacy_secret.is_some())',
    'file_path.is_some() && legacy_secret.is_some()',
    'base::register_comments_provider_runtime(extensions)',
    'base::CommentsTcpListenerConfig::from_environment()?.is_some()',
    'requires a built-in TCP client or an enabled TCP listener',
    'let (port, selection) = prepare_tcp_client(extensions, &snapshot)?;',
    'extensions.insert(keyring_selection);',
    'extensions.insert(snapshot);',
    'keyring_snapshot_from_context(runtime_ctx)',
    'SharedCommentsTcpAuthorityResolver>()',
    'comments_tcp_authority_from_snapshot(&snapshot)',
    'base::start_comments_tcp_listener_if_enabled(runtime_ctx).await',
  ],
  'host keyring composition',
);

hasAll(
  keyring,
  [
    'metadata.len() > MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES as u64',
    '.take((MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES + 1) as u64)',
    'bytes.len() > MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES',
    'document.schema_version != COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION',
    'document.generation == 0',
    'document.revoked_key_ids.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS',
    '!retained_ids.insert(entry.key_id)',
    '!revoked_ids.insert(raw_key_id.clone())',
    'retained_ids.contains(raw_key_id)',
    'CommentsTcpDelegationKeyring::new(active_key_id, keys)',
    '.with_legacy_unkeyed_key_id(legacy_key_id)',
  ],
  'bounded file validation',
);

hasAll(
  keyring,
  [
    'CommentsTcpDelegationSigner::with_keyring_and_ttl(',
    'snapshot.keyring()',
    'TcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(',
    'CommentsTcpDelegatingAuthorityResolver::with_keyring(',
    '.with_service_claim(COMMENTS_TCP_SERVICE_PERMISSION)',
    '.with_service_role(COMMENTS_TCP_SERVICE_ROLE)',
    '.with_max_ttl(Duration::from_millis(ttl_ms))',
    '.with_replay_capacity(replay_capacity)',
    'CommentsTcpChannelProtection::PlaintextLoopback',
    'must remain loopback until protected Comments TCP runtime evidence is retained',
  ],
  'client/server snapshot use',
);

hasAll(
  keyring,
  [
    '.field("key_ids", &"[REDACTED]")',
    '.field("secrets", &"[REDACTED]")',
    'retained_key_count',
    'revoked_key_count',
    'legacy_unkeyed_enabled',
  ],
  'redacted runtime metadata',
);
hasNone(
  keyring,
  [
    'tracing::',
    'println!(',
    'eprintln!(',
    'notify::',
    'tokio::fs',
    'watch(',
    'raw_secret',
    'file_path = %',
    'file_path = ?',
    '%file_path',
    '?file_path',
    'secret = %',
    'secret = ?',
    'format!("{document:?}")',
    'format!("{entry:?}")',
    'format!("{:?}", document)',
    'format!("{:?}", entry)',
    'dbg!(document',
    'dbg!(entry',
  ],
  'secret path, parsed-document sink, and reload non-claims',
);

hasAll(
  delegation,
  [
    'pub const MAX_COMMENTS_TCP_DELEGATION_KEYS: usize = 8;',
    'pub struct CommentsTcpDelegationKeyring',
    'pub fn with_keyring(keyring: CommentsTcpDelegationKeyring) -> Self',
    'pub fn with_keyring_and_ttl(',
    'pub fn with_keyring(',
    'fn keyed_delegation_signature(',
    'fixed_work_sha256_eq(&expected, &signed.signature)',
    'self.accept_nonce(&claims.nonce, claims.expires_at_unix_ms, now_ms)?;',
  ],
  'Comments delegation owner',
);

requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence status drift',
);
requireCondition(
  evidence.source_resolution?.programmatic_snapshot === true
    && evidence.source_resolution?.file_environment
      === 'RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE'
    && evidence.source_resolution?.legacy_secret_fallback
      === 'RUSTOK_COMMENTS_TCP_DELEGATION_SECRET'
    && evidence.source_resolution?.ambiguous_host_and_file_rejected === true
    && evidence.source_resolution?.ambiguous_host_and_legacy_rejected === true
    && evidence.source_resolution?.ambiguous_file_and_legacy_rejected === true
    && evidence.source_resolution?.cloud_secret_manager_sdk === false,
  'source resolution evidence drift',
);
requireCondition(
  evidence.file_contract?.maximum_bytes === 65536
    && evidence.file_contract?.second_read_cap_bytes === 65537
    && evidence.file_contract?.unknown_fields_rejected === true
    && evidence.file_contract?.schema_version === 1
    && evidence.file_contract?.positive_generation_required === true
    && evidence.file_contract?.maximum_retained_keys === 8
    && evidence.file_contract?.maximum_revoked_key_ids === 8
    && evidence.file_contract?.retained_revoked_disjoint === true
    && evidence.file_contract?.path_in_public_error === false
    && evidence.file_contract?.os_error_detail_in_public_error === false,
  'file evidence drift',
);
requireCondition(
  evidence.snapshot?.immutable_keyring === true
    && evidence.snapshot?.complete_validation_before_publication === true
    && evidence.snapshot?.client_signer_uses_snapshot === true
    && evidence.snapshot?.listener_resolver_uses_same_snapshot === true
    && evidence.snapshot?.external_runtime_authority_precedence === true
    && evidence.snapshot?.external_extension_authority_precedence === true
    && evidence.snapshot?.unused_snapshot_rejected === true
    && evidence.snapshot?.live_reload === false,
  'snapshot evidence drift',
);
requireCondition(
  evidence.metadata?.file_path === false
    && evidence.metadata?.active_key_id === false
    && evidence.metadata?.retained_key_ids === false
    && evidence.metadata?.revoked_key_ids === false
    && evidence.metadata?.secret_values === false
    && evidence.metadata?.generation_monotonicity_persisted === false
    && evidence.metadata?.revoked_ids_are_durable_denylist === false,
  'metadata evidence drift',
);
requireCondition(
  Array.isArray(evidence.dependency_contract?.new_direct_dependencies)
    && evidence.dependency_contract.new_direct_dependencies.length === 0
    && evidence.dependency_contract?.manifest_changed === false
    && evidence.dependency_contract?.cargo_lock_changed === false,
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
    '## Slice 77 — host-owned delegation keyring snapshot',
    '`RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE`',
    'file content size is hard-bounded to 1..=65536 bytes',
    'programmatic `SharedCommentsTcpDelegationKeyringSnapshot`',
    'The client signer receives a clone of the immutable keyring snapshot',
    'the same snapshot',
    'runtime-context or module-extension `SharedCommentsTcpAuthorityResolver`',
    'An otherwise unused snapshot fails startup composition',
    'cloud secret-manager SDK integration',
    'live file watching, polling, signal reload, or hot replacement',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-77 plan',
);

console.log('Blog Comments TCP delegation keyring host source verification passed');
