import fs from 'node:fs';

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-remote-adapter-core.json';
const adapterPath = 'crates/rustok-comments/src/remote.rs';
const providerExportPath = 'crates/rustok-comments/src/lib.rs';
const planPath = 'crates/rustok-blog/docs/implementation-plan-slice-67.md';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function json(path) {
  return JSON.parse(read(path));
}

function fail(message) {
  console.error(`[verify-blog-comments-remote-adapter-core] ${message}`);
  process.exit(1);
}

function hasAll(text, markers, label) {
  for (const marker of markers) {
    if (!text.includes(marker)) fail(`${label} missing ${marker}`);
  }
}

function sameSet(actual, expected, label) {
  const left = [...actual].sort().join('|');
  const right = [...expected].sort().join('|');
  if (left !== right) fail(`${label} drift: expected ${right}, got ${left}`);
}

const evidence = json(evidencePath);
const adapter = read(adapterPath);
const providerExport = read(providerExportPath);
const plan = read(planPath);

if (evidence.schema_version !== 1) fail('evidence schema_version drift');
if (
  evidence.module !== 'blog'
  || evidence.provider !== 'comments'
  || evidence.surface !== 'comments_remote_adapter_core'
) fail('evidence identity drift');
if (
  evidence.status !== 'source_verified_no_compile'
  || evidence.compile_policy !== 'not_run_by_request'
  || evidence.runtime_status !== 'not_run'
) fail('evidence execution status drift');
if (evidence.generated_from !== planPath) fail('evidence plan path drift');
if (evidence.source_contract?.provider_adapter !== adapterPath) {
  fail('provider adapter path drift');
}
if (evidence.source_contract?.provider_export !== providerExportPath) {
  fail('provider export path drift');
}

const operations = [
  'create_comment',
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
  'update_comment',
  'set_comment_status',
  'delete_comment',
];
sameSet(
  (evidence.operations ?? []).map((operation) => operation.name),
  operations,
  'remote operation set',
);

const readOperations = new Set([
  'get_comment',
  'list_comments_for_target',
  'list_public_comments_for_target',
]);
for (const operation of evidence.operations ?? []) {
  const expectedPolicy = readOperations.has(operation.name) ? 'read' : 'write';
  if (operation.policy !== expectedPolicy) {
    fail(`${operation.name} policy drift`);
  }
  hasAll(
    adapter,
    [operation.request, operation.response],
    `${operation.name} wire mapping`,
  );
}

sameSet(
  evidence.context_fields_preserved ?? [],
  [
    'tenant_id',
    'actor',
    'claims',
    'roles',
    'channel',
    'locale',
    'correlation_id',
    'causation_id',
    'traceparent',
    'idempotency_key',
    'deadline_ms',
  ],
  'PortContext preservation fields',
);

hasAll(
  adapter,
  [
    'pub enum CommentsThreadRequest',
    'pub enum CommentsThreadResponse',
    'pub trait CommentsThreadTransport: Send + Sync',
    'pub struct RemoteCommentsThreadPort',
    'pub fn remote_comments_thread_port',
    'impl CommentsThreadPort for RemoteCommentsThreadPort',
    'context.require_policy(PortCallPolicy::read())?',
    'context.require_policy(PortCallPolicy::write())?',
    'comments.remote_response_mismatch',
    'PortError::invariant_violation',
    'remote_adapter_accepts_a_transport_trait_object',
  ],
  'remote adapter source',
);

hasAll(
  providerExport,
  [
    '#[cfg(feature = "server")]\npub mod remote;',
    '#[cfg(feature = "server")]\npub use remote::*;',
  ],
  'provider exports',
);

hasAll(
  plan,
  [
    '# rustok-blog implementation plan — slice 67 continuation',
    'Typed storefront `AVAILABLE`, `UNAVAILABLE`, and `TIMEOUT` rendering is already',
    'Slice 67 adds the provider-owned typed remote adapter core.',
    'A concrete HTTP, gRPC, message-bus, or sidecar client is still pending.',
    'Status: `source_verified_no_compile`.',
    'Compile policy: `not_run_by_request`.',
    'Runtime status: `not_run`.',
    evidencePath,
    adapterPath,
    'scripts/verify/verify-blog-comments-remote-adapter-core.mjs',
  ],
  'slice 67 plan',
);

if (evidence.fail_closed?.response_mismatch_code !== 'comments.remote_response_mismatch') {
  fail('response mismatch code drift');
}
if (evidence.fail_closed?.response_mismatch_kind !== 'InvariantViolation') {
  fail('response mismatch kind drift');
}
if (evidence.fail_closed?.policy_checked_before_dispatch !== true) {
  fail('policy-before-dispatch marker drift');
}

sameSet(
  evidence.pending ?? [],
  [
    'concrete_network_transport',
    'endpoint_discovery',
    'authentication',
    'retry_backoff',
    'transport_cancellation',
    'host_publication',
    'in_process_remote_runtime_parity',
    'runtime_execution',
  ],
  'pending scope',
);

for (const forbidden of [
  'status": "compiled"',
  'runtime_status": "passed"',
  'tests passed',
  'CI passed',
  'production verified',
]) {
  if (read(evidencePath).includes(forbidden) || plan.includes(forbidden)) {
    fail(`false execution claim detected: ${forbidden}`);
  }
}

console.log('[verify-blog-comments-remote-adapter-core] source contract verified');
