import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-bearer-auth.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-72.md';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const delegationPath = 'crates/rustok-comments/src/tcp_delegation.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const digestPath = 'crates/rustok-api/src/digest.rs';
const manifestPath = 'crates/rustok-comments/Cargo.toml';
const lockPath = 'Cargo.lock';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-bearer-auth] ${message}`);
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

function packageBlock(lock, name) {
  const marker = `[[package]]\nname = "${name}"\n`;
  const start = lock.indexOf(marker);
  requireCondition(start >= 0, `Cargo.lock missing ${name}`);
  const next = lock.indexOf('\n[[package]]', start + marker.length);
  return lock.slice(start, next < 0 ? lock.length : next);
}

for (const path of [
  evidencePath,
  planPath,
  authPath,
  delegationPath,
  serverPath,
  transportPath,
  runtimePath,
  digestPath,
  manifestPath,
  lockPath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const auth = read(authPath);
const delegation = read(delegationPath);
const server = read(serverPath);
const transport = read(transportPath);
const runtime = read(runtimePath);
const digest = read(digestPath);
const manifest = read(manifestPath);
const lock = read(lockPath);

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_bearer_auth',
  'evidence identity drift',
);
requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'historical execution status drift',
);
requireCondition(evidence.generated_from === planPath, 'historical plan path drift');

const reads = [
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
];
const writes = [
  'create_comment',
  'update_comment',
  'set_comment_status',
  'delete_comment',
];
sameSet(evidence.authority?.default_allowed_operations ?? [], reads, 'bearer reads');
sameSet(evidence.authority?.default_denied_operations ?? [], writes, 'bearer writes');
requireCondition(
  evidence.authority?.all_operations_default === false
    && evidence.authority?.trusted_user_delegation_implemented === false,
  'slice-72 historical authority evidence drift',
);
requireCondition(
  evidence.host_configuration?.plaintext_loopback_only === true
    && evidence.host_configuration?.non_loopback_enabled === false
    && evidence.host_configuration?.allow_all_fallback === false,
  'historical host boundary drift',
);

for (const fragment of [
  'const BEARER_SCHEME: &str = "bearer";',
  'const DEFAULT_BEARER_OPERATIONS: [CommentsTcpOperation; 3]',
  'CommentsTcpOperation::GetComment',
  'CommentsTcpOperation::ListCommentsForTarget',
  'CommentsTcpOperation::ListPublicCommentsForTarget',
  'HashSet::from(DEFAULT_BEARER_OPERATIONS)',
  'comments.tcp_operation_forbidden',
  'comments.tcp_authentication_failed',
  'fixed_work_sha256_eq(&self.authorization_digest, &candidate_digest)',
  'sha256_digest(&[BEARER_PREFIX, secret])',
  '[REDACTED]',
]) {
  requireText(auth, fragment, 'bearer authentication');
}
requireCondition(
  !auth.includes('HashSet::from(CommentsTcpOperation::ALL)'),
  'bearer resolver must not become allow-all',
);

requireText(delegation, 'pub struct CommentsTcpDelegatingAuthorityResolver', 'separate delegation');
requireText(
  delegation,
  '.with_allowed_operations(COMPOSITE_SERVICE_OPERATIONS)',
  'separate delegation service authority',
);
requireCondition(
  !delegation.includes('HashSet::from(CommentsTcpOperation::ALL)'),
  'delegation composition must not silently widen bearer default',
);

for (const fragment of [
  'credential: Option<&CommentsTcpCredential>',
  'request: &CommentsThreadRequest',
  'let operation = CommentsTcpOperation::for_request(&request);',
  '.authorize(peer_addr, operation, credential.as_ref(), &request)',
  'apply_authority(request.context(), authority)?',
  'replace_request_context(&mut request, trusted_context);',
]) {
  requireText(server, fragment, 'server bearer/delegation seam');
}

for (const fragment of [
  'bearer_token: Option<CommentsTcpBearerToken>',
  'CommentsTcpRequestEnvelope::with_bearer(request, token)',
  'pub fn is_authenticated(&self) -> bool',
  'bearer_transport_debug_is_redacted',
]) {
  requireText(transport, fragment, 'bearer client path');
}

for (const fragment of [
  'RUSTOK_COMMENTS_TCP_BEARER_TOKEN',
  'RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID',
  'CommentsTcpBearerAuthorityResolver::from_token(token, actor)',
  '.with_claim(COMMENTS_TCP_SERVICE_PERMISSION)',
  '.with_role(COMMENTS_TCP_SERVICE_ROLE)',
  'endpoint.ip().is_loopback()',
  'bind_addr.ip().is_loopback()',
]) {
  requireText(runtime, fragment, 'host bearer fallback');
}
requireCondition(
  !runtime.includes('struct AllowAllCommentsTcpAuthority'),
  'host must not introduce allow-all bearer authority',
);

for (const fragment of [
  'pub fn sha256_digest(',
  'pub fn fixed_work_sha256_eq(',
  'does not claim',
]) {
  requireText(digest, fragment, 'shared digest contract');
}

requireText(manifest, 'tcp-transport = ["server", "dep:tokio"]', 'comments manifest');
requireCondition(!manifest.includes('dep:sha2'), 'comments must not gain direct SHA-256 dependency');
requireCondition(!manifest.includes('dep:subtle'), 'comments must not gain direct subtle dependency');
const commentsLock = packageBlock(lock, 'rustok-comments');
requireCondition(!commentsLock.includes('"sha2'), 'comments lock entry gained sha2');
requireCondition(!commentsLock.includes('"subtle"'), 'comments lock entry gained subtle');

for (const fragment of [
  '## Slice 72 — authenticated Comments TCP bearer envelope',
  'The resolver\'s safe default admits exactly three read operations',
  'Plaintext bearer mode remains loopback-only',
  'Status: `source_verified_no_compile`.',
  'Compile policy: `not_run_by_request`.',
  'Runtime status: `not_run`.',
]) {
  requireText(plan, fragment, 'slice-72 plan');
}

console.log('[verify-blog-comments-tcp-bearer-auth] retained bearer contract verified');
