#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const read = (path) => readFileSync(resolve(root, path), "utf8");
const requireAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing required marker: ${marker}`);
  }
};
const forbidAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(!text.includes(marker), `${label} contains forbidden marker: ${marker}`);
  }
};

const contractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-host-worker-retry-proof.json";
const documentPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d8-host-worker-retry-proof.md";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_host_worker_retry_iggy.rs";
const consumerFacadePath =
  "apps/server/src/services/forum_search_inbox_worker.rs";
const workerPath =
  "apps/server/src/services/forum_search_contract_consumer.rs";
const lifecyclePath = "apps/server/src/services/app_lifecycle.rs";
const runtimeContextPath =
  "apps/server/src/services/server_runtime_context.rs";
const serverManifestPath = "apps/server/Cargo.toml";
const searchManifestPath = "crates/rustok-search/Cargo.toml";
const parentPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json";

const contract = JSON.parse(read(contractPath));
assert.equal(
  contract.contract,
  "forum_search_versioned_invalidation_host_worker_retry_source_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D8");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.runtime_evidence_parent, parentPath);
assert.equal(contract.predecessor.task, "FORUM-23B2G2B3D7");
assert.equal(contract.predecessor.pull_request, 2788);
assert.equal(contract.predecessor.state_at_authorship, "merged");
assert.equal(
  contract.predecessor.merge_commit,
  "ed5bdacfdbf8107f3a8f4eed39b705d455a85c63",
);
assert.equal(
  contract.predecessor.parent_registration,
  "D7_registered_D8_deferred_until_D8_merge",
);
assert.equal(contract.host_boundary.package, "rustok-server");
assert.equal(contract.host_boundary.test, testPath);
assert.equal(
  contract.host_boundary.startup,
  "rustok_server::services::forum_search_inbox_worker::start_forum_search_contract_consumer_if_enabled",
);
assert.equal(
  contract.host_boundary.worker_handle,
  "ForumSearchContractConsumerWorkerHandle",
);
assert.equal(
  contract.host_boundary.shutdown,
  "rustok_server::services::app_lifecycle::StopHandle",
);
assert.equal(contract.host_boundary.dependency_change_required, false);
assert.deepEqual(contract.production_references, [
  workerPath,
  consumerFacadePath,
  runtimeContextPath,
  lifecyclePath,
]);
assert.equal(
  contract.evidence_artifact.path,
  "target/forum-search-versioned-invalidation-host-worker-retry-evidence.json",
);
assert.equal(contract.evidence_artifact.generation, "executable_test_only");
assert.equal(contract.evidence_artifact.hand_editing_forbidden, true);
assert.equal(contract.evidence_artifact.source_commit_required, true);
assert.equal(
  contract.evidence_artifact.written_only_after_worker_shutdown_and_cleanup,
  true,
);
assert.equal(contract.required_runtime.database, "postgresql");
assert.equal(contract.required_runtime.broker, "external_iggy");
assert.equal(contract.required_runtime.delivery_profile, "outbox_iggy");
assert.equal(
  contract.required_runtime.consumer_group,
  "rustok-search-forum-projection-v1",
);
assert.equal(contract.required_runtime.topic, "domain");
assert.equal(contract.required_runtime.configured_max_attempts, 3);
assert.deepEqual(contract.required_runtime.configured_backoff_ms, [25, 50]);
assert.equal(contract.required_runtime.configured_idle_poll_ms, 5000);
assert.equal(
  contract.scenario.id,
  "host_worker_retry_exhaustion_restart_and_stop",
);
assert.ok(contract.scenario.proves.length >= 9);
assert.ok(contract.identity_invariants.length >= 8);
assert.ok(contract.non_claims.length >= 10);
assert.ok(
  contract.maintainer_command.includes(
    "cargo test --locked -p rustok-server --test forum_versioned_invalidation_host_worker_retry_iggy",
  ),
);

const test = read(testPath);
requireAll(
  test,
  [
    "#[serial]",
    "host_worker_exhausts_retry_then_recovers_redelivery_and_stops_promptly",
    "FORUM-23B2G2B3D8",
    "start_forum_search_contract_consumer_if_enabled",
    "ForumSearchContractConsumerWorkerHandle",
    "ServerRuntimeContext::new",
    "Arc::new(EventRuntime",
    "EventDeliveryProfile::OutboxIggy",
    "IggyMode::External",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED",
    "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_IDLE_POLL_MS",
    "CONFIGURED_MAX_ATTEMPTS: i32 = 3",
    "CONFIGURED_IDLE_POLL_MS: u64 = 5_000",
    "CREATE SEQUENCE {RETRY_SEQUENCE}",
    "BEFORE INSERT ON search_projection_inbox",
    "PERFORM nextval('{RETRY_SEQUENCE}')",
    "USING ERRCODE = '40001'",
    "wait_for_worker_finished",
    "retry_attempt_count",
    "inbox_row_count",
    "shared_take::<ForumSearchContractConsumerWorkerHandle>()",
    "remove_retry_failure",
    "wait_for_exact_inbox_rows",
    "StopHandle",
    "stop.stop().await",
    "STOP_TIMEOUT",
    "assert_consumer_group_empty",
    "target/forum-search-versioned-invalidation-host-worker-retry-evidence.json",
    "source_commit()",
  ],
  "D8 server-hosted worker retry lifecycle test",
);
forbidAll(
  test,
  [
    "process_contract_event(",
    "forum_search_contract_consumer_loop(",
    "tokio::task::abort",
    ".abort()",
    "crates/rustok-search/tests/",
    "forum_search_versioned_invalidation_poison_ambiguity",
  ],
  "D8 server-hosted worker retry lifecycle test",
);

const facade = read(consumerFacadePath);
requireAll(
  facade,
  [
    '#[path = "forum_search_contract_consumer.rs"]',
    "ForumSearchContractConsumerWorkerHandle",
    "start_forum_search_contract_consumer_if_enabled",
  ],
  "Forum Search worker public server facade",
);

const worker = read(workerPath);
requireAll(
  worker,
  [
    "tokio::spawn(forum_search_contract_consumer_loop(",
    "PersistentContractDelivery::Event(consumed)",
    "process_contract_event(",
    "Err(error) if error.is_retryable() && attempt < config.max_attempts",
    "let delay = retry_delay(config, attempt);",
    "if wait_or_stop(delay, stop_rx).await",
    "Err(error) if error.is_retryable()",
    "broker offset remains uncommitted",
    "group.acknowledge(consumed).await",
    "Ok(None) => !wait_or_stop(config.idle_poll, &mut stop_rx).await",
  ],
  "production Forum Search worker retry and stop path",
);

const lifecycle = read(lifecyclePath);
requireAll(
  lifecycle,
  [
    "pub struct StopHandle",
    "pub fn subscribe(&self)",
    "pub async fn stop(&self)",
    "self.stop_tx.send(true)",
  ],
  "server StopHandle lifecycle",
);

const runtimeContext = read(runtimeContextPath);
requireAll(
  runtimeContext,
  [
    "pub struct ServerRuntimeContext",
    "pub fn new(",
    "pub fn shared_insert<T>",
    "pub fn shared_take<T>",
    "pub fn shared_map<T, R>",
  ],
  "server runtime context lifecycle ownership",
);

const serverManifest = read(serverManifestPath);
requireAll(
  serverManifest,
  [
    'rustok-search = { workspace = true, features = ["graphql"] }',
    "rustok-iggy.workspace = true",
    'rustok-iggy-connector = { workspace = true, features = ["migrations"] }',
    "sea-orm-migration.workspace = true",
    'serial_test = "3.5"',
  ],
  "server host dependencies",
);

const searchManifest = read(searchManifestPath);
forbidAll(
  searchManifest,
  [
    "iggy.workspace = true",
    "rustok-iggy.workspace = true",
    "rustok-iggy-connector",
  ],
  "Search owner manifest cross-module host dependencies",
);

const document = read(documentPath);
requireAll(
  document,
  [
    "FORUM-23B2G2B3D8",
    "source_ready_maintainer_execution_pending",
    contractPath,
    testPath,
    "start_forum_search_contract_consumer_if_enabled",
    "max_attempts = 3",
    "base_backoff = 25 ms",
    "max_backoff = 50 ms",
    "exactly three sequence",
    "zero rows",
    "different lifecycle instance ID",
    "four valid caused Forum invalidations",
    "StopHandle",
    "within one second",
    "merged through PR #2788",
    "Closed PR #2783",
    "D8 intentionally defers its own D0 registration",
    "No command above was run by the implementation agent",
  ],
  "D8 host-worker retry handoff",
);
forbidAll(
  document,
  [
    "FORUM-23B2G2B3D7 host worker",
    "D7 intentionally defers",
    "cargo test -p rustok-search",
    "production Rust change",
  ],
  "D8 host-worker retry handoff",
);

const parent = JSON.parse(read(parentPath));
assert.equal(parent.task, "FORUM-23B2G2B3D0");
assert.equal(parent.status, "source_ready_maintainer_execution_pending");
assert.ok(
  parent.source_ready_subproofs.some(
    ({ task, test: registeredTest }) =>
      task === "FORUM-23B2G2B3D7" &&
      registeredTest ===
        "apps/server/tests/forum_versioned_invalidation_multi_process_serialization.rs",
  ),
  "D0 parent must register merged D7 multi-process serialization proof",
);
assert.ok(
  !parent.source_ready_subproofs.some(
    ({ task }) => task === "FORUM-23B2G2B3D8",
  ),
  "D0 parent must not register unmerged D8",
);
assert.ok(
  parent.required_scenarios.some(
    ({ id }) => id === "acknowledgement_failure_restart",
  ),
  "D0 parent must retain acknowledgement/restart scenario",
);
assert.equal(parent.evidence_artifact.generation, "executable_runtime_only");
assert.equal(parent.evidence_artifact.hand_editing_forbidden, true);

console.log(
  "Forum Search D8 server-owned worker retry lifecycle source proof is internally consistent.",
);
