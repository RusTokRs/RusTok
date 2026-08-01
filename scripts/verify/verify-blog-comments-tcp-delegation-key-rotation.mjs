import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-key-rotation.json';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-76.md';
const manifestPath = 'crates/rustok-comments/Cargo.toml';
const lockPath = 'Cargo.lock';
const exportPath = 'crates/rustok-comments/src/lib.rs';
const delegationPath = 'crates/rustok-comments/src/tcp_delegation.rs';
const transportPath = 'crates/rustok-comments/src/tcp_transport.rs';
const serverPath = 'crates/rustok-comments/src/tcp_server.rs';
const runtimePath = 'apps/server/src/services/comments_provider_runtime.rs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-comments-tcp-delegation-key-rotation] ${message}`);
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
  manifestPath,
  lockPath,
  exportPath,
  delegationPath,
  transportPath,
  serverPath,
  runtimePath,
]) {
  requireCondition(fs.existsSync(path), `missing source artifact ${path}`);
}

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const manifest = read(manifestPath);
const lock = read(lockPath);
const exports = read(exportPath);
const delegation = read(delegationPath);
const transport = read(transportPath);
const server = read(serverPath);
const runtime = read(runtimePath);

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(
  evidence.module === 'blog'
    && evidence.provider === 'comments'
    && evidence.surface === 'comments_tcp_delegation_key_rotation',
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
  evidence.key_id?.minimum_bytes === 1
    && evidence.key_id?.maximum_bytes === 64
    && evidence.key_id?.signed === true
    && evidence.key_id?.authorization_principal === false
    && evidence.key_id?.secret === false,
  'key ID evidence drift',
);
requireCondition(
  evidence.keyring?.minimum_keys === 1
    && evidence.keyring?.maximum_keys === 8
    && evidence.keyring?.unique_ids_required === true
    && evidence.keyring?.active_key_required === true
    && evidence.keyring?.immutable_after_construction === true
    && evidence.keyring?.active_key_used_for_signing === true
    && evidence.keyring?.all_retained_keys_used_for_verification === true
    && evidence.keyring?.debug_secret_redaction === true
    && evidence.keyring?.debug_active_id_redaction === true,
  'keyring evidence drift',
);
requireCondition(
  evidence.signature?.algorithm === 'hmac_sha256'
    && evidence.signature?.verification_before_claims_parse === true
    && evidence.signature?.unknown_key_code === 'comments.tcp_delegation_invalid'
    && evidence.signature?.bad_signature_code === 'comments.tcp_delegation_invalid'
    && evidence.signature?.revoked_key_code === 'comments.tcp_delegation_invalid'
    && evidence.signature?.key_lookup_oracle_in_public_error === false,
  'signature evidence drift',
);
requireCondition(
  evidence.compatibility?.single_secret_constructor_retained === true
    && evidence.compatibility?.single_secret_key_id === 'legacy'
    && evidence.compatibility?.new_single_secret_tokens_are_keyed === true
    && evidence.compatibility?.old_unkeyed_tokens_supported === true
    && evidence.compatibility?.legacy_fallback_explicit_for_multi_key_ring === true
    && evidence.compatibility?.credential_scheme_changed === false
    && evidence.compatibility?.delegation_version_changed === false
    && evidence.compatibility?.host_environment_contract_changed === false,
  'compatibility evidence drift',
);
requireCondition(
  evidence.rotation?.overlapping_verification_keys === true
    && evidence.rotation?.independent_active_signing_key === true
    && evidence.rotation?.old_key_removal_revokes_tokens === true
    && evidence.rotation?.live_mutation === false
    && evidence.rotation?.scheduled_activation === false,
  'rotation evidence drift',
);
requireCondition(
  Array.isArray(evidence.dependency_contract?.new_direct_dependencies)
    && evidence.dependency_contract.new_direct_dependencies.length === 0
    && evidence.dependency_contract?.manifest_changed === false
    && evidence.dependency_contract?.cargo_lock_changed === false,
  'dependency evidence drift',
);

hasAll(
  delegation,
  [
    'pub const MAX_COMMENTS_TCP_DELEGATION_KEYS: usize = 8;',
    'pub const MAX_COMMENTS_TCP_DELEGATION_KEY_ID_BYTES: usize = 64;',
    'const LEGACY_DELEGATION_KEY_ID: &str = "legacy";',
    'pub struct CommentsTcpDelegationKeyId(String);',
    'pub struct CommentsTcpDelegationKeyring',
    'active_key_id: CommentsTcpDelegationKeyId',
    'keys: Arc<HashMap<CommentsTcpDelegationKeyId, CommentsTcpDelegationSecret>>',
    'legacy_unkeyed_key_id: Option<CommentsTcpDelegationKeyId>',
    'pub fn single(secret: CommentsTcpDelegationSecret) -> Self',
    'pub fn new(\n        active_key_id: CommentsTcpDelegationKeyId,',
    'keys.is_empty() || keys.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS',
    'by_id.insert(key_id, secret).is_some()',
    '!by_id.contains_key(&active_key_id)',
    'pub fn with_legacy_unkeyed_key_id(',
    'pub fn with_keyring(keyring: CommentsTcpDelegationKeyring) -> Self',
    'pub fn with_keyring_and_ttl(',
    'pub fn active_key_id(&self) -> &CommentsTcpDelegationKeyId',
    'let key_id = self.keyring.active_key_id().as_str().to_string();',
    'keyed_delegation_signature(',
    'key_id: Some(key_id)',
    'pub fn with_keyring(\n        bearer_token: CommentsTcpBearerToken,',
    'let expected = match signed.key_id.as_deref()',
    '.verification_secret(&key_id)',
    '.legacy_unkeyed_secret()',
    'fixed_work_sha256_eq(&expected, &signed.signature)',
    'serde_json::from_str::<CommentsTcpDelegationClaims>(&signed.payload)',
    '#[serde(default, skip_serializing_if = "Option::is_none")]',
    'key_id: Option<String>',
    'key_id.as_bytes()',
    'KEY_ID_SEPARATOR',
    'legacy_delegation_signature(',
    'comments.tcp_delegation_invalid',
    'overlapping_keyring_accepts_old_and_new_keyed_tokens',
    'revoked_or_unknown_key_id_fails_with_generic_invalid_code',
    'legacy_unkeyed_token_can_be_retained_during_rolling_upgrade',
    '[REDACTED]',
    '[CONFIGURED]',
  ],
  'delegation key rotation core',
);

const verificationIndex = delegation.indexOf(
  'fixed_work_sha256_eq(&expected, &signed.signature)',
);
const claimsParseIndex = delegation.indexOf(
  'serde_json::from_str::<CommentsTcpDelegationClaims>(&signed.payload)',
);
requireCondition(
  verificationIndex >= 0 && claimsParseIndex > verificationIndex,
  'claims must be parsed only after key selection and signature verification',
);

hasNone(
  delegation,
  [
    'println!(',
    'tracing::',
    'HashMap<String, CommentsTcpDelegationSecret>',
    'unknown delegation key',
    'revoked delegation key',
    'multi-replica replay protection',
  ],
  'delegation key/logging non-claims',
);

hasAll(
  exports,
  [
    'CommentsTcpDelegationKeyId',
    'CommentsTcpDelegationKeyring',
    'MAX_COMMENTS_TCP_DELEGATION_KEY_ID_BYTES',
    'MAX_COMMENTS_TCP_DELEGATION_KEYS',
    'CommentsTcpDelegationSigner',
    'CommentsTcpDelegatingAuthorityResolver',
  ],
  'delegation exports',
);

hasAll(
  transport,
  [
    'delegation_signer: Option<CommentsTcpDelegationSigner>',
    'let credential = signer.credential_for(&request)?;',
    'CommentsTcpRequestEnvelope::with_credential(request, credential)',
  ],
  'transport signer path',
);
hasAll(
  server,
  [
    '.authorize(peer_addr, operation, credential.as_ref(), &request)',
    'replace_request_context(&mut request, trusted_context);',
    'dispatch_request(self.provider.as_ref(), request).await',
  ],
  'server authority ordering',
);
hasAll(
  runtime,
  [
    'RUSTOK_COMMENTS_TCP_DELEGATION_SECRET',
    'CommentsTcpDelegationSigner::with_ttl(secret, Duration::from_millis(ttl_ms))',
    'CommentsTcpDelegatingAuthorityResolver::new(token, actor, delegation_secret)',
  ],
  'retained single-secret host compatibility',
);
hasNone(
  runtime,
  [
    'RUSTOK_COMMENTS_TCP_DELEGATION_KEYS_JSON',
    'RUSTOK_COMMENTS_TCP_DELEGATION_ACTIVE_KEY_ID',
    '%delegation_secret',
    '?delegation_secret',
  ],
  'host multi-key and secret logging non-claims',
);

hasAll(
  manifest,
  [
    'tcp-transport = ["server", "dep:tokio"]',
    'rustok-api.workspace = true',
  ],
  'manifest retained dependency boundary',
);
const commentsLockBlock = packageBlock(lock, 'rustok-comments');
hasNone(
  commentsLockBlock,
  ['"hmac"', '"sha2', '"subtle"', '"rustls"', '"tokio-rustls"'],
  'Cargo.lock retained dependency boundary',
);

hasAll(
  plan,
  [
    '# rustok-blog implementation plan — slice 76 continuation',
    '## Slice 76 — delegation key IDs and overlapping rotation core',
    '1..=8 unique keys',
    'with_legacy_unkeyed_key_id',
    'comments.tcp_delegation_invalid',
    'Recommended rotation sequence',
    'environment JSON or file parsing for multiple delegation keys',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
  ],
  'slice-76 plan',
);

console.log('[verify-blog-comments-tcp-delegation-key-rotation] source contract verified');
