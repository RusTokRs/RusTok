import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-host-channel-selection.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-75.md';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const channelPath = 'crates/rustok-comments/src/tcp_channel.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const commentsManifestPath = 'crates/rustok-comments/Cargo.toml';
const lockPath = 'Cargo.lock';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-host-channel-selection] ${message}`);
  process.exit(1);
}

function requireCondition(condition, message) {
  if (!condition) fail(message);
}

function hasAll(source, fragments, label) {
  for (const fragment of fragments) {
    requireCondition(source.includes(fragment), `${label} missing ${fragment}`);
  }
}

function hasNone(source, fragments, label) {
  for (const fragment of fragments) {
    requireCondition(!source.includes(fragment), `${label} contains forbidden ${fragment}`);
  }
}

function packageBlock(lock, name) {
  const marker = `[[package]]\nname = "${name}"\n`;
  const start = lock.indexOf(marker);
  requireCondition(start >= 0, `Cargo.lock missing package ${name}`);
  const next = lock.indexOf('\n[[package]]', start + marker.length);
  return lock.slice(start, next < 0 ? lock.length : next);
}

for (const path of [
  evidencePath,
  planPath,
  runtimePath,
  channelPath,
  transportPath,
  serverPath,
  commentsManifestPath,
  lockPath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const runtime = read(runtimePath);
const channel = read(channelPath);
const transport = read(transportPath);
const server = read(serverPath);
const commentsManifest = read(commentsManifestPath);
const lock = read(lockPath);

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_host_channel_selection',
  'evidence identity drift',
);
requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence execution status drift',
);
requireCondition(evidence.generated_from === planPath, 'plan path drift');

requireCondition(
  evidence.client_selection?.wrapper === 'SharedCommentsTcpClientChannelConnector'
    && evidence.client_selection?.source === 'ModuleRuntimeExtensions'
    && evidence.client_selection?.default === 'PlaintextLoopbackCommentsTcpChannel'
    && evidence.client_selection?.selected_before_port_publication === true
    && evidence.client_selection?.plaintext_profile === 'TcpLoopback'
    && evidence.client_selection?.protected_profile === 'TcpProtectedLoopback',
  'client selection evidence drift',
);
requireCondition(
  evidence.server_selection?.wrapper === 'SharedCommentsTcpServerChannelAcceptor'
    && JSON.stringify(evidence.server_selection?.precedence) === JSON.stringify([
      'ServerRuntimeContext',
      'ModuleRuntimeExtensions',
      'PlaintextLoopbackCommentsTcpChannel',
    ])
    && evidence.server_selection?.resolved_before_listener_spawn === true
    && evidence.server_selection?.shared_across_connections === true
    && evidence.server_selection?.handshake_owned_by_acceptor === true
    && evidence.server_selection?.request_idle_timeout_starts_after_channel_establishment === true,
  'server selection evidence drift',
);
requireCondition(
  evidence.policy?.plaintext_endpoint_loopback_only === true
    && evidence.policy?.protected_endpoint_loopback_only === true
    && evidence.policy?.listener_bind_loopback_only === true
    && evidence.policy?.accepted_peer_loopback_only === true
    && evidence.policy?.non_loopback_enabled === false
    && evidence.policy?.authenticated_encrypted_classification_is_runtime_evidence === false
    && evidence.policy?.channel_mints_application_authority === false
    && evidence.policy?.bearer_and_delegation_retained === true
    && evidence.policy?.tenant_match_retained === true
    && evidence.policy?.owner_policy_retained === true,
  'channel policy evidence drift',
);
requireCondition(
  Array.isArray(evidence.dependency_contract?.new_direct_dependencies)
    && evidence.dependency_contract.new_direct_dependencies.length === 0
    && evidence.dependency_contract?.manifest_changed === false
    && evidence.dependency_contract?.cargo_lock_changed === false,
  'dependency evidence drift',
);

hasAll(
  runtime,
  [
    'CommentsTcpChannelProtection, CommentsTcpClientChannelConnector,',
    'CommentsTcpServerChannelAcceptor, CommentsThreadPort,',
    'PlaintextLoopbackCommentsTcpChannel',
    'TcpProtectedLoopback',
    'pub struct SharedCommentsTcpClientChannelConnector(',
    'pub struct SharedCommentsTcpServerChannelAcceptor(',
    '.get::<SharedCommentsTcpClientChannelConnector>()',
    '.unwrap_or_else(plaintext_client_channel_connector)',
    'let channel_protection = channel_connector.protection();',
    'require_loopback_endpoint(endpoint, channel_protection)?;',
    'TcpJsonCommentsTransport::with_channel_connector_and_bearer_token(',
    'TcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(',
    '.shared_get::<SharedCommentsTcpServerChannelAcceptor>()',
    '.get::<SharedCommentsTcpServerChannelAcceptor>()',
    '.unwrap_or_else(plaintext_server_channel_acceptor)',
    'channel_acceptor: Arc<dyn CommentsTcpServerChannelAcceptor>',
    'let channel_acceptor = channel_acceptor.clone();',
    'handle_connection_with_acceptor_and_pre_request_timeout(',
    'channel_acceptor.as_ref()',
    'channel_protection = ?channel_protection',
    'peer_addr.ip().is_loopback()',
    'must remain loopback until protected Comments TCP runtime evidence is retained',
    'CommentsTcpBearerAuthorityResolver::from_token(token, actor)',
    'CommentsTcpDelegatingAuthorityResolver::new(token, actor, delegation_secret)',
  ],
  'host runtime',
);
hasNone(
  runtime,
  [
    'rustls::',
    'tokio_rustls::',
    'native_tls::',
    'openssl::',
    'struct AllowAllCommentsTcpAuthority',
    'TrustedCommentsTcpAuthority::new(',
    '0.0.0.0:',
    'retry(',
    'println!(',
  ],
  'host runtime non-claims',
);

hasAll(
  channel,
  [
    'pub enum CommentsTcpChannelProtection',
    'PlaintextLoopback',
    'AuthenticatedEncrypted',
    'pub trait CommentsTcpClientChannelConnector: Send + Sync',
    'pub trait CommentsTcpServerChannelAcceptor: Send + Sync',
    'pub struct PlaintextLoopbackCommentsTcpChannel;',
    'comments.tcp_plaintext_non_loopback',
  ],
  'channel seam',
);

hasAll(
  transport,
  [
    'channel_connector: Arc<dyn CommentsTcpClientChannelConnector>',
    'pub fn with_channel_connector_and_bearer_token(',
    'pub fn with_channel_connector_bearer_and_delegation(',
    'self.channel_connector.connect(self.endpoint).await?',
    'request.context().require_deadline_semantics()?;',
  ],
  'client transport',
);

hasAll(
  server,
  [
    'pub async fn handle_connection_with_acceptor_and_pre_request_timeout(',
    'acceptor: &dyn CommentsTcpServerChannelAcceptor',
    'let channel = acceptor.accept(stream, peer_addr).await?;',
    'timeout(duration, read_frame(channel, self.max_frame_bytes))',
    '.authorize(peer_addr, operation, credential.as_ref(), &request)',
    'replace_request_context(&mut request, trusted_context);',
  ],
  'server adapter',
);

hasNone(
  commentsManifest,
  ['rustls =', 'tokio-rustls =', 'native-tls =', 'openssl ='],
  'comments manifest',
);
const commentsLockBlock = packageBlock(lock, 'rustok-comments');
hasNone(
  commentsLockBlock,
  ['"rustls"', '"tokio-rustls"', '"native-tls"', '"openssl"'],
  'comments lock entry',
);

hasAll(
  plan,
  [
    '# rustok-blog implementation plan — slice 75 continuation',
    '## Slice 75 — host-selected protected Comments channel wiring',
    'SharedCommentsTcpClientChannelConnector',
    'SharedCommentsTcpServerChannelAcceptor',
    'does not implement or claim',
    'rustls/tokio-rustls',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-75 plan',
);

console.log('[verify-blog-comments-tcp-host-channel-selection] source contract verified');
