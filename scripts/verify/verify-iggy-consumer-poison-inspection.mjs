#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  migrations: readFileSync(
    "crates/rustok-iggy-connector/src/migrations.rs",
    "utf8",
  ),
  inspection: readFileSync(
    "crates/rustok-iggy-connector/src/consumer_poison_inspection.rs",
    "utf8",
  ),
};

const failures = [];

function requireText(name, source, text) {
  if (!source.includes(text)) {
    failures.push(`${name} is missing required marker: ${text}`);
  }
}

function forbidText(name, source, text) {
  if (source.includes(text)) {
    failures.push(`${name} contains forbidden marker: ${text}`);
  }
}

for (const marker of [
  '#[path = "consumer_poison_inspection.rs"]',
  "ConsumerPoisonReceiptInspector",
  "ConsumerPoisonReceiptSummary",
]) {
  requireText("connector migration export", files.migrations, marker);
}

for (const marker of [
  "pub struct ConsumerPoisonReceiptSummary",
  "total: u64",
  "reserved: u64",
  "publishing: u64",
  "expired_publishing: u64",
  "published: u64",
  "acknowledged: u64",
  "pub const fn has_recovery_work",
  "pub const fn has_expired_claims",
  "pub struct ConsumerPoisonReceiptInspector",
  "pub async fn summarize(",
  "validate_consumer_group(consumer_group)?",
  "SELECT COUNT(*) AS total",
  "COALESCE(SUM(CASE WHEN state = 'reserved'",
  "state = 'publishing' AND lease_expires_at <= CURRENT_TIMESTAMP",
  "FROM iggy_consumer_poison_receipts WHERE consumer_group = {prefix}1",
  "recognized != summary.total",
  "summary.expired_publishing > summary.publishing",
  "aggregate state counts do not match the total receipt count",
  "unknown_state_fails_closed",
]) {
  requireText("consumer poison inspection", files.inspection, marker);
}

const production = files.inspection.split("#[cfg(test)]")[0];
for (const forbidden of [
  "delivery_id:",
  ".delivery_id",
  "source_stream:",
  ".source_stream",
  "source_topic:",
  ".source_topic",
  "source_partition:",
  ".source_partition",
  "source_offset:",
  ".source_offset",
  "payload:",
  ".payload",
  "raw_payload",
  "stable_error_code:",
  ".stable_error_code",
  "publisher_id:",
  ".publisher_id",
  "ack_token",
  "tenant_id",
  "event_id",
  "DELETE ",
  "INSERT ",
  "UPDATE ",
  "reserve_and_claim",
  "release_claim",
  "mark_published",
  "mark_acknowledged",
  "move_to_dlq",
  "acknowledge(",
]) {
  forbidText("read-only inspection production code", production, forbidden);
}

if (failures.length > 0) {
  console.error("Iggy consumer poison inspection verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Iggy consumer poison inspection verification passed: one bounded consumer-group aggregate, known-state integrity, expired-lease counting, read-only behavior, and identity/payload isolation are locked.",
);
