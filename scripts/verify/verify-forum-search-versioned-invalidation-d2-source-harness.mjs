#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();
const failures = [];

const paths = {
  contract:
    "crates/rustok-forum/contracts/evidence/forum-search-versioned-invalidation-d2-source-harness.json",
  note:
    "crates/rustok-forum/docs/forum-23b2g2b3d2-versioned-invalidation-source-evidence.md",
  postgres:
    "crates/rustok-search/tests/forum_contract_ingress_postgres_test.rs",
  consumer: "crates/rustok-iggy/src/contract_consumer.rs",
  restart: "crates/rustok-iggy/src/contract_consumer_restart_tests.rs",
  worker: "apps/server/src/services/forum_search_contract_consumer.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  d0:
    "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json",
};

function target(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  if (!fs.existsSync(target(relativePath))) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return fs.readFileSync(target(relativePath), "utf8");
}

function readJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const contract = readJson(paths.contract);
const d0 = readJson(paths.d0);
const note = read(paths.note);
const postgres = read(paths.postgres);
const consumer = read(paths.consumer);
const restart = read(paths.restart);
const worker = read(paths.worker);
const plan = read(paths.plan);

if (contract) {
  if (
    contract.task !== "FORUM-23B2G2B3D2" ||
    contract.status !== "source_ready_maintainer_execution_pending"
  ) {
    failures.push(`${paths.contract}: task or status drift`);
  }
  if (
    contract.postgresql_ingress?.test_target !== paths.postgres ||
    contract.postgresql_ingress?.real_search_module_migrations !== true ||
    contract.postgresql_ingress?.single_root_row !== true ||
    contract.postgresql_ingress?.first_ingest_sequence_retained !== true ||
    contract.postgresql_ingress?.owner_revision_is_not_ingest_sequence !== true ||
    contract.postgresql_ingress
      ?.identity_conflict_is_non_retryable_semantic_poison !== true
  ) {
    failures.push(`${paths.contract}: PostgreSQL evidence boundary drift`);
  }
  if (
    contract.persistent_cursor_restart?.source_target !== paths.restart ||
    contract.persistent_cursor_restart
      ?.event_ack_failure_leaves_offset_uncommitted !== true ||
    contract.persistent_cursor_restart
      ?.event_redelivery_after_group_reconstruction !== true ||
    contract.persistent_cursor_restart
      ?.decode_failure_ack_failure_leaves_offset_uncommitted !== true ||
    contract.persistent_cursor_restart
      ?.decode_failure_redelivery_after_group_reconstruction !== true ||
    contract.persistent_cursor_restart
      ?.successful_restarted_ack_commits_exact_offset !== true
  ) {
    failures.push(`${paths.contract}: persistent cursor restart boundary drift`);
  }
  if (
    contract.single_execution_path?.search_inbox !==
      "search_projection_inbox" ||
    contract.single_execution_path?.second_inbox !== false ||
    contract.single_execution_path?.second_projector !== false ||
    contract.single_execution_path?.second_reconciler !== false ||
    contract.single_execution_path?.second_ordering_clock !== false
  ) {
    failures.push(`${paths.contract}: single execution path drift`);
  }
  if (contract.follow_up?.task !== "FORUM-23B2G2B3D3") {
    failures.push(`${paths.contract}: bounded follow-up drift`);
  }
}

if (
  d0?.task !== "FORUM-23B2G2B3D0" ||
  d0?.status !== "source_ready_maintainer_execution_pending"
) {
  failures.push(`${paths.d0}: accepted runtime protocol drift`);
}

requireAll(
  note,
  [
    "# FORUM-23B2G2B3D2 versioned invalidation source evidence",
    "source_ready_maintainer_execution_pending",
    "forum_contract_ingress_postgres_test.rs",
    "contract_consumer_restart_tests.rs",
    "exact offset to remain uncommitted",
    "FORUM-23B2G2B3D3",
    "These commands were not run",
  ],
  paths.note,
);

requireAll(
  postgres,
  [
    "RUSTOK_SEARCH_TEST_DATABASE_URL",
    "for migration in SearchModule.migrations()",
    "legacy_first_then_typed_restart_reuses_one_exact_root_row",
    "typed_first_then_legacy_delivery_keeps_search_owned_sequence",
    "conflicting_legacy_identity_is_non_retryable_semantic_poison",
    "ForumSearchContractIngress::new",
    "ON CONFLICT (event_id) DO NOTHING",
    "assert_ne!(typed_first.ingest_sequence, owner_revision)",
    "forum.search_projection.contract_inbox_identity_conflict",
    "DROP SCHEMA IF EXISTS",
  ],
  paths.postgres,
);
rejectAll(
  postgres,
  ["tokio::time::sleep", "std::thread::sleep", "loop {"],
  `${paths.postgres} deterministic boundary`,
);

requireAll(
  consumer,
  [
    "pub struct PersistentContractConsumerGroup",
    "pub async fn acknowledge(",
    "pub async fn acknowledge_decode_failure(",
    "#[path = \"contract_consumer_restart_tests.rs\"]",
    "mod restart_tests;",
  ],
  paths.consumer,
);

requireAll(
  restart,
  [
    "struct RestartableCursor",
    "impl ConsumerCursor for RestartableCursor",
    "injected acknowledgement failure",
    "event_ack_failure_is_redelivered_after_consumer_reconstruction",
    "decode_failure_ack_failure_is_redelivered_after_consumer_reconstruction",
    "assert_eq!(state.snapshot().await, (false, 1, 1))",
    "assert_eq!(state.snapshot().await, (true, 2, 2))",
    "redelivery.raw_payload()",
    "redelivery.delivery_id()",
  ],
  paths.restart,
);
rejectAll(
  restart,
  ["tokio::time::sleep", "std::thread::sleep"],
  `${paths.restart} deterministic boundary`,
);

requireAll(
  worker,
  [
    "ForumSearchContractIngressOutcome::DurablyAccepted",
    "if !acknowledge_event(group, config, stop_rx, &consumed).await",
    "broker acknowledgement failed; redelivery will recognize the durable inbox row",
    "raw poison acknowledgement failed; redelivery will recognize the durable receipt",
  ],
  `${paths.worker} production acknowledgement order`,
);

requireAll(
  plan,
  [
    "| `FORUM-23` | `in_progress` |",
    "FORUM-23B2G2B3D0 freezes executable runtime evidence",
    "FORUM-23B2G2B3D1 reconciles this canonical plan",
    "maintainer PostgreSQL/Iggy plus LINK-FORUM-03 runtime evidence remain",
  ],
  paths.plan,
);

for (const source of [contract ? JSON.stringify(contract) : "", note, postgres, restart]) {
  rejectAll(
    source,
    [
      "create_forum_search_contract_inbox",
      "second Search inbox",
      "second Forum Search projector",
      "LINK-FORUM-03 closed",
      "runtime_evidence_complete",
    ],
    "D2 non-claim boundary",
  );
}

if (failures.length > 0) {
  console.error("Forum Search D2 source evidence verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search D2 source evidence contract verified.");
