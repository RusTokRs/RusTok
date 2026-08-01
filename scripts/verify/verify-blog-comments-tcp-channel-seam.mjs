import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-channel-seam.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-74.md';
const lockPath = 'Cargo.lock';
const manifestPath = 'crates/rustok-comments/Cargo.toml';
const exportPath = 'crates/rustok-comments/src/lib.rs';
const channelPath = 'crates/rustok-comments/src/tcp_channel.rs';
const protocolPath = 'crates/rustok-comments/src/tcp_protocol.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const delegationPath = 'crates/rustok-comments/src/tcp_delegation.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-channel-seam] ${message}`);
  process.exit(1);
}

function requireCondition(condition, message) {
  if (!condition) fail(message);
}

function requireText(source, fragment, label) {
  requireCondition(source.includes(fragment), `${label} missing ${fragment}`);
}

function forbidText(source, fragment, label) {
  requireCondition(!source.includes(fragment), `${label} contains forbidden ${fragment}`);
}

function sameSet(actual, expected, label) {
  const left = [...actual].sort().join('|');
  const right = [...expected].sort().join('|');
  requireCondition(left === right, `${label} drift: expected ${right}, got ${left}`);
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
  lockPath,
  manifestPath,
  exportPath,
  channelPath,
  protocolPath,
  transportPath,
  serverPath,
  authPath,
  delegationPath,
  runtimePath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const lock = read(lockPath);
const manifest = read(manifestPath);
const exports = read(exportPath);
const channel = read(channelPath);
const protocol = read(protocolPath);
const transport = read(transportPath);
const server = read(serverPath);
const auth = read(authPath);
const delegation = read(delegationPath);
const runtime = read(runtimePath);

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_channel_seam',
  'evidence identity drift',
);
requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence execution status drift',
);
requireCondition(evidence.generated_from === planPath, 'evidence plan path drift');
requireCondition(
  evidence.source_contract?.channel === channelPath
    && evidence.source_contract?.framing === protocolPath
    && evidence.source_contract?.client_transport === transportPath
    && evidence.source_contract?.server_adapter === serverPath
    && evidence.source_contract?.provider_export === exportPath
    && evidence.source_contract?.provider_manifest === manifestPath
    && evidence.source_contract?.lockfile === lockPath,
  'evidence source path drift',
);

requireCondition(
  evidence.channel_io?.trait === 'CommentsTcpIo'
    && evidence.channel_io?.async_read === true
    && evidence.channel_io?.async_write === true
    && evidence.channel_io?.unpin === true
    && evidence.channel_io?.send === true
    && evidence.channel_io?.boxed_alias === 'BoxCommentsTcpIo'
    && evidence.channel_io?.concrete_stream_erased_after_establishment === true,
  'channel I/O evidence drift',
);
sameSet(
  evidence.protection_classification?.values ?? [],
  ['PlaintextLoopback', 'AuthenticatedEncrypted'],
  'protection classification',
);
requireCondition(
  evidence.protection_classification?.classification_is_runtime_evidence === false
    && evidence.protection_classification?.classification_mints_comments_authority === false,
  'classification non-claim drift',
);

requireCondition(
  evidence.client_connector?.trait === 'CommentsTcpClientChannelConnector'
    && evidence.client_connector?.host_injectable === true
    && evidence.client_connector?.establishment_inside_port_deadline === true
    && evidence.client_connector?.default === 'PlaintextLoopbackCommentsTcpChannel',
  'client connector evidence drift',
);
requireCondition(
  evidence.server_acceptor?.trait === 'CommentsTcpServerChannelAcceptor'
    && evidence.server_acceptor?.runs_before_first_frame_read === true
    && evidence.server_acceptor?.concrete_tls_acceptor_must_bound_handshake === true
    && evidence.server_acceptor?.pre_request_timeout_starts_after_channel_establishment === true
    && evidence.server_acceptor?.default === 'PlaintextLoopbackCommentsTcpChannel',
  'server acceptor evidence drift',
);
requireCondition(
  evidence.plaintext_profile?.client_endpoint_loopback_required === true
    && evidence.plaintext_profile?.server_peer_loopback_required === true
    && evidence.plaintext_profile?.non_loopback_error_code
      === 'comments.tcp_plaintext_non_loopback'
    && evidence.plaintext_profile?.host_non_loopback_enabled === false,
  'plaintext profile evidence drift',
);
requireCondition(
  evidence.framing?.generic_async_reader === true
    && evidence.framing?.generic_async_writer === true
    && evidence.framing?.owns_tcp_stream === false
    && evidence.framing?.length_prefix === 'u32_big_endian'
    && evidence.framing?.default_max_frame_bytes === 8388608
    && evidence.framing?.one_request_one_reply === true,
  'framing evidence drift',
);
requireCondition(
  evidence.dependency_contract?.new_direct_dependencies?.length === 0
    && evidence.dependency_contract?.rustok_comments_manifest_changed === false
    && evidence.dependency_contract?.cargo_lock_changed === false
    && evidence.dependency_contract?.rustls_adapter_implemented === false
    && evidence.dependency_contract?.tokio_rustls_adapter_implemented === false,
  'dependency evidence drift',
);
requireCondition(
  evidence.non_claims?.tls_implemented === false
    && evidence.non_claims?.mtls_implemented === false
    && evidence.non_claims?.encrypted_host_profile === false
    && evidence.non_claims?.non_loopback_safe === false
    && evidence.non_claims?.compile_passed === false
    && evidence.non_claims?.source_verifier_executed === false
    && evidence.non_claims?.runtime_executed === false,
  'non-claim evidence drift',
);

for (const fragment of [
  'tcp-transport = ["server", "dep:tokio"]',
  'tokio = { workspace = true, optional = true }',
]) requireText(manifest, fragment, 'comments manifest');
for (const fragment of [
  'dep:rustls',
  'dep:tokio-rustls',
  'rustls =',
  'tokio-rustls =',
  'rustls-pemfile =',
]) forbidText(manifest, fragment, 'comments manifest');
const commentsLock = packageBlock(lock, 'rustok-comments');
for (const fragment of ['"rustls"', '"tokio-rustls"', '"rustls-pemfile"']) {
  forbidText(commentsLock, fragment, 'rustok-comments Cargo.lock entry');
}

for (const fragment of [
  'pub mod tcp_channel;',
  'BoxCommentsTcpIo',
  'CommentsTcpChannelProtection',
  'CommentsTcpClientChannelConnector',
  'CommentsTcpIo',
  'CommentsTcpServerChannelAcceptor',
  'PlaintextLoopbackCommentsTcpChannel',
]) requireText(exports, fragment, 'Comments channel exports');

for (const fragment of [
  'pub trait CommentsTcpIo: AsyncRead + AsyncWrite + Unpin + Send',
  'impl<T> CommentsTcpIo for T where T: AsyncRead + AsyncWrite + Unpin + Send',
  'pub type BoxCommentsTcpIo = Box<dyn CommentsTcpIo>;',
  'pub enum CommentsTcpChannelProtection',
  'PlaintextLoopback',
  'AuthenticatedEncrypted',
  'pub trait CommentsTcpClientChannelConnector: Send + Sync',
  'async fn connect(&self, endpoint: SocketAddr)',
  'pub trait CommentsTcpServerChannelAcceptor: Send + Sync',
  'stream: TcpStream',
  'peer_addr: SocketAddr',
  'pub struct PlaintextLoopbackCommentsTcpChannel;',
  'ensure_loopback(endpoint, "endpoint")?;',
  'ensure_loopback(peer_addr, "peer")?;',
  'comments.tcp_plaintext_non_loopback',
  'stream.set_nodelay(true)',
]) requireText(channel, fragment, 'channel source contract');
for (const fragment of ['rustls::', 'tokio_rustls::', 'native_tls::', 'openssl::']) {
  forbidText(channel, fragment, 'channel concrete crypto non-claim');
}

for (const fragment of [
  'pub(crate) async fn write_frame<S>',
  'S: AsyncWrite + Unpin + ?Sized',
  'pub(crate) async fn read_frame<S>',
  'S: AsyncRead + Unpin + ?Sized',
  'length.to_be_bytes()',
  'u32::from_be_bytes(length_bytes)',
  'comments.tcp_invalid_frame_limit',
  'comments.tcp_frame_too_large',
]) requireText(protocol, fragment, 'generic framing');
forbidText(protocol, 'net::TcpStream', 'generic framing');

for (const fragment of [
  'channel_connector: Arc<dyn CommentsTcpClientChannelConnector>',
  'pub fn with_channel_connector(',
  'pub fn with_channel_connector_and_bearer_token(',
  'pub fn with_channel_connector_bearer_and_delegation(',
  'pub fn with_channel_connector_and_max_frame_bytes(',
  'pub fn channel_protection(&self) -> CommentsTcpChannelProtection',
  'self.channel_connector.connect(self.endpoint).await?',
  'write_frame(&mut *channel, request_payload, self.max_frame_bytes).await?;',
  'read_frame(&mut *channel, self.max_frame_bytes).await?;',
  'request.context().require_deadline_semantics()?;',
  'self.prepare_and_exchange(request)',
  'PlaintextLoopbackCommentsTcpChannel',
  '.field("channel_protection", &self.channel_connector.protection())',
]) requireText(transport, fragment, 'client channel injection');
forbidText(transport, 'TcpStream::connect(self.endpoint)', 'client transport');

for (const fragment of [
  'pub async fn handle_connection(',
  'pub async fn handle_connection_with_pre_request_timeout(',
  'pub async fn handle_connection_with_acceptor(',
  'pub async fn handle_connection_with_acceptor_and_pre_request_timeout(',
  'acceptor: &dyn CommentsTcpServerChannelAcceptor',
  'let channel = acceptor.accept(stream, peer_addr).await?;',
  'mut channel: BoxCommentsTcpIo',
  'channel: &mut dyn CommentsTcpIo',
  'timeout(duration, read_frame(channel, self.max_frame_bytes))',
  '.authorize(peer_addr, operation, credential.as_ref(), &request)',
  'replace_request_context(&mut request, trusted_context);',
  'dispatch_request(self.provider.as_ref(), request).await',
]) requireText(server, fragment, 'server channel injection');

for (const fragment of [
  'CommentsTcpRequestEnvelope::with_bearer(request, token)',
  '[REDACTED]',
]) requireText(auth, fragment, 'retained bearer envelope');
for (const fragment of [
  'CommentsTcpDelegationSigner',
  'comments.tcp_delegation_replayed',
  'PortActor::user(claims.actor_id)',
]) requireText(delegation, fragment, 'retained delegation authority');

for (const fragment of [
  'endpoint.ip().is_loopback()',
  'bind_addr.ip().is_loopback()',
  'RUSTOK_COMMENTS_TCP_BEARER_TOKEN',
  'RUSTOK_COMMENTS_TCP_DELEGATION_SECRET',
]) requireText(runtime, fragment, 'retained host restrictions');
for (const fragment of ['CommentsTcpChannelProtection::AuthenticatedEncrypted', 'tokio_rustls']) {
  forbidText(runtime, fragment, 'host encrypted-profile non-claim');
}

for (const fragment of [
  '## Slice 74 — protected channel injection core',
  'It deliberately does not claim that TLS or',
  '`comments.tcp_plaintext_non_loopback`',
  'The concrete rustls client connector, server acceptor,',
  'Status: `source_verified_no_compile`.',
  'Compile policy: `not_run_by_request`.',
  'Runtime status: `not_run`.',
]) requireText(plan, fragment, 'slice-74 plan');

console.log('[verify-blog-comments-tcp-channel-seam] source contract verified');
