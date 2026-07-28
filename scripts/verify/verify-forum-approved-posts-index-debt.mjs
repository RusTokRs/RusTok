#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath ?? "");
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-approved-posts-posting-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const schema = read(contract.initial_schema_owner);
const note = read(contract.owner_note);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26H" ||
  contract.composition?.author_count_index_added !== false ||
  contract.composition?.query_plan_evidence_added !== false ||
  contract.composition?.migration_changed !== false
) {
  failures.push(
    "FORUM-26H must keep author-count indexes, query-plan evidence and migrations explicitly undelivered",
  );
}

const indexDebt =
  "author-count index hardening and query-plan evidence before posting enforcement";
if (!contract.not_delivered?.includes(indexDebt)) {
  failures.push("FORUM-26H must keep author-count index and query-plan debt explicit");
}

for (const marker of [
  "idx_forum_topics_tenant_category_pinned_reply",
  "idx_forum_topics_tenant_status_updated",
  "idx_forum_replies_topic_position",
  "idx_forum_replies_topic_created",
]) {
  requireText(schema, marker, `initial Forum schema is missing expected existing index ${marker}`);
}

for (const marker of [
  "## Performance boundary",
  "does not add dedicated exact-author count indexes",
  "query plans against representative tenant cardinality",
  "author-count index and query-plan hardening",
  "before any posting owner begins invoking the policy composer synchronously",
]) {
  requireText(note, marker, `FORUM-26H owner note is missing performance debt marker ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum approved-posts index debt verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum approved-posts index debt remains explicit before enforcement.");
