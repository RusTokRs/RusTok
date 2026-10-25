import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readComposedRustSource } from './lib/rust-facade-source.mjs';
import { flattenWhitespace } from './lib/verify-text.mjs';

// This guard used to require that the files of the slice stayed byte-identical to a recorded git blob
// SHA. A digest describes one commit, not an invariant: the pin fired on every legitimate edit of files
// this guard does not own (rustok-events, apps/server/Cargo.toml, Cargo.lock), while every behavioural
// assertion below was intact. The snapshot is preserved as a record in
// crates/modules/rustok-blog/docs/comments-delegation-preserved-owner-snapshot-2026-10-26.md; the live
// rules are the markers, which are unchanged.

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
);
const read = (relativePath) =>
  readFileSync(path.join(root, relativePath), 'utf8');

const sourcePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs';
const storePath =
  'apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs';
const planPath =
  'crates/modules/rustok-blog/docs/implementation-plan-slice-86.md';
const evidencePath =
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-audited-worker-response-disconnect.json';

const source = readComposedRustSource(read, sourcePath);
const productionStore = readComposedRustSource(read, storePath);
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

function hasAllProse(content, markers, label) {
  // See scripts/verify/lib/verify-text.mjs: prose markers are wording contracts, so they
  // are compared against a whitespace-flattened view of the source.
  const flattened = flattenWhitespace(content);
  for (const marker of markers) {
    requireCondition(
      flattened.includes(flattenWhitespace(marker)),
      `${label} missing marker: ${marker}`,
    );
  }
}



hasAll(source, [
  'enum PostgresAuditedPersistenceStore',
  'PostgresAuditedPersistenceStore::Production(store)',
  '#[cfg(test)]\n    ResponseDisconnect(AuditedStoreResponseDisconnectHarness)',
  '#[cfg(test)]\n    fn from_response_disconnect_harness',
  'struct AuditedStoreResponseDisconnectHarness {',
  'mpsc::sync_channel(1)',
  'AuditedStoreResponseDisconnectCommand::VerifyCurrent',
  'AuditedStoreResponseDisconnectCommand::BootstrapEmpty',
  'AuditedStoreResponseDisconnectCommand::CompareAndStoreWithAudit',
  'drop(response);',
  'response_receiver.recv().unwrap_or(Err(',
  'CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable',
  'fn abort_on_indeterminate_audited_store_response() -> ! {',
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

// The abort helper must stay wired to the two fail-stop paths, not merely be defined: the
// indeterminate-store match arm and the response-completion drop guard. A bare
// `abort_on_indeterminate_audited_store_response()` marker would also be satisfied by the
// function definition alone, which would let a stranded call site pass unnoticed.
requireCondition(
  /Unavailable\) => \{\s*abort_on_indeterminate_audited_store_response\(\)/.test(source),
  'indeterminate audited store arm must abort',
);
requireCondition(
  /if !self\.completed \{\s*abort_on_indeterminate_audited_store_response\(\);/.test(source),
  'dropped audited response completion guard must abort',
);

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

hasAllProse(plan, [
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
