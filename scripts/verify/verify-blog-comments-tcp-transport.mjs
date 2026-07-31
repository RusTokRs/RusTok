import fs from 'node:fs';

const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-tcp-transport.json';
const manifestPath = 'crates/rustok-comments/Cargo.toml';
const exportPath = 'crates/rustok-comments/src/lib.rs';
const protocolPath = 'crates/rustok-comments/src/tcp_protocol.rs';
const remotePath = 'crates/rustok-comments/src/remote.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-68.md';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function json(path) {
  return JSON.parse(read(path));
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-transport] ${message}`);
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
  manifestPath,
  exportPath,
  protocolPath,
  remotePath,
  transportPath,
  planPath,
]) {
  if (!fs.existsSync(path)) fail(`missing source artifact ${path}`);
}

const evidence = json(evidencePath);
const manifest = read(manifestPath);
const exports = read(exportPath);
const protocol = read(protocolPath);
const remote = read(remotePath);
const transport = read(transportPath);
const plan = read(planPath);

if (evidence.schema_version !== 1) fail('evidence schema_version drift');
if (
  evidence.module !== 'blog'
  || evidence.provider !== 'comments'
  || evidence.surface !== 'comments_tcp_json_transport'
) fail('evidence identity drift');
if (
  evidence.status !== 'source_verified_no_compile'
  || evidence.compile_policy !== 'not_run_by_request'
  || evidence.runtime_status !== 'not_run'
) fail('evidence execution status drift');
if (evidence.generated_from !== planPath) fail('evidence plan path drift');

const expectedOperations = [
  'CreateComment',
  'GetComment',
  'ListCommentsForTarget',
  'ListPublicCommentsForTarget',
  'UpdateComment',
  'SetCommentStatus',
  'DeleteComment',
];
sameSet(evidence.operations ?? [], expectedOperations, 'transport operations');

if (
  evidence.source_contract?.remote_core !== remotePath
  || evidence.source_contract?.transport_adapter !== transportPath
  || evidence.source_contract?.provider_export !== exportPath
  || evidence.source_contract?.provider_manifest !== manifestPath
) fail('source contract path drift');

if (
  evidence.protocol?.name !== 'comments_tcp_length_prefixed_json_v1'
  || evidence.protocol?.length_prefix !== 'u32_big_endian'
  || evidence.protocol?.request_encoding !== 'serde_json'
  || evidence.protocol?.reply_encoding !== 'serde_json'
  || evidence.protocol?.connection_scope !== 'one_request_one_reply'
  || evidence.protocol?.default_max_frame_bytes !== 8388608
  || evidence.protocol?.endpoint_owner !== 'host'
) fail('protocol contract drift');

if (
  evidence.deadline?.source !== 'PortContext.deadline_ms'
  || evidence.deadline?.required_before_connect !== true
  || evidence.deadline?.scope !== 'connect_write_read_decode'
  || evidence.deadline?.mechanism !== 'tokio::time::timeout'
  || evidence.deadline?.timeout_code !== 'comments.tcp_timeout'
) fail('deadline contract drift');

if (evidence.fail_closed?.retry_implemented !== false) fail('retry status drift');

sameSet(
  evidence.pending ?? [],
  [
    'tcp_server_adapter',
    'endpoint_discovery',
    'authentication',
    'retry_backoff',
    'host_publication',
    'in_process_remote_runtime_parity',
    'runtime_execution',
  ],
  'historical slice 68 pending scope',
);

hasAll(
  manifest,
  [
    'tcp-transport = ["server", "dep:tokio"]',
    'tokio = { workspace = true, optional = true }',
  ],
  'comments manifest',
);

hasAll(
  exports,
  [
    '#[cfg(feature = "tcp-transport")]',
    'mod tcp_protocol;',
    'pub mod tcp_transport;',
    'pub use tcp_transport::TcpJsonCommentsTransport;',
  ],
  'comments exports',
);

hasAll(
  remote,
  [
    'pub fn context(&self) -> &PortContext',
    'pub enum CommentsThreadTransportReply',
    'Success(CommentsThreadResponse)',
    'Error(PortError)',
  ],
  'remote core',
);
for (const operation of expectedOperations) {
  if (!remote.includes(`Self::${operation} { context, .. }`)) {
    fail(`remote request context coverage missing ${operation}`);
  }
}

hasAll(
  protocol,
  [
    'pub const DEFAULT_MAX_COMMENTS_FRAME_BYTES: usize = 8 * 1024 * 1024;',
    'length.to_be_bytes()',
    'u32::from_be_bytes(length_bytes)',
    'comments.tcp_invalid_frame_limit',
    'comments.tcp_frame_too_large',
    'comments.tcp_unavailable',
    'comments.tcp_timeout',
  ],
  'shared TCP protocol',
);

hasAll(
  transport,
  [
    'pub struct TcpJsonCommentsTransport',
    'impl CommentsThreadTransport for TcpJsonCommentsTransport',
    'TcpStream::connect(self.endpoint)',
    'request.context().require_deadline_semantics()?;',
    'Duration::from_millis(deadline_ms)',
    'self.exchange(&request_payload)',
    'write_frame(&mut stream, request_payload, self.max_frame_bytes).await?;',
    'read_frame(&mut stream, self.max_frame_bytes).await?;',
    'CommentsThreadTransportReply::Success(response)',
    'CommentsThreadTransportReply::Error(error)',
    'comments.tcp_encode',
    'comments.tcp_decode',
    'provider_error_reply_is_preserved',
    'tcp_transport_is_injectable_without_connecting',
  ],
  'TCP transport',
);

hasNone(
  transport,
  [
    'loop {',
    'retry(',
    'Authorization',
    'Bearer ',
  ],
  'TCP transport non-claims',
);

hasAll(
  plan,
  [
    '# rustok-blog implementation plan — slice 68 continuation',
    'comments_tcp_length_prefixed_json_v1',
    'source_verified_no_compile',
    'tcp_server_adapter',
    'host_publication',
    'retry_backoff',
    'not_run_by_request',
  ],
  'slice 68 plan',
);

console.log('[verify-blog-comments-tcp-transport] source contract verified');
