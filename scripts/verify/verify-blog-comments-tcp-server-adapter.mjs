import fs from 'node:fs';

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json';
const exportPath = 'crates/rustok-comments/src/lib.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const channelPath = 'crates/rustok-comments/src/tcp_channel.rs';
const protocolPath = 'crates/rustok-comments/src/tcp_protocol.rs';
const remotePath = 'crates/rustok-comments/src/remote.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-69.md';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-server-adapter] ${message}`);
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
  exportPath,
  authPath,
  channelPath,
  protocolPath,
  remotePath,
  serverPath,
  transportPath,
  planPath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const exports = read(exportPath);
const auth = read(authPath);
const channel = read(channelPath);
const protocol = read(protocolPath);
const remote = read(remotePath);
const server = read(serverPath);
const transport = read(transportPath);
const plan = read(planPath);

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_server_adapter',
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
    'create_comment',
    'get_comment',
    'list_comments_for_target',
    'list_public_comments_for_target',
    'update_comment',
    'set_comment_status',
    'delete_comment',
  ],
  'server operations',
);
requireCondition(
  evidence.protocol?.length_prefix === 'u32_big_endian'
    && evidence.protocol?.default_max_frame_bytes === 8388608
    && evidence.protocol?.shared_client_server_framing === true,
  'historical protocol drift',
);
requireCondition(
  evidence.authority?.required === true
    && evidence.authority?.allow_all_fallback === false
    && evidence.authority?.tenant_match_required === true,
  'authority evidence drift',
);

for (const fragment of [
  'pub trait CommentsTcpServerChannelAcceptor: Send + Sync',
  'async fn accept(',
  'PlaintextLoopbackCommentsTcpChannel',
  'comments.tcp_plaintext_non_loopback',
]) requireText(channel, fragment, 'server channel seam');

for (const fragment of [
  'pub(crate) async fn write_frame<S>',
  'S: AsyncWrite + Unpin + ?Sized',
  'pub(crate) async fn read_frame<S>',
  'S: AsyncRead + Unpin + ?Sized',
  'length.to_be_bytes()',
  'u32::from_be_bytes(length_bytes)',
]) requireText(protocol, fragment, 'generic framing');
requireCondition(!protocol.includes('net::TcpStream'), 'framing must not own TcpStream');

for (const fragment of [
  'pub const COMMENTS_TCP_PROTOCOL_VERSION: u16 = 1;',
  'pub struct CommentsTcpCredential',
  'pub struct CommentsTcpRequestEnvelope',
  'comments.tcp_authentication_failed',
  'comments.tcp_operation_forbidden',
]) requireText(auth, fragment, 'authentication contract');

for (const fragment of [
  'pub enum CommentsTcpOperation',
  'pub const ALL: [Self; 7]',
  'pub struct TrustedCommentsTcpAuthority',
  'pub trait CommentsTcpAuthorityResolver: Send + Sync',
  'pub struct TcpJsonCommentsServerAdapter',
  'authority: Arc<dyn CommentsTcpAuthorityResolver>',
  'pub async fn handle_connection(',
  'pub async fn handle_connection_with_acceptor(',
  'pub async fn handle_connection_with_acceptor_and_pre_request_timeout(',
  'let channel = acceptor.accept(stream, peer_addr).await?;',
  'mut channel: BoxCommentsTcpIo',
  'channel: &mut dyn CommentsTcpIo',
  'serde_json::from_slice::<CommentsTcpRequestEnvelope>',
  'protocol_version != COMMENTS_TCP_PROTOCOL_VERSION',
  'request.context().require_deadline_semantics()?;',
  '.authorize(peer_addr, operation, credential.as_ref(), &request)',
  'apply_authority(request.context(), authority)?',
  'replace_request_context(&mut request, trusted_context);',
  'dispatch_request(self.provider.as_ref(), request).await',
  'comments.tcp_authority_tenant_mismatch',
  'comments.tcp_server_timeout',
  'comments.tcp_server_invalid_request',
]) requireText(server, fragment, 'TCP server adapter');

for (const variant of [
  'CreateComment',
  'GetComment',
  'ListCommentsForTarget',
  'ListPublicCommentsForTarget',
  'UpdateComment',
  'SetCommentStatus',
  'DeleteComment',
]) {
  requireCondition(
    server.includes(`CommentsThreadRequest::${variant}`),
    `server dispatch missing ${variant}`,
  );
}
for (const forbidden of ['TcpListener', '.accept().await', 'AllowAll', 'retry(']) {
  requireCondition(!server.includes(forbidden), `server contains forbidden ${forbidden}`);
}

for (const fragment of [
  'pub mod tcp_channel;',
  'pub mod tcp_server;',
  'CommentsTcpServerChannelAcceptor',
  'PlaintextLoopbackCommentsTcpChannel',
  'TcpJsonCommentsServerAdapter',
]) requireText(exports, fragment, 'Comments exports');

for (const fragment of [
  'pub enum CommentsThreadRequest',
  'pub enum CommentsThreadResponse',
  'pub enum CommentsThreadTransportReply',
]) requireText(remote, fragment, 'remote envelopes');

for (const fragment of [
  'channel_connector: Arc<dyn CommentsTcpClientChannelConnector>',
  'write_frame(&mut *channel, request_payload, self.max_frame_bytes).await?;',
  'read_frame(&mut *channel, self.max_frame_bytes).await?;',
]) requireText(transport, fragment, 'client shared framing');

for (const fragment of [
  '# rustok-blog implementation plan — slice 69 continuation',
  'CommentsTcpAuthorityResolver',
  'source_verified_no_compile',
  'host runtime configuration',
  'not_run_by_request',
]) requireText(plan, fragment, 'slice 69 plan');

console.log('[verify-blog-comments-tcp-server-adapter] retained server contract verified');
