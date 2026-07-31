import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-listener-lifecycle.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-71.md';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const bootstrapPath = 'apps/server/src/services/server_bootstrap.rs';
const adapterPath = 'crates/rustok-comments/src/tcp_server.rs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function requireCondition(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
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
const adapter = read(adapterPath);

requireCondition(evidence.schema_version === 1, 'evidence schema must remain version 1');
requireCondition(
  evidence.surface === 'comments_tcp_listener_lifecycle',
  'evidence surface must identify the Comments TCP listener lifecycle',
);
requireCondition(
  evidence.status === 'source_verified_no_compile',
  'evidence must remain source-only',
);
requireCondition(
  evidence.compile_policy === 'not_run_by_request',
  'compile policy must record that execution was not requested',
);
requireCondition(
  evidence.runtime_status === 'not_run',
  'runtime status must remain not_run',
);
requireCondition(
  evidence.configuration.default_enabled === false,
  'listener must remain disabled by default',
);
requireCondition(
  evidence.configuration.loopback_required === true,
  'plaintext listener must remain loopback-only',
);
requireCondition(
  evidence.configuration.default_max_connections === 64,
  'default connection bound must remain 64',
);
requireCondition(
  evidence.configuration.default_pre_request_timeout_ms === 2000,
  'default pre-request timeout must remain 2000 ms',
);
requireCondition(
  evidence.configuration.default_shutdown_grace_ms === 5000,
  'default shutdown grace must remain 5000 ms',
);
requireCondition(
  evidence.configuration.default_max_frame_bytes === 8388608,
  'default frame bound must remain 8 MiB',
);
requireCondition(
  evidence.authority.allow_all_fallback === false,
  'authority must not gain an allow-all fallback',
);
requireCondition(
  evidence.authority.listener_start_without_authority === false,
  'listener must not start without host authority',
);
requireCondition(
  evidence.provider.consumer_port_reused_as_server_provider === false,
  'consumer-selected TCP port must remain separate from server provider selection',
);
requireCondition(
  evidence.lifecycle.bounded_concurrency === 'tokio::sync::Semaphore',
  'listener concurrency must remain semaphore bounded',
);
requireCondition(
  evidence.lifecycle.shared_shutdown_signal === 'StopHandle',
  'listener must remain attached to the shared StopHandle',
);
requireCondition(
  evidence.lifecycle.abort_after_grace === true,
  'shutdown must remain bounded by a grace period',
);
requireCondition(
  evidence.connection.pre_request_timeout_code ===
    'comments.tcp_server_idle_timeout',
  'pre-request timeout code must remain stable',
);

for (const fragment of [
  'RUSTOK_COMMENTS_TCP_LISTENER_ENABLED',
  'RUSTOK_COMMENTS_TCP_BIND',
  'RUSTOK_COMMENTS_TCP_MAX_CONNECTIONS',
  'RUSTOK_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS',
  'RUSTOK_COMMENTS_TCP_SHUTDOWN_GRACE_MS',
  'RUSTOK_COMMENTS_TCP_MAX_FRAME_BYTES',
  'DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS: usize = 64',
  'DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS: u64 = 2_000',
  'DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS: u64 = 5_000',
  'DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES: usize = 8 * 1024 * 1024',
  'pub struct SharedCommentsTcpAuthorityResolver',
  'pub struct SharedCommentsTcpServerProvider',
  'pub struct CommentsTcpListenerConfig',
  'pub struct CommentsTcpListenerHandle',
  'CommentsTcpListenerLifecycleReservation',
  'bind_addr.ip().is_loopback()',
  'bind_addr.port() == 0',
  'max_frame_bytes > u32::MAX as usize',
  'runtime.is_registry_only()',
  'runtime.is_worker_only()',
  'SharedCommentsTcpAuthorityResolver',
  'in_process_comments_thread_port(',
  'TcpJsonCommentsServerAdapter::with_max_frame_bytes(',
  'TcpListener::bind(config.bind_addr)',
  'Semaphore::new(config.max_connections)',
  'try_acquire_owned()',
  'JoinSet::new()',
  'peer_addr.ip().is_loopback()',
  'handle_connection_with_pre_request_timeout(',
  'StopHandle::new()',
  'stop_handle.subscribe()',
  'timeout(config.shutdown_grace',
  'connections.abort_all()',
  'code = %error.code',
  'kind = ?error.kind',
  'retryable = error.retryable',
]) {
  requireText(runtime, fragment, runtimePath);
}

requireCondition(
  !runtime.includes('struct AllowAllCommentsTcpAuthority'),
  'runtime must not introduce an allow-all Comments authority resolver',
);
requireCondition(
  !runtime.includes('TrustedCommentsTcpAuthority::new('),
  'host listener runtime must not manufacture trusted authority from the payload',
);

for (const fragment of [
  'handle_connection_with_pre_request_timeout',
  'comments.tcp_server_invalid_idle_timeout',
  'comments.tcp_server_idle_timeout',
  'timeout(duration, read_frame(stream, self.max_frame_bytes))',
  'request.context().require_deadline_semantics()',
  '.authorize(peer_addr, operation, request.context())',
]) {
  requireText(adapter, fragment, adapterPath);
}

requireText(
  bootstrap,
  'start_comments_tcp_listener_if_enabled(',
  bootstrapPath,
);
requireText(
  bootstrap,
  'bootstrap_app_runtime(runtime_ctx.clone(), auth_config.clone(), &rustok_settings).await?',
  bootstrapPath,
);
requireText(plan, '## Slice 71 — host-owned Comments TCP listener lifecycle', planPath);
requireText(plan, 'Status: `source_verified_no_compile`.', planPath);
requireText(plan, 'concrete authenticated authority resolver', planPath);
requireText(plan, 'intentionally not run in this slice', planPath);

console.log('Blog Comments TCP listener lifecycle source contract is retained.');
