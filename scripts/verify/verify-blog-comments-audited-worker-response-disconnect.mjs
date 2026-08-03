import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);
const read = (relativePath) =>
  readFileSync(path.join(root, relativePath), 'utf8');
const readBuffer = (relativePath) =>
  readFileSync(path.join(root, relativePath));

const sourcePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs';
const storePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs';
const planPath =
  'crates/rustok-blog/docs/implementation-plan-slice-86.md';
const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-comments-audited-worker-response-disconnect.json';

const source = read(sourcePath);
const productionStore = read(storePath);
const plan = read(planPath);
const evidence = JSON.parse(read(evidencePath));

function requireCondition(condition, message) {
  if (!condition) throw new Error(message);
}

function hasAll(content, markers, label) {
  for (const marker of markers) {
    requireCondition(
      content.includes(marker),
      `${label} missing marker: ${marker}`,
    );
  }
}

function gitBlobSha(relativePath) {
  const content = readBuffer(relativePath);
  return createHash('sha1')
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest('hex');
}

const preserved = {
  [storePath]: '8a27f5ec3938f2b4efe16c6acafb93cb3faadcf6',
  'apps/server/Cargo.toml':
    'd6a77ff30fd67c3d4fee16afbadc584c2bcce9e7',
  'apps/server/tests/blog_comments_schedule_audit_postgres.rs':
    '4ebe3a285495a8cbeb69729a9ca7f3452456785d',
  'crates/rustok-blog/docs/implementation-plan-slice-85.md':
    'f8a3ad0216ae534e9e913e966fe0141adba22c94',
};

for (const [relativePath, expectedSha] of Object.entries(preserved)) {
  requireCondition(
    gitBlobSha(relativePath) === expectedSha,
    `preserved owner drift: ${relativePath}`,
  );
}

hasAll(source, [
  'enum PostgresAuditedPersistenceStore',
  'PostgresAuditedPersistenceStore::Production(store)',
  '#[cfg(test)]\n    ResponseDisconnect(AuditedStoreResponseDisconnectHarness)',
  '#[cfg(test)]\n    fn from_response_disconnect_harness',
  '#[cfg(test)]\nstruct AuditedStoreResponseDisconnectHarness',
  'mpsc::sync_channel(1)',
  'AuditedStoreResponseDisconnectCommand::VerifyCurrent',
  'AuditedStoreResponseDisconnectCommand::BootstrapEmpty',
  'AuditedStoreResponseDisconnectCommand::CompareAndStoreWithAudit',
  'drop(response);',
  'response_receiver.recv().unwrap_or(Err(',
  'CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable',
  ') => abort_on_indeterminate_audited_store_response(),',
  'fn audited_worker_response_disconnect_aborts()',
  'fn audited_worker_response_disconnect_child()',
  'std::env::current_exe()',
  '.arg("--ignored")',
  '.arg("--test-threads=1")',
  'CHILD_TIMEOUT',
  'child.kill()',
  'stderr.contains(CHILD_READY_MARKER)',
  'status.signal(),',
  'Some(6)',
  'SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger::from_response_disconnect_harness',
  'audited.replace_host_schedule(',
], 'audited worker-response disconnect source');

requireCondition(
  (
    source.match(
      /#\[ignore = "(?:requires subprocess abort and signal observation|subprocess entry point for audited worker-response disconnect)"\]/g,
    ) ?? []
  ).length === 2,
  'parent and child disconnect tests must remain explicitly ignored',
);

requireCondition(
  !source.includes('pub fn from_response_disconnect_harness')
    && !source.includes('pub struct AuditedStoreResponseDisconnectHarness'),
  'diagnostic seam must not become public',
);

const testModuleOffset = source.indexOf('#[cfg(test)]\nmod tests');
requireCondition(testModuleOffset > 0, 'cfg(test) module boundary is missing');
const nonTestPrefix = source.slice(0, testModuleOffset);
requireCondition(
  !nonTestPrefix.includes('RUSTOK_BLOG_AUDITED_WORKER_RESPONSE_DISCONNECT_CHILD'),
  'diagnostic environment switch escaped into non-test composition',
);

hasAll(productionStore, [
  'PostgresAuditStoreCommand::CompareAndStoreWithAudit',
  'response_receiver.recv().unwrap_or(Err(',
  'CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable',
  'run_postgres_audit_store_worker(runtime, database, receiver);',
], 'production audited PostgreSQL store');

hasAll(plan, [
  '## 2026-08-03 continuation audit',
  'Slice 86 — audited worker-response disconnect fail-stop harness',
  'compiled only under `cfg(test)`',
  'drops it without sending a value',
  'Existing fail-stop path under test',
  'signal 6 (`SIGABRT`)',
  'Status: `source_verified_no_compile`',
  'outbox dispatcher contract',
  'intentionally not run',
], 'slice 86 plan');

requireCondition(
  evidence.status === 'source_verified_no_compile',
  'evidence status drift',
);
requireCondition(
  evidence.production_boundary.public_api_changed === false,
  'public API overclaim',
);
requireCondition(
  evidence.production_boundary.runtime_environment_switch_added === false,
  'runtime environment switch overclaim',
);
requireCondition(
  evidence.diagnostic_backend.cfg_test_only === true,
  'cfg(test) diagnostic boundary missing',
);
requireCondition(
  evidence.diagnostic_backend.bounded_command_capacity === 1,
  'diagnostic command bound drift',
);
requireCondition(
  evidence.diagnostic_backend.audited_compare_response_sender_dropped === true,
  'response disconnect evidence missing',
);
requireCondition(
  evidence.diagnostic_backend.abort_helper_called_directly === false,
  'harness must not call the abort helper directly',
);
requireCondition(
  evidence.fail_stop_path.outer_persistence_bridge_used === true,
  'outer audited bridge evidence missing',
);
requireCondition(
  evidence.fail_stop_path.unavailable_maps_to_abort === true,
  'Unavailable-to-abort evidence missing',
);
requireCondition(
  evidence.subprocess.timeout_seconds === 10,
  'subprocess timeout drift',
);
requireCondition(
  evidence.subprocess.expected_unix_signal === 6,
  'Unix abort signal drift',
);
requireCondition(
  evidence.plan_status.worker_response_disconnect_source_gate_open === false,
  'source gate must be represented by slice 86',
);
requireCondition(
  evidence.plan_status.worker_response_disconnect_runtime_gate_open === true,
  'runtime gate must remain open until execution',
);
requireCondition(
  evidence.execution.rust_tests_run === false,
  'Rust execution overclaim',
);
requireCondition(
  evidence.execution.javascript_verifiers_run === false,
  'verifier execution overclaim',
);
requireCondition(
  evidence.execution.subprocess_run === false,
  'subprocess execution overclaim',
);
requireCondition(
  evidence.execution.signal_observed === false,
  'signal observation overclaim',
);

console.log(
  'Blog Comments audited worker-response disconnect source contract verified',
);
