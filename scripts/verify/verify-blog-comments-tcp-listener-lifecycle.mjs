import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-71.md';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const bootstrapPath = 'apps/server/src/services/server_bootstrap.rs';
const digestPath = 'crates/rustok-api/src/digest.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const adapterPath = 'crates/rustok-comments/src/tcp_server.rs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function requireCondition(condition, message) {
  if (!condition) throw new Error(message);
}

function requireText(source, fragment, path) {
  requireCondition(
    source.includes(fragment),
    `${path} must retain source fragment: ${fragment}`,
  );
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const runtime = read(runtimePath);
const bootstrap = read(bootstrapPath);
const digest = read(digestPath);
const auth = read(authPath);
const adapter = read(adapterPath);

requireCondition(evidence.schema_version === 1, 'evidence schema must remain version 1');
requireCondition(
  evidence.surface === 'comments_tcp_listener_lifecycle',
  'evidence surface must identify the Comments TCP listener lifecycle',
);
requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'listener evidence execution status must remain source-only',
);
requireCondition(evidence.configuration.default_enabled === false, 'listener must remain opt-in');
requireCondition(evidence.configuration.loopback_required === true, 'listener must remain loopback-only');
requireCondition(evidence.configuration.default_max_connections === 64, 'connection bound drift');
requireCondition(evidence.configuration.default_pre_request_timeout_ms === 2000, 'idle timeout drift');
requireCondition(evidence.configuration.default_shutdown_grace_ms === 5000, 'shutdown grace drift');
requireCondition(evidence.configuration.default_max_frame_bytes === 8388608, 'frame bound drift');
requireCondition(evidence.authority.allow_all_fallback === false, 'authority widened');
requireCondition(evidence.authority.listener_start_without_authority === false, 'authority became optional');
requireCondition(
  evidence.provider.consumer_port_reused_as_server_provider === false,
  'consumer port/server provider separation drift',
);
requireCondition(evidence.lifecycle.bounded_concurrency === 'tokio::sync::Semaphore', 'semaphore drift');
requireCondition(evidence.lifecycle.shared_shutdown_signal === 'StopHandle', 'shutdown signal drift');
requireCondition(evidence.lifecycle.abort_after_grace === true, 'bounded shutdown drift');
requireCondition(
  evidence.connection.pre_request_timeout_code === 'comments.tcp_server_idle_timeout',
  'pre-request timeout code drift',
);

for (const fragment of [
  'RUSTOK_COMMENTS_TCP_LISTENER_ENABLED',
  'RUSTOK_COMMENTS_TCP_BIND',
  'RUSTOK_COMMENTS_TCP_BEARER_TOKEN',
  'RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID',
  'RUSTOK_COMMENTS_TCP_MAX_CONNECTIONS',
  'RUSTOK_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS',
  'RUSTOK_COMMENTS_TCP_SHUTDOWN_GRACE_MS',
  'RUSTOK_COMMENTS_TCP_MAX_FRAME_BYTES',
  'DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS: usize = 64',
  'DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS: u64 = 2_000',
  'DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS: u64 = 5_000',
  'DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES: usize = 8 * 1024 * 1024',
  'pub struct SharedCommentsTcpAuthorityResolver',
  'pub struct SharedCommentsTcpServerChannelAcceptor(',
  'pub struct SharedCommentsTcpServerProvider',
  'pub struct CommentsTcpListenerConfig',
  'pub struct CommentsTcpListenerHandle',
  'CommentsTcpListenerLifecycleReservation',
  'bind_addr.ip().is_loopback()',
  'bind_addr.port() == 0',
  'runtime.is_registry_only()',
  'runtime.is_worker_only()',
  '.shared_get::<SharedCommentsTcpServerChannelAcceptor>()',
  '.get::<SharedCommentsTcpServerChannelAcceptor>()',
  '.unwrap_or_else(plaintext_server_channel_acceptor)',
  'let channel_protection = channel_acceptor.protection();',
  'comments_tcp_authority_from_environment',
  'CommentsTcpBearerAuthorityResolver::from_token(token, actor)',
  '.with_claim(COMMENTS_TCP_SERVICE_PERMISSION)',
  '.with_role(COMMENTS_TCP_SERVICE_ROLE)',
  'in_process_comments_thread_port(',
  'TcpJsonCommentsServerAdapter::with_max_frame_bytes(',
  'TcpListener::bind(config.bind_addr)',
  'Semaphore::new(config.max_connections)',
  'try_acquire_owned()',
  'JoinSet::new()',
  'peer_addr.ip().is_loopback()',
  'handle_connection_with_acceptor_and_pre_request_timeout(',
  'channel_acceptor.as_ref()',
  'StopHandle::new()',
  'stop_handle.subscribe()',
  'timeout(config.shutdown_grace',
  'connections.abort_all()',
  'channel_protection = ?channel_protection',
  'code = %error.code',
  'kind = ?error.kind',
  'retryable = error.retryable',
]) requireText(runtime, fragment, runtimePath);

requireCondition(
  !runtime.includes('struct AllowAllCommentsTcpAuthority'),
  'runtime must not introduce allow-all authority',
);
requireCondition(
  !runtime.includes('TrustedCommentsTcpAuthority::new('),
  'listener runtime must not mint authority from payload data',
);
requireCondition(
  !runtime.includes('0.0.0.0:'),
  'listener must not publish a wildcard endpoint',
);

for (const fragment of [
  'pub const SHA256_DIGEST_BYTES: usize = 32;',
  'pub fn sha256_digest(',
  'pub fn fixed_work_sha256_eq(',
]) requireText(digest, fragment, digestPath);

for (const fragment of [
  'pub struct CommentsTcpBearerAuthorityResolver',
  'fixed_work_sha256_eq',
  'comments.tcp_authentication_failed',
  '[REDACTED]',
]) requireText(auth, fragment, authPath);

for (const fragment of [
  'pub async fn handle_connection_with_acceptor_and_pre_request_timeout(',
  'acceptor: &dyn CommentsTcpServerChannelAcceptor',
  'let channel = acceptor.accept(stream, peer_addr).await?;',
  'comments.tcp_server_invalid_idle_timeout',
  'comments.tcp_server_idle_timeout',
  'timeout(duration, read_frame(channel, self.max_frame_bytes))',
  'serde_json::from_slice::<CommentsTcpRequestEnvelope>',
  'request.context().require_deadline_semantics()',
  'credential.as_ref()',
]) requireText(adapter, fragment, adapterPath);

requireText(bootstrap, 'start_comments_tcp_listener_if_enabled(', bootstrapPath);
requireText(plan, '## Slice 71 — host-owned Comments TCP listener lifecycle', planPath);
requireText(plan, 'Status: `source_verified_no_compile`.', planPath);

console.log('Blog Comments TCP listener lifecycle source contract is retained.');
