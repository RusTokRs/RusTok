import fs from 'node:fs';

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json';
const exportPath = 'crates/rustok-comments/src/lib.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
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

function hasAll(text, snippets, label) {
  for (const snippet of snippets) {
    if (!text.includes(snippet)) fail(`${label} missing ${snippet}`);
  }
}

function hasNone(text, snippets, label) {
  for (const snippet of snippets) {
    if (text.includes(snippet)) fail(`${label} contains forbidden ${snippet}`);
  }
}

function sameSet(actual, expected, label) {
  const left = [...actual].sort().join('|');
  const right = [...expected].sort().join('|');
  if (left !== right) fail(`${label} drift: expected ${right}, got ${left}`);
}

for (const path of [
  evidencePath,
  exportPath,
  authPath,
  protocolPath,
  remotePath,
  serverPath,
  transportPath,
  planPath,
]) {
  if (!fs.existsSync(path)) fail(`missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const exports = read(exportPath);
const auth = read(authPath);
const protocol = read(protocolPath);
const remote = read(remotePath);
const server = read(serverPath);
const transport = read(transportPath);
const plan = read(planPath);

if (evidence.schema_version !== 1) fail('evidence schema_version drift');
if (
  evidence.module !== 'blog'
  || evidence.provider !== 'comments'
  || evidence.surface !== 'comments_tcp_server_adapter'
) fail('evidence identity drift');
if (
  evidence.status !== 'source_verified_no_compile'
  || evidence.compile_policy !== 'not_run_by_request'
  || evidence.runtime_status !== 'not_run'
) fail('evidence execution status drift');
if (evidence.generated_from !== planPath) fail('evidence plan path drift');

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

if (
  evidence.source_contract?.server_adapter !== serverPath
  || evidence.source_contract?.shared_protocol !== protocolPath
  || evidence.source_contract?.client_transport !== transportPath
  || evidence.source_contract?.provider_export !== exportPath
) fail('source contract path drift');

if (
  evidence.protocol?.identity !== 'comments_tcp_length_prefixed_json_v1'
  || evidence.protocol?.length_prefix !== 'u32_big_endian'
  || evidence.protocol?.default_max_frame_bytes !== 8388608
  || evidence.protocol?.shared_client_server_framing !== true
) fail('protocol evidence drift');

if (
  evidence.authority?.required !== true
  || evidence.authority?.allow_all_fallback !== false
  || evidence.authority?.tenant_match_required !== true
) fail('authority evidence drift');

hasAll(
  protocol,
  [
    'pub const DEFAULT_MAX_COMMENTS_FRAME_BYTES: usize = 8 * 1024 * 1024;',
    'length.to_be_bytes()',
    'u32::from_be_bytes(length_bytes)',
    'comments.tcp_invalid_frame_limit',
    'comments.tcp_frame_too_large',
  ],
  'shared TCP protocol',
);

hasAll(
  auth,
  [
    'pub const COMMENTS_TCP_PROTOCOL_VERSION: u16 = 1;',
    'pub struct CommentsTcpCredential',
    'pub struct CommentsTcpRequestEnvelope',
    'pub struct CommentsTcpBearerAuthorityResolver',
    'comments.tcp_authentication_failed',
    'comments.tcp_operation_forbidden',
  ],
  'TCP authentication contract',
);

hasAll(
  server,
  [
    'pub enum CommentsTcpOperation',
    'pub const ALL: [Self; 7]',
    'pub struct TrustedCommentsTcpAuthority',
    'pub trait CommentsTcpAuthorityResolver: Send + Sync',
    'credential: Option<&CommentsTcpCredential>',
    'pub struct TcpJsonCommentsServerAdapter',
    'authority: Arc<dyn CommentsTcpAuthorityResolver>',
    'pub async fn handle_connection(',
    'CommentsThreadTransportReply::Success(response)',
    'CommentsThreadTransportReply::Error(error)',
    'serde_json::from_slice::<CommentsTcpRequestEnvelope>',
    'envelope.into_parts()',
    'protocol_version != COMMENTS_TCP_PROTOCOL_VERSION',
    'comments.tcp_server_unsupported_protocol',
    'request.context().require_deadline_semantics()?;',
    'credential.as_ref()',
    'comments.tcp_authority_tenant_mismatch',
    'trusted.actor = authority.actor;',
    'trusted.claims = authority.claims;',
    'trusted.roles = authority.roles;',
    'dispatch_request(self.provider.as_ref(), request).await',
    'comments.tcp_server_timeout',
    'comments.tcp_server_invalid_request',
  ],
  'TCP server adapter',
);

for (const variant of [
  'CreateComment',
  'GetComment',
  'ListCommentsForTarget',
  'ListPublicCommentsForTarget',
  'UpdateComment',
  'SetCommentStatus',
  'DeleteComment',
]) {
  if (!server.includes(`CommentsThreadRequest::${variant}`)) {
    fail(`server dispatch missing ${variant}`);
  }
}

hasNone(
  server,
  [
    'TcpListener',
    '.accept().await',
    'loop {',
    'AllowAll',
    'retry(',
  ],
  'server adapter non-claims',
);

hasAll(
  exports,
  [
    'pub mod tcp_auth;',
    'mod tcp_protocol;',
    'pub mod tcp_server;',
    'pub mod tcp_transport;',
    'CommentsTcpBearerAuthorityResolver',
    'CommentsTcpRequestEnvelope',
    'CommentsTcpAuthorityResolver, CommentsTcpOperation, TcpJsonCommentsServerAdapter,',
    'TrustedCommentsTcpAuthority,',
  ],
  'Comments exports',
);

hasAll(
  remote,
  [
    'pub enum CommentsThreadRequest',
    'pub enum CommentsThreadResponse',
    'pub enum CommentsThreadTransportReply',
  ],
  'remote envelopes',
);

hasAll(
  transport,
  [
    'use crate::tcp_protocol::{',
    'pub use crate::tcp_protocol::DEFAULT_MAX_COMMENTS_FRAME_BYTES;',
    'CommentsTcpRequestEnvelope::with_bearer(request, token)',
    'write_frame(&mut stream, request_payload, self.max_frame_bytes).await?;',
    'read_frame(&mut stream, self.max_frame_bytes).await?;',
  ],
  'TCP client shared framing',
);

hasAll(
  plan,
  [
    '# rustok-blog implementation plan — slice 69 continuation',
    'CommentsTcpAuthorityResolver',
    'source_verified_no_compile',
    'host runtime configuration',
    'not_run_by_request',
  ],
  'slice 69 plan',
);

console.log('[verify-blog-comments-tcp-server-adapter] source contract verified');
