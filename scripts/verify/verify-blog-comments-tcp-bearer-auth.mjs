import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-bearer-auth.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-72.md';
const lockPath = 'Cargo.lock';
const apiManifestPath = 'crates/rustok-api/Cargo.toml';
const digestPath = 'crates/rustok-api/src/digest.rs';
const apiExportPath = 'crates/rustok-api/src/lib.rs';
const commentsManifestPath = 'crates/rustok-comments/Cargo.toml';
const commentsExportPath = 'crates/rustok-comments/src/lib.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';

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

function hasAll(source, fragments, label) {
  for (const fragment of fragments) {
    requireCondition(
      source.includes(fragment),
      `${label} missing source fragment: ${fragment}`,
    );
  }
}

function hasNone(source, fragments, label) {
  for (const fragment of fragments) {
    requireCondition(
      !source.includes(fragment),
      `${label} contains forbidden source fragment: ${fragment}`,
    );
  }
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
  apiManifestPath,
  digestPath,
  apiExportPath,
  commentsManifestPath,
  commentsExportPath,
  authPath,
  serverPath,
  transportPath,
  runtimePath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const lock = read(lockPath);
const apiManifest = read(apiManifestPath);
const digest = read(digestPath);
const apiExports = read(apiExportPath);
const commentsManifest = read(commentsManifestPath);
const commentsExports = read(commentsExportPath);
const auth = read(authPath);
const server = read(serverPath);
const transport = read(transportPath);
const runtime = read(runtimePath);

requireCondition(evidence.schema_version === 1, 'evidence schema must remain version 1');
requireCondition(evidence.module === 'blog', 'evidence module must remain blog');
requireCondition(evidence.provider === 'comments', 'evidence provider must remain comments');
requireCondition(
  evidence.surface === 'comments_tcp_bearer_auth',
  'evidence surface must remain comments_tcp_bearer_auth',
);
requireCondition(
  evidence.status === 'source_verified_no_compile'
    && evidence.compile_policy === 'not_run_by_request'
    && evidence.runtime_status === 'not_run',
  'evidence execution status drift',
);
requireCondition(evidence.generated_from === planPath, 'evidence plan path drift');

requireCondition(
  evidence.source_contract?.authentication === authPath
    && evidence.source_contract?.client_transport === transportPath
    && evidence.source_contract?.server_adapter === serverPath
    && evidence.source_contract?.host_runtime === runtimePath
    && evidence.source_contract?.provider_export === commentsExportPath
    && evidence.source_contract?.provider_manifest === commentsManifestPath
    && evidence.source_contract?.digest_helper === digestPath
    && evidence.source_contract?.digest_export === apiExportPath,
  'evidence source contract path drift',
);

requireCondition(
  evidence.wire_envelope?.protocol_version === 1
    && evidence.wire_envelope?.version_field === 'protocol_version'
    && evidence.wire_envelope?.credential_field === 'credential'
    && evidence.wire_envelope?.request_field === 'request'
    && evidence.wire_envelope?.encoding === 'serde_json'
    && evidence.wire_envelope?.length_prefix === 'u32_big_endian'
    && evidence.wire_envelope?.one_request_per_connection === true
    && evidence.wire_envelope?.unsupported_version_code
      === 'comments.tcp_server_unsupported_protocol'
    && evidence.wire_envelope?.credential_copied_to_port_context === false,
  'wire envelope evidence drift',
);

requireCondition(
  evidence.credential?.scheme === 'bearer'
    && evidence.credential?.minimum_bytes === 1
    && evidence.credential?.maximum_bytes === 4096
    && evidence.credential?.ascii_only === true
    && evidence.credential?.whitespace_allowed === false
    && evidence.credential?.control_bytes_allowed === false
    && evidence.credential?.debug_redacted === true
    && evidence.credential?.client_transport_debug_redacted === true
    && evidence.credential?.raw_secret_logged === false,
  'credential evidence drift',
);

requireCondition(
  evidence.comparison?.digest === 'sha256'
    && evidence.comparison?.digest_bytes === 32
    && evidence.comparison?.digest_function === 'rustok_api::sha256_digest'
    && evidence.comparison?.fixed_work_digest_compare
      === 'rustok_api::fixed_work_sha256_eq'
    && evidence.comparison?.digest_compare_early_return === false
    && evidence.comparison?.digest_compare_length_dependent_loop === false
    && evidence.comparison?.compiler_hardware_side_channel_audit === false,
  'digest comparison evidence drift',
);

requireCondition(
  Array.isArray(evidence.dependency_contract?.rustok_comments_new_direct_dependencies)
    && evidence.dependency_contract.rustok_comments_new_direct_dependencies.length === 0
    && evidence.dependency_contract?.rustok_comments_manifest_restored_to_pre_slice_set === true
    && evidence.dependency_contract?.cargo_lock_package_entry_change_required === false
    && evidence.dependency_contract?.sha256_dependency_owner === 'rustok-api',
  'dependency evidence drift',
);

const defaultReads = [
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
];
const defaultWrites = [
  'create_comment',
  'update_comment',
  'set_comment_status',
  'delete_comment',
];
const allOperations = [...defaultReads, ...defaultWrites];

requireCondition(
  evidence.authority?.peer_loopback_required === true
    && evidence.authority?.canonical_uuid_tenant_required === true
    && evidence.authority?.operation_allowlist === true
    && evidence.authority?.all_operations_default === false
    && evidence.authority?.operation_denied_code === 'comments.tcp_operation_forbidden'
    && evidence.authority?.authentication_failure_code
      === 'comments.tcp_authentication_failed'
    && evidence.authority?.authentication_failure_message
      === 'Comments TCP service authentication failed'
    && evidence.authority?.missing_and_wrong_token_indistinguishable === true
    && evidence.authority?.tenant_match_after_authentication === true
    && evidence.authority?.trusted_user_delegation_implemented === false,
  'authority evidence drift',
);
sameSet(
  evidence.authority?.default_allowed_operations ?? [],
  defaultReads,
  'default bearer read operations',
);
sameSet(
  evidence.authority?.default_denied_operations ?? [],
  defaultWrites,
  'default bearer denied writes',
);
sameSet(evidence.protocol_operations ?? [], allOperations, 'protocol operations');

requireCondition(
  evidence.host_configuration?.consumer_tcp_requires_bearer === true
    && evidence.host_configuration?.listener_bearer_environment
      === 'RUSTOK_COMMENTS_TCP_BEARER_TOKEN'
    && evidence.host_configuration?.listener_service_actor_environment
      === 'RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID'
    && evidence.host_configuration?.service_actor_kind === 'service'
    && evidence.host_configuration?.service_actor_uuid_required === true
    && evidence.host_configuration?.built_in_claims?.length === 1
    && evidence.host_configuration.built_in_claims[0] === 'comments:manage'
    && evidence.host_configuration?.built_in_roles?.length === 1
    && evidence.host_configuration.built_in_roles[0] === 'admin'
    && evidence.host_configuration?.external_authority_override_precedence === true
    && evidence.host_configuration?.allow_all_fallback === false
    && evidence.host_configuration?.plaintext_loopback_only === true
    && evidence.host_configuration?.non_loopback_enabled === false,
  'host configuration evidence drift',
);
sameSet(
  evidence.host_configuration?.built_in_operation_upper_bound ?? [],
  defaultReads,
  'built-in host operation upper bound',
);

for (const value of Object.values(evidence.fail_closed ?? {})) {
  requireCondition(typeof value === 'boolean', 'fail_closed values must remain boolean');
}
requireCondition(
  evidence.fail_closed?.default_owner_writes_rejected === true
    && evidence.fail_closed?.provider_dispatch_before_authentication === false
    && evidence.fail_closed?.silent_in_process_fallback_for_explicit_tcp === false,
  'fail-closed dispatch/write evidence drift',
);

hasAll(apiManifest, ['sha2.workspace = true'], 'rustok-api manifest');
hasAll(
  digest,
  [
    'pub const SHA256_DIGEST_BYTES: usize = 32;',
    'pub fn sha256_digest(chunks: &[&[u8]])',
    'pub fn fixed_work_sha256_eq(',
    'for index in 0..SHA256_DIGEST_BYTES',
    'difference |= expected[index] ^ candidate[index];',
    'difference == 0',
    'does not claim',
  ],
  'shared digest helper',
);
hasAll(
  apiExports,
  [
    'pub mod digest;',
    'pub use digest::{SHA256_DIGEST_BYTES, fixed_work_sha256_eq, sha256_digest};',
  ],
  'rustok-api exports',
);

hasAll(
  commentsManifest,
  [
    'tcp-transport = ["server", "dep:tokio"]',
    'rustok-api.workspace = true',
    'tokio = { workspace = true, optional = true }',
  ],
  'rustok-comments manifest',
);
hasNone(
  commentsManifest,
  [
    'dep:sha2',
    'dep:subtle',
    'sha2 = { workspace = true, optional = true }',
    'subtle = { version = "2", optional = true }',
  ],
  'rustok-comments direct dependency boundary',
);
const commentsLockBlock = packageBlock(lock, 'rustok-comments');
const apiLockBlock = packageBlock(lock, 'rustok-api');
hasNone(commentsLockBlock, ['"sha2', '"subtle"'], 'rustok-comments Cargo.lock entry');
requireCondition(
  /"sha2(?: 0\.11\.0)?"/.test(apiLockBlock),
  'rustok-api Cargo.lock entry must retain SHA-256',
);

hasAll(
  commentsExports,
  [
    'pub mod tcp_auth;',
    'COMMENTS_TCP_PROTOCOL_VERSION',
    'CommentsTcpBearerAuthorityResolver',
    'CommentsTcpBearerToken',
    'CommentsTcpCredential',
    'CommentsTcpRequestEnvelope',
  ],
  'Comments auth exports',
);

hasAll(
  auth,
  [
    'pub const COMMENTS_TCP_PROTOCOL_VERSION: u16 = 1;',
    'const MAX_BEARER_TOKEN_BYTES: usize = 4_096;',
    'const BEARER_SCHEME: &str = "bearer";',
    'const BEARER_PREFIX: &[u8] = b"Bearer ";',
    'const DEFAULT_BEARER_OPERATIONS: [CommentsTcpOperation; 3]',
    'CommentsTcpOperation::GetComment',
    'CommentsTcpOperation::ListCommentsForTarget',
    'CommentsTcpOperation::ListPublicCommentsForTarget',
    'HashSet::from(DEFAULT_BEARER_OPERATIONS)',
    'fixed_work_sha256_eq(&self.authorization_digest, &candidate_digest)',
    'sha256_digest(&[BEARER_PREFIX, secret])',
    '!peer_addr.ip().is_loopback()',
    '!is_canonical_uuid(&claimed_context.tenant_id)',
    'comments.tcp_operation_forbidden',
    'comments.tcp_authentication_failed',
    'Comments TCP service authentication failed',
    'default_bearer_resolver_rejects_owner_writes',
    '[REDACTED]',
  ],
  'Comments bearer authentication',
);
hasNone(
  auth,
  ['ConstantTimeEq', 'use sha2::', 'use subtle::', 'println!(', 'tracing::'],
  'Comments auth dependency and logging boundary',
);

hasAll(
  server,
  [
    'pub const ALL: [Self; 7]',
    'credential: Option<&CommentsTcpCredential>',
    'serde_json::from_slice::<CommentsTcpRequestEnvelope>',
    'envelope.into_parts()',
    'protocol_version != COMMENTS_TCP_PROTOCOL_VERSION',
    'comments.tcp_server_unsupported_protocol',
    'credential.as_ref()',
    'apply_authority(request.context(), authority)?',
    'replace_request_context(&mut request, trusted_context);',
    'dispatch_request(self.provider.as_ref(), request).await',
    'comments.tcp_authority_tenant_mismatch',
  ],
  'Comments TCP server adapter',
);

hasAll(
  transport,
  [
    'bearer_token: Option<CommentsTcpBearerToken>',
    'pub fn with_bearer_token(',
    'pub fn with_bearer_secret(',
    'CommentsTcpRequestEnvelope::with_bearer(request, token)',
    'CommentsTcpRequestEnvelope::unauthenticated(request)',
    'serde_json::to_vec(&envelope)',
    'request.context().require_deadline_semantics()?;',
    'bearer_transport_debug_is_redacted',
  ],
  'Comments TCP client transport',
);
hasNone(transport, ['retry(', 'loop {', 'println!(', 'tracing::'], 'client non-claims');

hasAll(
  runtime,
  [
    'RUSTOK_COMMENTS_TCP_BEARER_TOKEN',
    'RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID',
    'comments_tcp_bearer_token_from_environment()?;',
    'TcpJsonCommentsTransport::with_bearer_token(endpoint, bearer_token)',
    'SharedCommentsTcpAuthorityResolver',
    '.unwrap_or_else(comments_tcp_bearer_authority_from_environment)',
    'CommentsTcpBearerAuthorityResolver::from_token(token, actor)',
    '.with_claim(COMMENTS_TCP_SERVICE_PERMISSION)',
    '.with_role(COMMENTS_TCP_SERVICE_ROLE)',
    'const COMMENTS_TCP_SERVICE_ROLE: &str = "admin";',
    'const COMMENTS_TCP_SERVICE_PERMISSION: &str = "comments:manage";',
    'parse_comments_tcp_service_actor_id',
    'endpoint.ip().is_loopback()',
    'bind_addr.ip().is_loopback()',
  ],
  'server host bearer wiring',
);
hasNone(
  runtime,
  [
    'struct AllowAllCommentsTcpAuthority',
    'TrustedCommentsTcpAuthority::new(',
    '%bearer_token',
    '?bearer_token',
    'bearer_token = %',
    'bearer_token = ?',
    '0.0.0.0:',
  ],
  'server host authentication boundary',
);

hasAll(
  plan,
  [
    '## Slice 72 — authenticated Comments TCP bearer envelope',
    'fixed-work comparison over two 32-byte digests',
    'does not claim a separately audited compiler or hardware side-channel',
    '`Cargo.lock` package-entry mutation',
    'The resolver\'s safe default admits exactly three read operations',
    'The default rejects `CreateComment`, `UpdateComment`, `SetCommentStatus`, and',
    'sensitive writes require separately established trusted user authority',
    'Plaintext bearer mode remains loopback-only',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
    'intentionally not run in this slice',
  ],
  'slice 72 plan',
);

console.log('[verify-blog-comments-tcp-bearer-auth] source contract verified');
