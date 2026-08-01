import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-user-delegation.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-73.md';
const lockPath = 'Cargo.lock';
const apiManifestPath = 'crates/rustok-api/Cargo.toml';
const digestPath = 'crates/rustok-api/src/digest.rs';
const apiExportPath = 'crates/rustok-api/src/lib.rs';
const commentsManifestPath = 'crates/rustok-comments/Cargo.toml';
const commentsExportPath = 'crates/rustok-comments/src/lib.rs';
const authPath = 'crates/rustok-comments/src/tcp_auth.rs';
const delegationPath = 'crates/rustok-comments/src/tcp_delegation.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';
const blogContextPath = 'crates/rustok-blog/src/services/comment.rs';
const commentsPolicyPath = 'crates/rustok-comments/src/services.rs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-user-delegation] ${message}`);
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
  delegationPath,
  serverPath,
  transportPath,
  runtimePath,
  blogContextPath,
  commentsPolicyPath,
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
const delegation = read(delegationPath);
const server = read(serverPath);
const transport = read(transportPath);
const runtime = read(runtimePath);
const blogContext = read(blogContextPath);
const commentsPolicy = read(commentsPolicyPath);

requireCondition(evidence.schema_version === 1, 'evidence schema must remain version 1');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_user_delegation',
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
  evidence.source_contract?.digest_helper === digestPath
    && evidence.source_contract?.digest_export === apiExportPath
    && evidence.source_contract?.delegation === delegationPath
    && evidence.source_contract?.authentication === authPath
    && evidence.source_contract?.server_adapter === serverPath
    && evidence.source_contract?.client_transport === transportPath
    && evidence.source_contract?.provider_export === commentsExportPath
    && evidence.source_contract?.host_runtime === runtimePath
    && evidence.source_contract?.blog_context_owner === blogContextPath
    && evidence.source_contract?.comments_policy_owner === commentsPolicyPath,
  'evidence source path drift',
);

requireCondition(
  evidence.delegation?.version === 1
    && evidence.delegation?.credential_scheme === 'delegated_hmac_sha256'
    && evidence.delegation?.signature === 'hmac_sha256'
    && evidence.delegation?.secret_environment
      === 'RUSTOK_COMMENTS_TCP_DELEGATION_SECRET'
    && evidence.delegation?.secret_minimum_bytes === 32
    && evidence.delegation?.secret_maximum_bytes === 4096
    && evidence.delegation?.default_ttl_ms === 5000
    && evidence.delegation?.maximum_ttl_ms === 30000
    && evidence.delegation?.request_digest
      === 'sha256_serialized_comments_thread_request'
    && evidence.delegation?.signature_compare
      === 'rustok_api::fixed_work_sha256_eq'
    && evidence.delegation?.compiler_hardware_side_channel_audit === false,
  'delegation evidence drift',
);

sameSet(
  evidence.signed_bindings ?? [],
  [
    'version',
    'tenant_id',
    'user_actor_id',
    'claims',
    'single_role',
    'operation',
    'correlation_id',
    'idempotency_key',
    'issued_at_unix_ms',
    'expires_at_unix_ms',
    'nonce',
    'complete_request_digest',
  ],
  'signed binding set',
);

sameSet(
  evidence.authority_paths?.bearer_reads ?? [],
  [
    'get_comment',
    'list_comments_for_target',
    'list_public_comments_for_target',
  ],
  'bearer read path',
);
sameSet(
  evidence.authority_paths?.bearer_system_operations ?? [],
  ['set_comment_status'],
  'service moderation path',
);
sameSet(
  evidence.authority_paths?.delegated_user_operations ?? [],
  ['create_comment', 'update_comment', 'delete_comment'],
  'delegated user write path',
);
requireCondition(
  evidence.authority_paths?.operation_derived_from_request === true
    && evidence.authority_paths?.operation_request_match_required === true
    && evidence.authority_paths?.principal_replaced_before_dispatch === true
    && evidence.authority_paths?.owner_policy_runs_after_replacement === true,
  'authority ordering evidence drift',
);

requireCondition(
  evidence.replay?.one_time_per_listener_process === true
    && evidence.replay?.shared_across_adapter_clones === true
    && evidence.replay?.expired_entries_pruned === true
    && evidence.replay?.default_capacity === 4096
    && evidence.replay?.maximum_capacity === 65536
    && evidence.replay?.duplicate_code === 'comments.tcp_delegation_replayed'
    && evidence.replay?.full_or_unavailable_code
      === 'comments.tcp_delegation_replay_unavailable'
    && evidence.replay?.fail_closed_before_provider_dispatch === true
    && evidence.replay?.multi_process === false
    && evidence.replay?.multi_replica === false
    && evidence.replay?.durable === false
    && evidence.replay?.survives_restart === false,
  'replay evidence drift',
);

requireCondition(
  evidence.deadline?.source === 'PortContext.deadline_ms'
    && evidence.deadline?.scope === 'sign_claims_encode_connect_write_read_decode'
    && evidence.deadline?.mechanism === 'tokio::time::timeout'
    && evidence.deadline?.original_deadline_bounds_complete_attempt === true
    && evidence.deadline?.retry_implemented === false,
  'deadline evidence drift',
);

requireCondition(
  evidence.host_configuration?.delegation_opt_in === true
    && evidence.host_configuration?.missing_delegation_secret_profile
      === 'bearer_read_only'
    && evidence.host_configuration?.external_authority_override_precedence === true
    && evidence.host_configuration?.plaintext_loopback_only === true
    && evidence.host_configuration?.non_loopback_enabled === false
    && evidence.host_configuration?.allow_all_fallback === false
    && evidence.host_configuration?.raw_secret_logged === false,
  'host configuration evidence drift',
);

requireCondition(
  Array.isArray(evidence.dependency_contract?.new_direct_dependencies)
    && evidence.dependency_contract.new_direct_dependencies.length === 0
    && evidence.dependency_contract?.cargo_lock_package_entry_change_required === false
    && evidence.dependency_contract?.sha256_hmac_dependency_owner === 'rustok-api',
  'dependency evidence drift',
);

hasAll(apiManifest, ['sha2.workspace = true'], 'rustok-api manifest');
hasAll(
  digest,
  [
    'pub const SHA256_BLOCK_BYTES: usize = 64;',
    'pub fn hmac_sha256(key: &[u8], chunks: &[&[u8]])',
    'if key.len() > SHA256_BLOCK_BYTES',
    'let mut inner_pad = [0x36_u8; SHA256_BLOCK_BYTES];',
    'let mut outer_pad = [0x5c_u8; SHA256_BLOCK_BYTES];',
    'outer.update(inner_digest);',
    'hmac_matches_rfc_4231_case_one',
    'pub fn fixed_work_sha256_eq(',
  ],
  'shared digest/HMAC helper',
);
hasAll(
  apiExports,
  [
    'SHA256_BLOCK_BYTES',
    'SHA256_DIGEST_BYTES',
    'fixed_work_sha256_eq',
    'hmac_sha256',
    'sha256_digest',
  ],
  'rustok-api digest exports',
);

hasAll(
  commentsManifest,
  [
    'tcp-transport = ["server", "dep:tokio"]',
    'rustok-api.workspace = true',
  ],
  'rustok-comments manifest',
);
hasNone(
  commentsManifest,
  ['dep:hmac', 'dep:sha2', 'dep:subtle', 'hmac =', 'subtle ='],
  'rustok-comments direct dependency boundary',
);
const commentsLockBlock = packageBlock(lock, 'rustok-comments');
hasNone(
  commentsLockBlock,
  ['"hmac"', '"sha2', '"subtle"'],
  'rustok-comments Cargo.lock entry',
);

hasAll(
  commentsExports,
  [
    'pub mod tcp_delegation;',
    'CommentsTcpDelegatingAuthorityResolver',
    'CommentsTcpDelegationSecret',
    'CommentsTcpDelegationSigner',
    'MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY',
    'CommentsTcpTransportConfigError',
  ],
  'Comments delegation exports',
);

hasAll(
  auth,
  [
    'const DELEGATED_HMAC_SCHEME: &str = "delegated_hmac_sha256";',
    'pub(crate) fn delegated(token: String)',
    'pub(crate) fn token(&self) -> &str',
    'pub(crate) fn with_credential(',
    'request: &CommentsThreadRequest',
    'let claimed_context: &PortContext = request.context();',
    '[REDACTED]',
  ],
  'credential/auth seam',
);
hasNone(auth, ['println!(', 'tracing::'], 'credential logging boundary');

hasAll(
  delegation,
  [
    'pub const COMMENTS_TCP_DELEGATION_VERSION: u16 = 1;',
    'pub const DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS: u64 = 5_000;',
    'pub const MAX_COMMENTS_TCP_DELEGATION_TTL_MS: u64 = 30_000;',
    'pub const DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY: usize = 4_096;',
    'pub const MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY: usize = 65_536;',
    'const DELEGATION_SCHEME: &str = "delegated_hmac_sha256";',
    'const DELEGATION_SIGNATURE_DOMAIN:',
    'pub struct CommentsTcpDelegationSecret',
    'pub struct CommentsTcpDelegationSigner',
    'pub struct CommentsTcpDelegatingAuthorityResolver',
    'context.require_write_semantics()?;',
    'context.actor.kind != PortActorKind::User',
    'context.roles.len() != 1',
    'request_digest(request)?',
    'hmac_sha256(secret.as_bytes(), &[DELEGATION_SIGNATURE_DOMAIN, payload])',
    'fixed_work_sha256_eq(&expected, &signed.signature)',
    'CommentsTcpOperation::for_request(request) != operation',
    'claims.idempotency_key != idempotency_key',
    'claims.request_digest != digest',
    'claims.expires_at_unix_ms < now_ms',
    'replay.entries.retain(|_, expiry| *expiry >= now_ms);',
    'replay.entries.contains_key(nonce)',
    'replay.entries.len() >= replay.capacity',
    'comments.tcp_delegation_replayed',
    'comments.tcp_delegation_replay_unavailable',
    'operation == CommentsTcpOperation::SetCommentStatus',
    'request.context().actor.kind == PortActorKind::System',
    'PortActor::user(claims.actor_id)',
    '[REDACTED]',
  ],
  'delegation core',
);
hasNone(
  delegation,
  ['println!(', 'tracing::', '0.0.0.0:', 'multi-replica replay protection'],
  'delegation non-claims/logging boundary',
);

hasAll(
  server,
  [
    'pub const fn as_str(self) ->',
    'pub const fn is_write(self) -> bool',
    'pub fn for_request(request: &CommentsThreadRequest) -> Self',
    'request: &CommentsThreadRequest',
    'let operation = CommentsTcpOperation::for_request(&request);',
    '.authorize(peer_addr, operation, credential.as_ref(), &request)',
    'apply_authority(request.context(), authority)?',
    'replace_request_context(&mut request, trusted_context);',
    'dispatch_request(self.provider.as_ref(), request).await',
  ],
  'server authorization ordering',
);

hasAll(
  transport,
  [
    'delegation_signer: Option<CommentsTcpDelegationSigner>',
    'pub fn with_bearer_and_delegation(',
    'pub fn supports_delegated_writes(&self) -> bool',
    'let operation = CommentsTcpOperation::for_request(&request);',
    'request.context().actor.kind == PortActorKind::System',
    'let credential = signer.credential_for(&request)?;',
    'CommentsTcpRequestEnvelope::with_credential(request, credential)',
    'self.prepare_and_exchange(request)',
    'Duration::from_millis(deadline_ms)',
  ],
  'client delegation routing/deadline',
);
hasNone(transport, ['retry(', 'loop {', 'println!(', 'tracing::'], 'client non-claims');

hasAll(
  runtime,
  [
    'RUSTOK_COMMENTS_TCP_DELEGATION_SECRET',
    'RUSTOK_COMMENTS_TCP_DELEGATION_TTL_MS',
    'RUSTOK_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY',
    'comments_tcp_delegation_signer_from_environment()',
    'TcpJsonCommentsTransport::with_bearer_and_delegation(',
    'comments_tcp_authority_from_environment',
    'CommentsTcpDelegatingAuthorityResolver::new(',
    '.with_service_claim(COMMENTS_TCP_SERVICE_PERMISSION)',
    '.with_service_role(COMMENTS_TCP_SERVICE_ROLE)',
    '.with_max_ttl(Duration::from_millis(ttl_ms))',
    '.with_replay_capacity(replay_capacity)',
    'authentication = "bearer_or_delegated_hmac_or_host_override"',
    'endpoint.ip().is_loopback()',
    'bind_addr.ip().is_loopback()',
  ],
  'host delegation configuration',
);
hasNone(
  runtime,
  [
    'struct AllowAllCommentsTcpAuthority',
    '0.0.0.0:',
    '%delegation_secret',
    '?delegation_secret',
    'delegation_secret = %',
    'delegation_secret = ?',
  ],
  'host secret/logging boundary',
);

hasAll(
  blogContext,
  [
    'comments_write_port_context(',
    'PortActor::user(',
    '.with_idempotency_key(',
    '.with_deadline(std::time::Duration::from_secs(2))',
    'context = context.with_role(security.role.to_string());',
    'context = context.with_claim(permission.to_string());',
    'comments_write_port_context(\n                    tenant_id,\n                    &SecurityContext::system(),',
  ],
  'Blog authority context construction',
);

hasAll(
  commentsPolicy,
  [
    'fn enforce_create_scope(&self, security: &SecurityContext)',
    'security.user_id',
    'fn enforce_owned_scope(',
    'PermissionScope::Own if security.user_id == Some(author_id)',
    'fn enforce_moderation_scope(&self, security: &SecurityContext)',
    'security.get_scope(Resource::Comments, Action::Moderate)',
    'security.get_scope(Resource::Comments, Action::Manage)',
  ],
  'Comments owner policy',
);

hasAll(
  plan,
  [
    '## Slice 73 — signed user delegation for Comments TCP writes',
    '`delegated_hmac_sha256`',
    'SHA-256 of the complete serialized `CommentsThreadRequest`',
    'process-local replay cache',
    'hard-bounded to 1..=65536',
    'cluster-wide, multi-process, multi-replica, or durable replay prevention',
    'Plaintext credential modes remain loopback-only',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
    'intentionally not run in this slice',
  ],
  'slice 73 plan',
);

console.log('[verify-blog-comments-tcp-user-delegation] source contract verified');
