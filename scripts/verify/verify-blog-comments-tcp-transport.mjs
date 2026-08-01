import fs from 'node:fs';

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json';
const manifestPath = 'crates/rustok-comments/Cargo.toml';
const exportPath = 'crates/rustok-comments/src/lib.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const channelPath = 'crates/rustok-comments/src/tcp_channel.rs';
const protocolPath = 'crates/rustok-comments/src/tcp_protocol.rs';
const remotePath = 'crates/rustok-comments/src/remote.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-68.md';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-transport] ${message}`);
  process.exit(1);
}

function requireCondition(condition, message) {
  if (!condition) fail(message);
}

function requireText(source, fragment, label) {
  requireCondition(source.includes(fragment), `${label} missing ${fragment}`);
}

function sameSet(actual, expected, label) {
  const left = [...actual].sort().join('|');
  const right = [...expected].sort().join('|');
  requireCondition(left === right, `${label} drift: expected ${right}, got ${left}`);
}

for (const path of [
  evidencePath,
  manifestPath,
  exportPath,
  authPath,
  channelPath,
  protocolPath,
  remotePath,
  transportPath,
  planPath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const manifest = read(manifestPath);
const exports = read(exportPath);
const auth = read(authPath);
const channel = read(channelPath);
const protocol = read(protocolPath);
const remote = read(remotePath);
const transport = read(transportPath);
const plan = read(planPath);

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_json_transport',
  'evidence identity drift',
);
requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence execution status drift',
);
requireCondition(evidence.generated_from === planPath, 'historical plan path drift');

sameSet(
  evidence.operations ?? [],
  [
    'CreateComment',
    'GetComment',
    'ListCommentsForTarget',
    'ListPublicCommentsForTarget',
    'UpdateComment',
    'SetCommentStatus',
    'DeleteComment',
  ],
  'transport operations',
);
requireCondition(
  evidence.protocol?.length_prefix === 'u32_big_endian'
    && evidence.protocol?.request_encoding === 'serde_json'
    && evidence.protocol?.reply_encoding === 'serde_json'
    && evidence.protocol?.connection_scope === 'one_request_one_reply'
    && evidence.protocol?.default_max_frame_bytes === 8388608,
  'historical protocol drift',
);
requireCondition(
  evidence.deadline?.source === 'PortContext.deadline_ms'
    && evidence.deadline?.required_before_connect === true
    && evidence.deadline?.mechanism === 'tokio::time::timeout'
    && evidence.deadline?.timeout_code === 'comments.tcp_timeout',
  'historical deadline drift',
);
requireCondition(evidence.fail_closed?.retry_implemented === false, 'retry status drift');

for (const fragment of [
  'tcp-transport = ["server", "dep:tokio"]',
  'tokio = { workspace = true, optional = true }',
]) requireText(manifest, fragment, 'comments manifest');
for (const forbidden of ['dep:rustls', 'dep:tokio-rustls', 'rustls =', 'tokio-rustls =']) {
  requireCondition(!manifest.includes(forbidden), `comments manifest contains ${forbidden}`);
}

for (const fragment of [
  'pub mod tcp_auth;',
  'pub mod tcp_channel;',
  'mod tcp_protocol;',
  'pub mod tcp_transport;',
  'CommentsTcpClientChannelConnector',
  'PlaintextLoopbackCommentsTcpChannel',
  'TcpJsonCommentsTransport',
]) requireText(exports, fragment, 'comments exports');

for (const fragment of [
  'pub const COMMENTS_TCP_PROTOCOL_VERSION: u16 = 1;',
  'pub struct CommentsTcpRequestEnvelope',
  'CommentsTcpRequestEnvelope',
  '[REDACTED]',
]) requireText(auth, fragment, 'authentication envelope');

for (const fragment of [
  'pub trait CommentsTcpIo: AsyncRead + AsyncWrite + Unpin + Send',
  'pub trait CommentsTcpClientChannelConnector: Send + Sync',
  'async fn connect(&self, endpoint: SocketAddr)',
  'PlaintextLoopback',
  'AuthenticatedEncrypted',
  'ensure_loopback(endpoint, "endpoint")?;',
  'comments.tcp_plaintext_non_loopback',
]) requireText(channel, fragment, 'channel connector');

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
requireCondition(!protocol.includes('net::TcpStream'), 'framing must not own TcpStream');

for (const fragment of [
  'pub enum CommentsThreadRequest',
  'pub enum CommentsThreadTransportReply',
  'Success(CommentsThreadResponse)',
  'Error(PortError)',
]) requireText(remote, fragment, 'remote typed core');

for (const fragment of [
  'channel_connector: Arc<dyn CommentsTcpClientChannelConnector>',
  'pub fn with_channel_connector(',
  'pub fn with_channel_connector_and_bearer_token(',
  'pub fn with_channel_connector_bearer_and_delegation(',
  'self.channel_connector.connect(self.endpoint).await?',
  'write_frame(&mut *channel, request_payload, self.max_frame_bytes).await?;',
  'read_frame(&mut *channel, self.max_frame_bytes).await?;',
  'request.context().require_deadline_semantics()?;',
  'Duration::from_millis(deadline_ms)',
  'self.prepare_and_exchange(request)',
  'CommentsTcpRequestEnvelope::with_bearer(request, token)',
  'CommentsThreadTransportReply::Success(response)',
  'CommentsThreadTransportReply::Error(error)',
  'pub fn channel_protection(&self) -> CommentsTcpChannelProtection',
  'default_transport_is_plaintext_loopback',
]) requireText(transport, fragment, 'TCP transport');
for (const forbidden of ['TcpStream::connect(self.endpoint)', 'retry(', 'println!(', 'tracing::info!']) {
  requireCondition(!transport.includes(forbidden), `transport contains forbidden ${forbidden}`);
}

for (const fragment of [
  '# rustok-blog implementation plan — slice 68 continuation',
  'comments_tcp_length_prefixed_json_v1',
  'source_verified_no_compile',
  'retry_backoff',
  'not_run_by_request',
]) requireText(plan, fragment, 'slice 68 plan');

console.log('[verify-blog-comments-tcp-transport] retained transport contract verified');
