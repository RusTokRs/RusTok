#!/usr/bin/env node

import { readFileSync } from "node:fs";

const test = readFileSync(
  "crates/rustok-iggy-connector/tests/consumer_poison_receipt_postgres.rs",
  "utf8",
);
const failures = [];

function requireText(text) {
  if (!test.includes(text)) {
    failures.push(`PostgreSQL poison receipt evidence is missing: ${text}`);
  }
}

function forbidText(text) {
  if (test.includes(text)) {
    failures.push(`PostgreSQL poison receipt evidence contains forbidden marker: ${text}`);
  }
}

for (const marker of [
  '#![cfg(feature = "migrations")]',
  'RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL',
  '.or_else(|_| std::env::var("DATABASE_URL"))',
  'CREATE SCHEMA "{schema_name}"',
  'DROP SCHEMA IF EXISTS "{}" CASCADE',
  'SET search_path TO "{schema_name}", public',
  '.max_connections(1)',
  'async fn connect_worker(&self)',
  'ConsumerPoisonReceiptStore::new(test_db.connect_worker().await?)',
  'tokio::join!(',
  'concurrent_publishers_have_one_claim_owner',
  'expired_lease_is_reclaimed_and_fences_the_previous_publisher',
  'conflicts_roll_back_without_overwriting_original_identity',
  'terminal_states_and_aggregate_inspection_remain_consistent',
  'ConsumerPoisonPublishClaim::Busy',
  'ConsumerPoisonReceiptError::ClaimLost',
  'ConsumerPoisonReceiptError::IdentityConflict',
  'CURRENT_TIMESTAMP - INTERVAL \'1 second\'',
  '("iggy.contract.decode_invalid", 1) | ("iggy.contract.schema_invalid", 2)',
  'the winning reservation must retain one atomic first-observed diagnostic pair',
  'retained.stable_error_code, "iggy.contract.decode_invalid"',
  'retained.first_delivery_attempt_count, 1',
  'assert_eq!(count_receipts(&test_db.db).await?, 1)',
  'ConsumerPoisonReceiptInspector::new(test_db.db.clone())',
  'assert_eq!(summary.reserved(), 1)',
  'assert_eq!(summary.published(), 1)',
  'assert_eq!(summary.acknowledged(), 1)',
  'ConsumerPoisonPublishClaim::AlreadyPublished',
  'ConsumerPoisonPublishClaim::AlreadyAcknowledged',
]) {
  requireText(marker);
}

for (const marker of [
  'postgres://localhost',
  'postgresql://localhost',
  'DROP DATABASE',
  'CREATE DATABASE',
  'TRUNCATE ',
  'DELETE FROM iggy_consumer_poison_receipts',
  'std::thread::sleep',
  'tokio::time::sleep',
  '.max_connections(4)',
]) {
  forbidText(marker);
}

const directUpdates = [...test.matchAll(/UPDATE iggy_consumer_poison_receipts/g)].length;
if (directUpdates !== 1) {
  failures.push(
    `PostgreSQL poison receipt evidence must contain exactly one direct receipt UPDATE for deterministic lease expiry; found ${directUpdates}`,
  );
}

if (failures.length > 0) {
  console.error("Iggy consumer poison PostgreSQL evidence verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Iggy consumer poison PostgreSQL evidence verification passed: opt-in isolated schemas, independent claim connections, ownership fencing, collision rollback, atomic first-diagnostic retention, and aggregate terminal consistency are locked.",
);
